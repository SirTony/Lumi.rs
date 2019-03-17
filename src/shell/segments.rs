use std::io::{ BufRead, BufReader, Read, Write, Result, Error, ErrorKind };
use std::collections::HashMap;
use std::boxed::Box;
use std::fs::File;
use std::path::Path;
use std::process::{ Command, Child, Stdio };
use std::env::{ VarError, var, set_var };
use kernel::{ get_exit_code, clear_screen };
use std::any::Any;
use clap::{ App, AppSettings };

type CommandAction = fn( Vec<String>, Option<Vec<String>> ) -> Result<ShellResult>;

macro_rules! make_app {
    ( $y: expr ) => {{
        App::from_yaml( $y )
            .author( crate_authors!() )
            .version( crate_version!() )
            .setting( AppSettings::ColoredHelp )
            .setting( AppSettings::ColorAuto )
    }}
}

fn change_dir( argv: Vec<String>, _input: Option<Vec<String>> ) -> Result<ShellResult> {
    let yaml = load_yaml!( "cli_args/cd.yaml" );
    match make_app!( yaml ).get_matches_from_safe( argv ) {
        Ok( args ) => {
            println!( "{:#?}", args.value_of( "DIR" ) );
            ShellResult::ok()
        },

        Err( e ) => {
            eprintln!( "{}", e );
            ShellResult::ok()
        }
    }
}

fn clear( _argv: Vec<String>, _input: Option<Vec<String>> ) -> Result<ShellResult> {
    unsafe { clear_screen(); }
    ShellResult::ok()
}

lazy_static! {
    static ref COMMANDS: HashMap<&'static str, CommandAction> = {
        let mut map = HashMap::new();

        map.insert( "cd", change_dir as CommandAction );
        map.insert( "cls", clear as CommandAction );
        map.insert( "clear", clear as CommandAction );

        map
    };
}

#[derive( Debug )]
pub struct ShellResult {
    code: Option<i32>,
    stdout: Option<Vec<String>>,
    stderr: Option<Vec<String>>
}

impl ShellResult {
    pub fn code( &self ) -> Option<i32> {
        self.code
    }

    pub fn ok() -> Result<ShellResult> {
        Ok( ShellResult {
            code: Some( 0 ),
            stdout: None,
            stderr: None
        } )
    }

    pub fn  ok_with_text( s: String ) -> Result<ShellResult> {
        Ok( ShellResult {
            code: Some( 0 ),
            stdout: Some( vec![ s ] ),
            stderr: None
        } )
    }
}

macro_rules! ensure_result {
    ( $r: expr ) => {{
        if $r.code().is_none() || $r.code().unwrap() != 0 {
            return Ok( $r );
        }
    }}
}

pub trait Executable {
    fn execute( &self, capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult>;
    fn as_any( &self ) -> &dyn Any;
}

#[derive( Debug, Eq, PartialEq )]
pub enum RedirectMode {
    StdIn,
    StdOut,
    StdErr,
    StdBoth,
}

pub type Exec = Box<dyn Executable>;

pub struct Empty;

impl Executable for Empty {
    fn execute( &self, _capture: bool, _input: Option<Vec<String>> ) -> Result<ShellResult> {
        ShellResult::ok()
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct Text( pub String );

impl Executable for Text {
    fn execute( &self, _capture: bool, _input: Option<Vec<String>> ) -> Result<ShellResult> {
        ShellResult::ok_with_text( self.0.clone() )
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct Cmd {
    pub command: Exec,
    pub args: Option<Vec<Exec>>,
}

impl Executable for Cmd {
    fn execute( &self, capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult> {
        let res = self.command.execute( true, None )?;
        ensure_result!( res );

        let name = format!( "{}", res.stdout.unwrap().join( "" ) );
        let mut argv = Vec::new();
        if let Some( args ) = &self.args {
            for x in args.iter() {
                if let Some( lines ) = x.execute( true, None )?.stdout {
                    for line in lines { argv.push( line ); }
                }
            }
        }

        if let Some( cmd ) = COMMANDS.get( &*name ) {
            argv.insert( 0, name );
            return cmd( argv, input );
        }

        let mut proc = Command::new( &name );
        if input.is_some() {
            proc.stdin( Stdio::piped() );
        }

        if capture {
            proc.stdout( Stdio::piped() );
            proc.stderr( Stdio::piped() );
        }

        proc.args( argv );

        let subprocess = if let Some( lines ) = input {
            let mut child = proc.spawn()?;
            {
                let stdin = child.stdin.as_mut();
                if let Some( stdin ) = stdin {
                    for line in lines {
                        writeln!( stdin, "{}", line )?;
                    }
                }
            }

            SubProcess::Spawned { process: child, capture }
        } else {
            SubProcess::Waiting { process: proc, capture }
        };

        match subprocess.result() {
            Ok( x ) => Ok( x ),
            Err( ref e ) if e.kind() == ErrorKind::NotFound
                => Err(
                    Error::new(
                        ErrorKind::NotFound,
                        format!(
                            "'{name}' is not a recognized command, script file, or executable program.",
                            name = name
                        )
                    )
                ),
            Err( e ) => Err( e )
        }
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct TextInterp( pub Vec<Exec> );

impl Executable for TextInterp {
    fn execute( &self, _capture: bool, _input: Option<Vec<String>> ) -> Result<ShellResult> {
        let mut parts = Vec::new();
        for seg in &self.0 {
            let res = seg.execute( true, None )?;
            ensure_result!( res );

            if let Some( mut lines ) = res.stdout {
                parts.append( &mut lines );
            }
        }

        ShellResult::ok_with_text( parts.join( "" ) )
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct CmdInterp( pub Exec );

impl Executable for CmdInterp {
    fn execute( &self, _capture: bool, _input: Option<Vec<String>> ) -> Result<ShellResult> {
        self.0.execute( true, None )
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct Pipe {
    pub left: Exec,
    pub right: Exec,
}

impl Executable for Pipe {
    fn execute( &self, capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult> {
        let left = self.left.execute( true, input )?;
        ensure_result!( left );

        self.right.execute( capture, left.stdout )
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct Seq {
    pub safe: bool,
    pub left: Exec,
    pub right: Exec,
}

impl Executable for Seq {
    fn execute( &self, capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult> {
        if self.safe {
            let left = self.left.execute( false, None )?;
            ensure_result!( left );

            self.right.execute( capture, input )
        } else {
            self.left.execute( false, None )?;
            self.right.execute( capture, input )
        }
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct Var( pub String );

impl Executable for Var {
    fn execute( &self, _capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult> {
        match input {
            Some( x ) => {
                let value = x.join( " " );
                set_var( &self.0, &value );
                ShellResult::ok_with_text( value )
            },

            None => match var( &self.0 ) {
                Ok( x ) => ShellResult::ok_with_text( x ),
                Err( e ) => match e {
                    VarError::NotPresent => Err(
                        Error::new(
                            ErrorKind::Other,
                            format!( "variable '{}' not found", self.0 )
                        )
                    ),

                    VarError::NotUnicode( _ ) => Err(
                        Error::new(
                            ErrorKind::Other,
                            format!( "variable '{}' contains invalid data", self.0 )
                        )
                    )
                }
            }
        }
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

pub struct Redirect {
    pub mode: RedirectMode,
    pub left: Exec,
    pub right: Exec,
}

impl Executable for Redirect {
    fn execute( &self, _capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult> {
        use self::RedirectMode::*;

        let right = self.right.execute( true, None )?;
        ensure_result!( right );

        let s = match right.stdout {
            Some( x ) => x.join( "" ),
            None => String::new()
        };

        let path = Path::new( &s );
        let input = match &self.mode {
            StdIn => {
                let f = File::open( path )?;
                let mut reader = BufReader::new( f );
                let mut lines = Vec::new();

                for line in reader.lines() {
                    lines.push( line? );
                }

                if lines.len() == 0 { None } else { Some( lines ) }
            },
            _ => input,
        };

        let left = self.left.execute( true, input )?;
        ensure_result!( left );

        if &self.mode == &StdOut || &self.mode == &StdErr || &self.mode == &StdBoth {
            let mut f = File::create( path )?;

            if &self.mode == &StdOut || &self.mode == &StdBoth {
                if let Some( stdout ) = left.stdout {
                    for line in stdout {
                        f.write( line.as_bytes() )?;
                        f.write( b"\n" )?;
                    }
                }
            }

            if &self.mode == &StdErr || &self.mode == &StdBoth {
                if let Some( stderr ) = left.stderr {
                    for line in stderr {
                        f.write( line.as_bytes() )?;
                        f.write( b"\n" )?;
                    }
                }
            }

            f.flush()?;
            f.sync_all()?;
        }

        ShellResult::ok()
    }

    fn as_any( &self ) -> &dyn Any {
        self
    }
}

enum SubProcess {
    Spawned {
        process: Child,
        capture: bool,
    },

    Waiting {
        process: Command,
        capture: bool,
    }
}

impl SubProcess {
    pub fn result( self ) -> Result<ShellResult> {
        use self::SubProcess::*;

        match self {
            Spawned { mut process, capture: true } => SubProcess::read_child( &mut process ),
            Spawned { mut process, capture: false } => Ok( ShellResult {
                code: get_exit_code( process.wait()? ),
                stdout: None,
                stderr: None,
            } ),

            Waiting { mut process, capture: true } => SubProcess::read_command( &mut process ),
            Waiting { mut process, capture: false } => Ok( ShellResult {
                code: get_exit_code( process.status()? ),
                stdout: None,
                stderr: None,
            } )
        }
    }

    fn read_command( proc: &mut Command ) -> Result<ShellResult> {
        let res = proc.output()?;
        Ok( ShellResult {
            code: get_exit_code( res.status ),
            stdout: if res.stdout.len() > 0 {
                let buf = String::from_utf8_lossy( &res.stdout ).into_owned();
                Some( SubProcess::split_lines( buf ) )
            } else {
                None
            },
            stderr: if res.stderr.len() > 0 {
                let buf = String::from_utf8_lossy( &res.stderr ).into_owned();
                Some( SubProcess::split_lines( buf ) )
            } else {
                None
            }
        } )
    }

    fn split_lines( buf: String ) -> Vec<String> {
        buf.split( "\n" )
        .map( | x | x.trim() )
        .filter( | x | x.len() > 0 )
        .map( | x | x.to_string() )
        .collect()
    }

    fn read_child( child: &mut Child ) -> Result<ShellResult> {
        Ok( ShellResult {
            code: get_exit_code( child.wait()? ),
            stdout:
            if let Some( mut stdout ) = child.stdout.take() {
                let mut buf = String::new();
                let sz = stdout.read_to_string( &mut buf )?;

                if sz > 0 {
                    Some( SubProcess::split_lines( buf ) )
                } else {
                    None
                }
            } else {
                None
            },
            stderr:
            if let Some( mut stderr ) = child.stderr.take() {
                let mut buf = String::new();
                let sz = stderr.read_to_string( &mut buf )?;

                if sz > 0 {
                    Some( SubProcess::split_lines( buf ) )
                } else {
                    None
                }
            } else {
                None
            }
        } )
    }
}
