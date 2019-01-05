use std::io::{ BufRead, BufReader, Read, Write, Result, Error, ErrorKind };
use std::boxed::Box;
use std::fs::File;
use std::path::Path;
use std::process::{ Command, Child, Stdio };
use std::env::{ VarError, var, set_var };
use kernel::get_exit_code;

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

#[derive( Debug, Eq, PartialEq )]
pub enum RedirectMode {
    StdIn,
    StdOut,
    StdErr,
    StdBoth,
}

#[derive( Debug )]
pub enum ShellSegment {
    Empty,
    Text { text: String },
    Command {
        cmd: Box<ShellSegment>,
        args: Option<Vec<ShellSegment>>,
    },
    StringInterp { parts: Vec<ShellSegment> },
    CmdInterp { cmd: Box<ShellSegment> },
    Pipe {
        left: Box<ShellSegment>,
        right: Box<ShellSegment>
    },
    Seq {
        safe: bool,
        left: Box<ShellSegment>,
        right: Box<ShellSegment>
    },
    Var { name: String },
    Redirect {
        left: Box<ShellSegment>,
        right: Box<ShellSegment>,
        mode: RedirectMode
    },
}

impl ShellSegment {
    pub fn execute( &self, capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult> {
        match self {
            ShellSegment::Empty => ShellResult::ok(),
            ShellSegment::Text { text } => ShellResult::ok_with_text( text.clone() ),
            ShellSegment::CmdInterp { cmd } => cmd.execute( true, None ),
            ShellSegment::StringInterp { parts: segs } => {
                let mut parts = Vec::new();
                for seg in segs {
                    let res = seg.execute( true, None )?;
                    ensure_result!( res );

                    if let Some( mut lines ) = res.stdout {
                        parts.append( &mut lines );
                    }
                }

                ShellResult::ok_with_text( parts.join( "" ) )

            },
            ShellSegment::Seq { safe, left, right } =>
            if *safe {
                let left = left.execute( capture, None )?;
                match left.code {
                    Some( 0 ) => right.execute( capture, None ),
                    _ => Ok( left )
                }
            } else {
                left.execute( capture, None )?;
                right.execute( capture, None )
            },
            ShellSegment::Pipe { left, right } => {
                let left = left.execute( true, input )?;
                if let Some( code ) = left.code {
                    if code == 0 {
                        return right.execute( capture, left.stdout );
                    }
                }

                Ok( left )
            },
            ShellSegment::Redirect { left, right, mode } => {
                use self::RedirectMode::*;

                let right = right.execute( true, None )?;
                ensure_result!( right );

                let s = match right.stdout {
                    Some( x ) => x.join( "" ),
                    None => "".into()
                };
                let path = Path::new( &s );
                let input = match mode {
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

                let left = left.execute( true, input )?;
                ensure_result!( left );

                if *mode == StdOut || *mode == StdErr || *mode == StdBoth {
                    let mut f = File::create( path )?;

                    if *mode == StdOut || *mode == StdBoth {
                        if let Some( stdout ) = left.stdout {
                            for line in stdout {
                                f.write( line.as_bytes() )?;
                                f.write( b"\n" )?;
                            }
                        }
                    }

                    if *mode == StdErr || *mode == StdBoth {
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
            },
            ShellSegment::Var { name } => {
                match input {
                    Some( x ) => {
                        let value = x.join( " " );
                        set_var( name, &value );
                        ShellResult::ok_with_text( value )
                    },

                    None => match var( name ) {
                        Ok( value ) => ShellResult::ok_with_text( value ),
                        Err( e ) => match e {
                            VarError::NotPresent => Err(
                                Error::new(
                                    ErrorKind::Other,
                                    format!( "variable '{}' not found", name )
                                )
                            ),

                            VarError::NotUnicode( _ ) => Err(
                                Error::new(
                                    ErrorKind::Other,
                                    format!( "variable '{}' contains invalid data", name )
                                )
                            )
                        }
                    },
                }
            },
            ShellSegment::Command { cmd, args } => {
                let res = cmd.execute( true, None )?;
                ensure_result!( res );

                let name = format!( "{}", res.stdout.unwrap().join( "" ) );
                let mut proc = Command::new( &name );
                if input.is_some() {
                    proc.stdin( Stdio::piped() );
                }

                if capture {
                    proc.stdout( Stdio::piped() );
                    proc.stderr( Stdio::piped() );
                }

                if let Some( args ) = args {
                    for x in args.iter() {
                        if let Some( lines ) = x.execute( true, None )?.stdout {
                            for line in lines { proc.arg( line ); }
                        }
                    }
                }

                if let Some( lines ) = input {
                    if lines.len() > 0 {
                        let mut child = proc.spawn()?;
                        {
                            let stdin = child.stdin.as_mut();
                            if let Some( stdin ) = stdin {
                                for line in lines {
                                    writeln!( stdin, "{}", line )?;
                                }
                            }
                        }

                        if capture {
                            ShellSegment::read_child( &mut child )
                        } else {
                            ShellSegment::get_status( &mut proc )
                        }
                    } else {
                        if capture {
                            ShellSegment::read_command( &mut proc )
                        } else {
                            ShellSegment::get_status( &mut proc )
                        }
                    }
                } else {
                    if capture {
                        ShellSegment::read_command( &mut proc )
                    } else {
                        ShellSegment::get_status( &mut proc )
                    }
                }
            }
        }
    }

    fn read_command( proc: &mut Command ) -> Result<ShellResult> {
        let res = proc.output()?;
        Ok( ShellResult {
            code: get_exit_code( res.status ),
            stdout: if res.stdout.len() > 0 {
                Some(
                    ShellSegment::split_lines(
                        String::from_utf8_lossy( &res.stdout ).into_owned()
                    )
                )
            } else {
                None
            },
            stderr: if res.stderr.len() > 0 {
                Some(
                    ShellSegment::split_lines(
                        String::from_utf8_lossy( &res.stderr ).into_owned()
                    )
                )
            } else {
                None
            }
        } )
    }

    fn get_status( proc: &mut Command ) -> Result<ShellResult> {
        Ok( ShellResult {
            code: get_exit_code( proc.status()? ),
            stdout: None,
            stderr: None
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
                    Some( ShellSegment::split_lines( buf ) )
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
                    Some( ShellSegment::split_lines( buf ) )
                } else {
                    None
                }
            } else {
                None
            }
        } )
    }
}
