use std::io::{ Read, Write, Result, Error, ErrorKind };
use std::boxed::Box;
use std::process::{ Command, Stdio, ExitStatus };
use kernel::get_exit_code;

#[derive( Debug )]
pub struct ShellResult {
    code: Option<i32>,
    stdout: Option<Vec<String>>,
    stderr: Option<Vec<String>>
}

impl ShellResult {
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

#[derive( Debug )]
pub enum ShellSegment {
    Text( String ),
    Command( String, Option<Vec<Box<ShellSegment>>> ),
    Interp( Box<ShellSegment> ),
    Pipe( Box<ShellSegment>, Box<ShellSegment> ),
    Seq( bool, Box<ShellSegment>, Box<ShellSegment> ),
    Var( Option<String>, String ),
}

impl ShellSegment {
    pub fn execute( &self, capture: bool, input: Option<Vec<String>> ) -> Result<ShellResult> {
        match self {
            ShellSegment::Text( s ) => ShellResult::ok_with_text( s.clone() ),
            ShellSegment::Interp( seg ) => seg.execute( true, None ),
            ShellSegment::Seq( safe, left, right ) =>
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
            ShellSegment::Pipe( left, right ) => {
                let left = left.execute( true, input )?;
                if let Some( code ) = left.code {
                    if code == 0 {
                        return right.execute( capture, left.stdout );
                    }
                }

                Ok( left )
            }
            ShellSegment::Command( cmd, args ) => {
                let mut proc = Command::new( cmd );

                if input.is_some() {
                    proc.stdin( Stdio::piped() );
                } else {
                    proc.stdin( Stdio::inherit() );
                }

                if capture {
                    proc.stdout( Stdio::piped() );
                    proc.stderr( Stdio::piped() );
                }

                if let Some( args ) = args {
                    for x in args.into_iter() {
                        if let Some( lines ) = x.execute( true, None )?.stdout {
                            for line in lines { proc.arg( line ); }
                        }
                    }
                }

                let mut child = proc.spawn()?;
                if let Some( lines ) = input {
                    for line in lines {
                        writeln!( child.stdin.as_mut().unwrap(), "{}", line )?;
                    }
                }

                if capture {
                    Ok( ShellResult {
                        code: get_exit_code( child.wait()? ),
                        stdout:
                        if let Some( mut stdout ) = child.stdout {
                            let mut buf = String::new();
                            let sz = stdout.read_to_string( &mut buf )?;

                            if sz > 0 {
                                Some(
                                    buf.split( "\n" )
                                    .map( | x | x.trim() )
                                    .filter( | x | x.len() > 0 )
                                    .map( | x | x.to_string() )
                                    .collect()
                                    )
                            } else {
                                None
                            }
                        } else {
                            None
                        },
                        stderr:
                        if let Some( mut stderr ) = child.stderr {
                            let mut buf = String::new();
                            let sz = stderr.read_to_string( &mut buf )?;

                            if sz > 0 {
                                Some(
                                    buf.split( "\n" )
                                    .map( | x | x.trim() )
                                    .filter( | x | x.len() > 0 )
                                    .map( | x | x.to_string() )
                                    .collect()
                                )
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } )
                } else {
                    Ok( ShellResult {
                        code: get_exit_code( proc.spawn()?.wait()? ),
                        stdout: None,
                        stderr: None
                    } )
                }
            },

            _ => Err( Error::new( ErrorKind::Other, "unimplemented segment" ) )
        }
    }
}
