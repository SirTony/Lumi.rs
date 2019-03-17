extern crate yansi;
extern crate whoami;
extern crate dirs;
extern crate crossterm;

#[macro_use]
extern crate lazy_static;

#[cfg( windows )]
extern crate winapi;

#[macro_use]
extern crate clap;

mod peek;
mod parsing;
mod shell;
mod kernel;
mod empty;

use std::env::{ current_dir as env_current_dir };
use std::io::{ Result, Error, ErrorKind, Write, stdin, stdout };
use std::fmt::Display;
use yansi::Paint;
use crossterm::terminal;
use kernel::{ clear_screen, disable_ctrl_c };
use shell::parsing::*;
use parsing::*;

fn main() {
    unsafe {
        disable_ctrl_c();
        clear_screen();
    }

    loop {
        print_prompt();

        let mut line = String::new();
        match stdin().read_line( &mut line ) {
            Ok( _ ) => {
                if line.trim().len() == 0 {
                    println!( "" );
                    continue;
                }

                line = line.trim_end_matches( | c | c == '\r' || c == '\n' ).to_string();
                let mut lexer = ShellLexer::new( line.clone() );
                let tokens = match lexer.tokenize() {
                    Ok( tks ) => tks,
                    Err( e ) => {
                        show_lex_error( e, &line );
                        continue;
                    },
                };

                let mut parser = ShellParser::new( tokens );
                let seg = match parser.parse_all() {
                    Ok( seg ) => seg,
                    Err( e ) => {
                        show_parse_error( e, &line );
                        continue;
                    },
                };

                //let res = seg.execute( false, None );
                match seg.execute( false, None ) {
                    Err( e ) => {
                        println!( "" );
                        error( e );
                        println!( "" );
                    },
                    _ => {},
                }

                //println!( "{:#?}", res );
                stdout().flush().unwrap();
            },
            Err( e ) => {
                error( format!( "unable to read from STDIN (reason: {})", e.to_string() ) );
            }
        }
    }
}

fn error<D: Display>( msg: D ) {
    let painted = Paint::red( msg ).dimmed();
    println!( "{}", painted );
}

fn show_lex_error( e: LexError, input: &String ) {
    use parsing::LexErrorKind::*;

    match e.kind() {
        UnexpectedChar { character, codepoint } => {
            error(
                format!(
                    "unexpected character '{0}' (0x{1:X}) at position {2}",
                    character,
                    codepoint,
                    e.span().start.index
                )
            );

            point_to( input, e.span().start.index );
        },

        UnexpectedEOI { reason } => {
            error(
                format!(
                    "unexpected end-of-input ({0}) at position {1}",
                    reason,
                    e.span().start.index
                )
            );

            point_to( input, e.span().start.index );
        },
    }
}

fn show_parse_error( e: ParseError, input: &String ) {
    use parsing::ParseErrorKind::*;

    match e.kind() {
        ExpectSegment { found } => {
            error(
                format!(
                    "expecting shell segment, found {0} at position {1}",
                    found,
                    e.span().unwrap().start.index
                )
            );

            point_to( input, e.span().unwrap().start.index );
        },

        ExpectString => {
            error(
                format!(
                    "redirection target must be a string or string interpolation (at position {})",
                    e.span().unwrap().start.index
                )
            );

            point_to( input, e.span().unwrap().start.index );
        },

        UnexpectedEOI => {
            error(
                format!(
                    "unexpected end-of-input (malformed token stream, indicates an internal bug)"
                )
            );
        },

        Unexpected { expect, found } => {
            error(
                format!(
                    "unexpected {0}, expecting {1} at position {2}",
                    found,
                    expect,
                    e.span().unwrap().start.index
                )
            );

            point_to( input, e.span().unwrap().start.index );
        }
    }
}

fn point_to( input: &String, at: usize ) {
    let pad_size: usize = 10;
    let prefix = "... ";
    let term = terminal();
    let ( w, _ ) = term.terminal_size();

    let should_trim = at > pad_size && input.len() > w as usize;
    let mut section = if should_trim {
        format!(
            "{0}{1}",
            prefix,
            &input[( at - pad_size )..]
        )
    } else {
        input.clone()
    };

    if section.len() > w as usize {
        let suffix = " ...";
        section = format!(
            "{0}{1}",
            &input[..( w as usize - suffix.len() )],
            suffix
        );
    }

    let len = if should_trim {
        pad_size + prefix.len()
    } else {
        at
    };

    let ws: String = ( 0 .. len ).map( | _ | ' ' ).collect();
    let ln: String = ( 0 .. len ).map( | _ | '─' ).collect();

    println!( "" );
    println!( "{}", section );
    println!( "{}", Paint::red( format!( "{}^", ws ) ) );
    println!( "{}", Paint::red( format!( "{}┘", ln ) ) );
    stdout().flush().unwrap();
}

fn print_prompt() {
    use whoami::{ username, host as computer };

    fn get_current_dir() -> String {
        let full_path = current_dir().unwrap();

        // std::fs::canonicalize returns a full UNC path
        // with preceeding \\?\ on Windows so we need to trim that.
        if full_path.starts_with( "\\\\?\\" ) {
            full_path[4..].to_string()
        } else {
            full_path
        }
    }

    print!(
        "${user}@{machine}[{dir}]> ",
        user    = Paint::green( username() ),
        machine = Paint::yellow( computer() ).dimmed(),
        dir     = Paint::cyan( get_current_dir() ).dimmed()
    );

    stdout().flush().unwrap();
}

fn current_dir() -> Result<String> {
    use dirs::home_dir;

    let home = home_dir().ok_or(
        Error::new(
            ErrorKind::Other,
            "unable to locate user's home dir!"
        )
    )?.canonicalize()?;

    let curr = env_current_dir()?.canonicalize()?;

    if curr.starts_with( &home ) {
        let home = home.to_string_lossy().into_owned();
        let curr = curr.to_string_lossy().into_owned();

        Ok( curr.replace( &home, "~" ) )
    } else {
        Ok( curr.to_string_lossy().into_owned().to_string() )
    }
}
