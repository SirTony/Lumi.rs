use yansi::Paint;
use std::env::current_dir;
use std::io::{ Result, Error, ErrorKind, Write, stdin, stdout };
use std::fmt::Display;
use shell::config::{ Config, ColorSpace, ColorPalette, PromptStyle };
use kernel::{ clear_screen, disable_ctrl_c };
use shell::parsing::*;
use parsing::*;
use crossterm::terminal;

pub struct Repl<'a> {
    config: &'a Config
}

impl<'a> Repl<'a> {
    pub fn new( cfg: &'a Config ) -> Repl {
        Repl {
            config: cfg
        }
    }

    pub fn run( &self ) {
        if self.config.palette().is_some() && cfg!( windows ) {
            ColorPalette::enable_windows_ascii();
        }

        unsafe {
            disable_ctrl_c();
            clear_screen();
        }

        loop {
            self.print_prompt();

            let mut line = String::new();
            match stdin().read_line( &mut line ) {
                Ok( _ ) => {
                    if line.trim().len() == 0 {
                        println!( "" );
                        continue;
                    }

                    line = line.trim_end_matches( '\r' ).to_string();
                    let mut lexer = ShellLexer::new( line.clone() );
                    let tokens = match lexer.tokenize() {
                        Ok( tks ) => tks,
                        Err( e ) => {
                            self.show_lex_error( e, &line );
                            continue;
                        },
                    };

                    let mut parser = ShellParser::new( tokens );
                    let seg = match parser.parse_all() {
                        Ok( seg ) => seg,
                        Err( e ) => {
                            self.show_parse_error( e, &line );
                            continue;
                        },
                    };

                    let res = seg.execute( false, None );
                    println!( "{:#?}", res.unwrap() );
                },
                Err( e ) => {
                    self.error( format!( "unable to read from STDIN (reason: {})", e.to_string() ) );
                }
            }
        }
    }

    fn show_lex_error( &self, e: LexError, input: &String ) {
        use parsing::LexErrorKind::*;

        match e.kind() {
            UnexpectedChar { character, codepoint } => {
                self.error(
                    format!(
                        "unexpected character '{0}' (0x{1:X}) at position {2}",
                        character,
                        codepoint,
                        e.span().start.index
                    )
                );

                self.point_to( input, e.span().start.index );
            },

            UnexpectedEOI { reason } => {
                self.error(
                    format!(
                        "unexpected end-of-input ({0}) at position {1}",
                        reason,
                        e.span().start.index
                    )
                );

                self.point_to( input, e.span().start.index );
            },
        }
    }

    fn show_parse_error( &self, e: ParseError, input: &String ) {
        use parsing::ParseErrorKind::*;

        match e.kind() {
            ExpectSegment { found } => {
                self.error(
                    format!(
                        "expecting shell segment, found {0} at position {1}",
                        found,
                        e.span().unwrap().start.index
                    )
                );

                self.point_to( input, e.span().unwrap().start.index );
            },

            ExpectString => {
                self.error(
                    format!(
                        "redirection target must be a string or string interpolation (at position {})",
                        e.span().unwrap().start.index
                    )
                );

                self.point_to( input, e.span().unwrap().start.index );
            },

            UnexpectedEOI => {
                self.error(
                    format!(
                        "unexpected end-of-input (malformed token stream, indicates an internal bug)"
                    )
                );
            },

            Unexpected { expect, found } => {
                self.error(
                    format!(
                        "unexpected {0}, expecting {1} at position {2}",
                        found,
                        expect,
                        e.span().unwrap().start.index
                    )
                );

                self.point_to( input, e.span().unwrap().start.index );
            }
        }
    }

    fn point_to( &self, input: &String, at: usize ) {
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
        println!( "{}", self.paint( ColorSpace::Error( format!( "{}^", ws ) ) ) );
        println!( "{}", self.paint( ColorSpace::Error( format!( "{}┘", ln ) ) ) );
    }

    fn notice<D: Display>( &self, msg: D ) {
        self.msg( ColorSpace::Notice( "INF" ), msg )
    }

    fn warning<D: Display>( &self, msg: D ) {
        self.msg( ColorSpace::Warning( "WRN" ), msg )
    }

    fn error<D: Display>( &self, msg: D ) {
        self.msg( ColorSpace::Error( "ERR" ), msg )
    }

    fn msg<D: Display, T: Display>( &self, tag: ColorSpace<T>, msg: D ) {
        println!( "[{0}] :: {1}", self.paint( tag ), msg );
    }

    fn paint<D>( &self, what: ColorSpace<D> ) -> Paint<D> {
        if let Some( palette ) = self.config.palette() {
            palette.paint( what )
        } else {
            Paint::new( what.unwrap() )
        }
    }

    fn print_prompt( &self ) {
        use whoami::{ username, computer };

        fn get_current_dir( use_tilde: bool ) -> ColorSpace<String> {
            let full_path = Repl::current_dir( use_tilde ).unwrap();

            ColorSpace::Dir(
                // std::fs::canonicalize returns a full UNC path
                // with preceeding \\?\ on Windows so we need to trim that.
                if full_path.starts_with( "\\\\?\\" ) {
                    full_path[4..].to_string()
                } else {
                    full_path
                }
            )
        }

        match self.config.prompt() {
            PromptStyle::Lumi => {
                let username = self.paint( ColorSpace::User( username() ) );
                print!( "$ {0}@{1}> ", username, self.paint( get_current_dir( true ) ) );
            },
            PromptStyle::Windows => {
                // don't display tilde on windows
                print!( "{0}> ", self.paint( get_current_dir( false ) ) );
            },

            PromptStyle::Linux => {
                let username = self.paint( ColorSpace::User( username() ) );
                let machine  = self.paint( ColorSpace::Machine( computer() ) );

                print!( "{0}@{1}:{2}$ ", username, machine, self.paint( get_current_dir( true ) ) );
            }
        }

        stdout().flush().unwrap();
    }

    pub fn current_dir( use_tilde: bool ) -> Result<String> {
        use dirs::home_dir;

        let home = home_dir().ok_or(
            Error::new(
                ErrorKind::Other,
                "unable to locate user's home dir!"
            )
        )?.canonicalize()?;

        let curr = current_dir()?.canonicalize()?;

        if use_tilde && curr.starts_with( &home ) {
            let home = home.to_string_lossy().into_owned();
            let curr = curr.to_string_lossy().into_owned();

            Ok( curr.replace( &home, "~" ) )
        } else {
            Ok( curr.to_string_lossy().into_owned().to_string() )
        }
    }
}
