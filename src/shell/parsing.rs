use parsing::*;
use std::mem::discriminant;
use std::string::ToString;
use shell::segments::*;
use std::collections::{ HashSet, HashMap };

#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub enum ShellTokenKind {
    String( String ),
    Interp( Vec<ShellToken> ),

    Dollar,
    Semi,
    Amp,
    Pipe,

    // <
    StdIn,

    // >
    StdOut,

    // >>
    StdErr,

    // >>>
    StdBoth,

    LParen,
    RParen,

    EndOfInput,
}

impl ToString for ShellTokenKind {
    fn to_string( &self ) -> String {
        use ShellTokenKind::*;

        match self {
            String( x ) => x.clone(),
            Interp( _ ) => "string interpolation".to_string(),

            Dollar => "$".to_string(),
            Semi => ";".to_string(),
            Amp => "&".to_string(),
            Pipe => "|".to_string(),
            StdIn => "<".to_string(),
            StdOut => ">".to_string(),
            StdErr => ">>".to_string(),
            StdBoth => ">>>".to_string(),
            LParen => "(".to_string(),
            RParen => ")".to_string(),
            EndOfInput => "<end-of-input>".to_string(),
        }
    }
}

#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct ShellToken {
    kind: ShellTokenKind,
    span: TextSpan,
}

impl ToString for ShellToken {
    fn to_string( &self ) -> String {
        self.kind.to_string()
    }
}

impl SyntaxToken for ShellToken {
    type Kind = ShellTokenKind;

    fn kind( &self ) -> &Self::Kind {
        &self.kind
    }

    fn span( &self ) -> &TextSpan {
        &self.span
    }
}

#[derive( Clone )]
pub struct ShellLexer {
    scanner: Scanner,
    mode: LexerMode,
    special: HashSet<char>,
    punct: HashMap<&'static str, ShellTokenKind>,
}

#[derive( Debug, Clone, Eq, PartialEq )]
enum LexerMode {
    Normal,
    Interp,
}

impl ShellLexer {
    pub fn new( source: String ) -> ShellLexer {
        use self::ShellTokenKind::*;

        let mut punct = HashMap::new();
        punct.insert( "$", Dollar );
        punct.insert( ";", Semi );
        punct.insert( "&", Amp );
        punct.insert( "|", Pipe );
        punct.insert( "(", LParen );
        punct.insert( ")", RParen );
        punct.insert( "<", StdIn );
        // these must be kept sorted by length in descending order
        punct.insert( ">>>", StdBoth );
        punct.insert( ">>", StdErr );
        punct.insert( ">", StdOut );

        let mut special = HashSet::new();
        special.insert( '$' );
        special.insert( ';' );
        special.insert( '&' );
        special.insert( '|' );
        special.insert( '<' );
        special.insert( '>' );
        special.insert( '"' );
        special.insert( '\'' );
        special.insert( '`' );
        special.insert( '(' );
        special.insert( ')' );
        special.insert( '{' );
        special.insert( '}' );

        ShellLexer {
            scanner: Scanner::new( source, 0, 1, 1 ),
            mode: LexerMode::Normal,
            special,
            punct,
        }
    }

    pub fn tokenize( &mut self ) -> Result<Vec<ShellToken>, LexError> {
        let tokenizers = &[
            ShellLexer::try_lex_quoted,
            ShellLexer::try_lex_punct,
            ShellLexer::try_lex_unquoted,
        ];

        let mut tokens = Vec::new();
        while !self.scanner.is_empty() {
            self.scanner.skip_while( | c | c.is_whitespace() );

            if self.scanner.is_empty() { break; }

            let c = self.scanner.peek().unwrap();
            if self.mode == LexerMode::Interp && c == '}' {
                break;
            }

            let mut found = false;
            for tokenizer in tokenizers {
                if let Some( token ) = tokenizer( self, c )? {
                    tokens.push( token );
                    found = true;
                    break;
                }
            }

            if !found {
                self.scanner.push_mark();
                return Err( LexError::unexpected_char( c, self.scanner.pop_span().unwrap() ) );
            }
        }

        self.scanner.push_mark();
        let span = self.scanner.pop_span().unwrap();
        tokens.push( ShellToken {
            kind: ShellTokenKind::EndOfInput,
            span,
        } );

        Ok( tokens )
    }

    fn try_lex_unquoted( &mut self, c: char ) -> Result<Option<ShellToken>, LexError> {
        let special = &self.special;
        if c.is_whitespace() || c.is_control() || special.contains( &c ) {
            return Ok( None );
        }

        self.scanner.push_mark();
        let s = self.scanner.take_while( | c | !c.is_whitespace() && !c.is_control() && !special.contains( &c ) );
        let span = self.scanner.pop_span().unwrap();
        Ok( Some( ShellToken {
            span,
            kind: ShellTokenKind::String( s ),
        } ) )
    }

    fn try_lex_quoted( &mut self, c: char ) -> Result<Option<ShellToken>, LexError> {
        if c != '"' && c != '\'' && c != '`' {
            return Ok( None );
        }

        self.scanner.push_mark();
        let term = self.scanner.consume().unwrap();
        self.scanner.push_mark();
        let mut tokens = Vec::<ShellToken>::new();
        let mut buf = String::new();
        let mut mark = false;
        let mut take = false;
        while !self.scanner.is_empty() && self.scanner.peek().unwrap() != c {
            let c = self.scanner.peek().unwrap();

            if mark {
                self.scanner.push_mark();
                mark = false;
                continue;
            }

            if take {
                buf.push( self.scanner.consume().unwrap() );
                take = false;
                continue;
            }

            match c {
                '\\' => {
                    if self.scanner.peek_ahead( 1 ).map_or( false, | c | c == term ) {
                        self.scanner.consume().unwrap();
                        take = true;
                        continue;
                    } else {
                        take = true;
                        continue;
                    }
                },

                '{' => {
                    let tk = ShellToken {
                        span: self.scanner.pop_span().unwrap(),
                        kind: ShellTokenKind::String( buf.clone() ),
                    };

                    buf.clear();
                    tokens.push( tk );
                    mark = true;

                    let mut interp = self.clone();
                    interp.mode = LexerMode::Interp;
                    interp.scanner.push_mark();
                    interp.scanner.consume().unwrap();

                    let tks = interp.tokenize()?;
                    if interp.scanner.is_empty() || interp.scanner.consume().unwrap() != '}' {
                        return Err( LexError::unexpected_eoi(
                            "string interpolation does not terminate",
                            self.scanner.pop_span().unwrap(),
                        ) );
                    }

                    let tk = ShellToken {
                        span: interp.scanner.pop_span().unwrap(),
                        kind: ShellTokenKind::Interp( tks ),
                    };

                    tokens.push( tk );
                    self.scanner = interp.scanner;
                },

                _ => buf.push( self.scanner.consume().unwrap() ),
            }
        }

        if self.scanner.is_empty() || self.scanner.consume().unwrap() != term {
            return Err( LexError::unexpected_eoi(
                "string does not terminate",
                self.scanner.pop_span().unwrap(),
            ) );
        }

        let is_interp = tokens.len() > 0;
        if is_interp && buf.len() > 0 {
            //self.scanner.push_mark();
            let tk = ShellToken {
                span: self.scanner.pop_span().unwrap(),
                kind: ShellTokenKind::String( buf.clone() ),
            };

            tokens.push( tk );
        }

        Ok( Some( ShellToken {
            span: self.scanner.pop_span().unwrap(),
            kind: if is_interp {
                ShellTokenKind::Interp( tokens )
            } else {
                ShellTokenKind::String( buf.clone() )
            },
        } ) )
    }

    fn try_lex_punct( &mut self, _: char ) -> Result<Option<ShellToken>, LexError> {
        self.scanner.push_mark();
        for ( k, v ) in &self.punct {
            if self.scanner.take_if_next( k ).is_some() {
                let span = self.scanner.pop_span().unwrap();
                return Ok( Some( ShellToken {
                    kind: (*v).clone(),
                    span,
                } ) );
            }
        }

        self.scanner.pop_mark();
        Ok( None )
    }
}

#[derive( Ord, Eq, PartialOrd, PartialEq )]
enum Precedence {
    Invalid = 0,
    Seq = 1,
    Pipe = 2,
    Redir = 3,
    Cmd = 4,
}

pub struct ShellParser {
    tokens: TokenStream<ShellToken>,
    parse_commands: bool,
}

impl ShellParser {
    pub fn new( tokens: Vec<ShellToken> ) -> ShellParser {
        ShellParser {
            tokens: TokenStream::new( tokens ),
            parse_commands: true,
        }
    }

    pub fn parse_all( &mut self ) -> Result<Exec, ParseError> {
        if self.tokens.is_empty() {
            return Ok( Box::new( Empty ) );
        }

        let tree = self.parse( Precedence::Invalid )?;
        self.tokens.consume_a( &ShellTokenKind::EndOfInput )?;

        Ok( tree )
    }

    fn parse( &mut self, prec: Precedence ) -> Result<Exec, ParseError> {
        use ShellTokenKind::*;

        let mut tk = self.tokens.consume()?;

        let mut left: Exec = match tk.kind() {
            String( s ) => self.parse_string( s )?,
            Interp( tks ) => self.parse_interp( tks )?,
            Dollar => {
                if self.tokens.match_a( &LParen ) {
                    self.tokens.consume_a( &LParen )?;
                    let seg = self.with_commands( | p | p.parse( Precedence::Invalid ) )?;
                    self.tokens.consume_a( &RParen )?;

                    Box::new( CmdInterp( seg ) )
                } else {
                    let tk = self.tokens.consume_a( &String( std::string::String::new() ) )?;
                    let name = match tk.kind() {
                        String( s ) => s,
                        _ => unreachable!()
                    };

                    Box::new( Var( name.clone() ) )
                }
            },
            _ => return Err( ParseError::expect_segment(
                tk.to_string(),
                tk.span().clone()
            ) )
        };

        while prec < get_prec( self.tokens.peek() ) {
            tk = self.tokens.consume()?;
            left = match tk.kind() {
                Amp => {
                    let right = self.parse( Precedence::Seq )?;
                    Box::new( Seq {
                        safe: true,
                        left: left,
                        right: right,
                    } )
                },
                Semi => {
                    let right = self.parse( Precedence::Seq )?;
                    Box::new( Seq {
                        safe: false,
                        left: left.into(),
                        right: right.into(),
                    } )
                },
                Pipe => {
                    let right = self.parse( Precedence::Pipe )?;
                    Box::new( super::segments::Pipe {
                        left: left.into(),
                        right: right.into(),
                    } )
                },
                StdIn => self.parse_redirect( left, tk )?,
                StdOut => self.parse_redirect( left, tk )?,
                StdErr => self.parse_redirect( left, tk )?,
                StdBoth => self.parse_redirect( left, tk )?,

                _ => unreachable!(),
            };
        }

        return Ok( left );

        fn get_prec( tk: Option<&ShellToken> ) -> Precedence {
            use self::Precedence::*;
            if let Some( tk ) = tk {
                match tk.kind() {
                    Amp => Seq,
                    Semi => Seq,
                    ShellTokenKind::Pipe => Pipe,
                    StdIn => Redir,
                    StdOut => Redir,
                    StdErr => Redir,
                    StdBoth => Redir,

                    _ => Invalid,
                }
            } else {
                Invalid
            }
        }
    }

    fn with_commands<F>( &mut self, f: F ) -> Result<Exec, ParseError>
        where F: FnOnce( &mut ShellParser ) -> Result<Exec, ParseError>
    {
        let orig = self.parse_commands;
        self.parse_commands = true;
        let res = f( self );
        self.parse_commands = orig;

        res
    }

    fn without_commands<F>( &mut self, f: F ) -> Result<Exec, ParseError>
        where F: FnOnce( &mut ShellParser ) -> Result<Exec, ParseError>
    {
        let orig = self.parse_commands;
        self.parse_commands = false;
        let res = f( self );
        self.parse_commands = orig;

        res
    }

    fn has_segment( &mut self ) -> bool {
        match self.tokens.peek() {
            Some( x ) => {
                let x = discriminant( x.kind() );
                x == discriminant( &ShellTokenKind::String( std::string::String::new() ) ) ||
                x == discriminant( &ShellTokenKind::Interp( Vec::new() ) ) ||
                x == discriminant( &ShellTokenKind::Dollar )
            },

            None => false
        }
    }

    fn parse_string( &mut self, s: &String ) -> Result<Exec, ParseError> {
        let seg = Box::new( Text( s.clone() ) );

        if !self.parse_commands {
            Ok( seg )
        } else {
            self.parse_args( seg )
        }
    }

    fn parse_interp( &mut self, tks: &Vec<ShellToken> ) -> Result<Exec, ParseError> {
        let mut segs = Vec::new();
        for tk in tks {
            let seg: Exec = match tk.kind() {
                ShellTokenKind::String( s ) => Box::new( Text( s.clone() ) ),
                ShellTokenKind::Interp( tks ) => {
                    let mut parser = ShellParser::new( tks.clone() );
                    parser.parse_all()?
                },
                _ => unreachable!(),
            };

            segs.push( seg );
        }

        let seg = Box::new( TextInterp( segs ) );

        if !self.parse_commands {
            Ok( seg )
        } else {
            self.parse_args( seg )
        }
    }

    fn parse_args( &mut self, seg: Exec ) -> Result<Exec, ParseError> {
        let mut segs = Vec::new();
        while self.has_segment() {
            let seg = self.without_commands( | p | p.parse( Precedence::Cmd ) )?;
            segs.push( seg );
        }

        if segs.len() == 0 {
            Ok( Box::new( Cmd {
                command: seg,
                args: None,
            } ) )
        } else {
            Ok( Box::new( Cmd {
                command: seg,
                args: Some( segs ),
            } ) )
        }
    }

    fn parse_redirect( &mut self, left: Exec, tk: ShellToken ) -> Result<Exec, ParseError> {
        let span = match self.tokens.peek() {
            Some( tk ) => tk.span().clone(),
            None => tk.span.clone()
        };

        let right = self.without_commands( | p | p.parse( Precedence::Redir ) )?;
        let is_valid =
            right.as_any().downcast_ref::<Text>().is_some() ||
            right.as_any().downcast_ref::<Redirect>().is_some();

        if !is_valid {
            return Err( ParseError::expect_string( span ) )
        }

        let mode = match tk.kind() {
            ShellTokenKind::StdIn => RedirectMode::StdIn,
            ShellTokenKind::StdOut => RedirectMode::StdOut,
            ShellTokenKind::StdErr => RedirectMode::StdErr,
            ShellTokenKind::StdBoth => RedirectMode::StdBoth,
            _ => unreachable!(),
        };

        Ok( Box::new( Redirect { left, right, mode } ) )
    }
}
