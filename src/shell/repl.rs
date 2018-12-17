use yansi::Paint;
use std::env::current_dir;
use std::io::{ Result, Error, ErrorKind, Write, stdin, stdout };
use std::fmt::Display;
use shell::config::{ Config, ColorSpace, PromptStyle };
use kernel::disable_ctrl_c;

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
        unsafe { disable_ctrl_c(); }

        if let Some( palette ) = self.config.palette() {
            palette.apply();
        }

        loop {
            self.print_prompt();

            let mut line = String::new();
            match stdin().read_line( &mut line ) {
                Ok( _ ) => {
                    // just echo for now
                    println!( "{0}", line );
                },
                Err( e ) => {
                    self.error( format!( "unable to read from STDIN (reason: {})", e.to_string() ) );
                }
            }
        }
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
            Paint::default( what.unwrap() )
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
                let machine  = self.paint( ColorSpace::User( computer() ) );

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
