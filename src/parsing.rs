use peek::BufferedPeekable;
use std::mem::discriminant;
use std::fmt::{ Display, Formatter, Debug };

pub trait SyntaxToken {
    type Kind: ToString;

    fn kind( &self ) -> &Self::Kind;
    fn span( &self ) -> &TextSpan;
}

#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct Location {
    pub index: usize,
    pub line: usize,
    pub column: usize,
}

#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct TextSpan {
    pub start: Location,
    pub end: Location,
}

impl TextSpan {
    pub fn length( &self ) -> usize {
        self.end.index - self.start.index
    }
}

impl Display for TextSpan {
    fn fmt( &self, formatter: &mut Formatter<'_> ) -> std::fmt::Result {
        formatter.write_fmt( format_args!( "line {0}, column {1}", self.start.line, self.start.column ) )
    }
}

#[derive( Debug )]
pub enum ParseErrorKind {
    UnexpectedEOI,
    Unexpected {
        expect: String,
        found: String,
    },
    ExpectSegment { found: String },
    ExpectString,
}

#[derive( Debug )]
pub struct ParseError {
    kind: ParseErrorKind,
    span: Option<TextSpan>,
}

impl ParseError {
    pub fn new( kind: ParseErrorKind, span: Option<TextSpan> ) -> ParseError {
        ParseError { kind, span }
    }

    pub fn kind( &self ) -> &ParseErrorKind {
        &self.kind
    }

    pub fn span( &self ) -> Option<&TextSpan> {
        match self.span {
            Some( ref x ) => Some( x ),
            None => None,
        }
    }

    pub fn unexpected_eoi() -> ParseError {
        ParseError::new( ParseErrorKind::UnexpectedEOI, None )
    }

    pub fn unexpected( expect: String, found: String, span: TextSpan ) -> ParseError {
        ParseError::new(
            ParseErrorKind::Unexpected {
                expect,
                found,
            },
            Some( span )
        )
    }

    pub fn expect_segment( found: String, span: TextSpan ) -> ParseError {
        ParseError::new(
            ParseErrorKind::ExpectSegment { found },
            Some( span )
        )
    }

    pub fn expect_string( span: TextSpan ) -> ParseError {
        ParseError::new(
            ParseErrorKind::ExpectString,
            Some( span )
        )
    }
}

pub struct TokenStream<T> {
    tokens: BufferedPeekable<T>
}

impl<T: SyntaxToken + ToString + Debug> TokenStream<T>
{
    pub fn new( tokens: Vec<T> ) -> TokenStream<T> {
        TokenStream {
            tokens: BufferedPeekable::new( tokens )
        }
    }

    pub fn is_empty( &mut self ) -> bool {
        self.tokens.is_empty()
    }

    pub fn peek( &mut self ) -> Option<&T> {
        self.tokens.peek()
    }

    pub fn peek_ahead( &mut self, distance: usize ) -> Option<&T> {
        self.tokens.peek_ahead( distance )
    }

    pub fn match_a( &mut self, what: &T::Kind ) -> bool {
        if let Some( tk ) = self.peek() {
            if discriminant( tk.kind() ) == discriminant( what ) {
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn consume( &mut self ) -> Result<T, ParseError> {
        match self.tokens.consume() {
            Some( x ) => Ok( x ),
            None => Err( ParseError::unexpected_eoi() )
        }
    }

    pub fn consume_a( &mut self, what: &T::Kind ) -> Result<T, ParseError> {
        let tk = self.consume()?;
        if discriminant( tk.kind() ) == discriminant( what ) {
            Ok( tk )
        } else {
            Err( ParseError::unexpected(
                what.to_string(),
                tk.to_string(),
                tk.span().clone()
            ) )
        }
    }
}

#[derive( Debug )]
pub enum LexErrorKind {
    UnexpectedChar {
        character: char,
        codepoint: u16,
    },
    UnexpectedEOI {
        reason: &'static str,
    },
}

#[derive( Debug )]
pub struct LexError {
    kind: LexErrorKind,
    span: TextSpan,
}

impl LexError {
    pub fn new( kind: LexErrorKind, span: TextSpan ) -> LexError {
        LexError { kind, span }
    }

    pub fn kind( &self ) -> &LexErrorKind {
        &self.kind
    }

    pub fn span( &self ) -> &TextSpan {
        &self.span
    }

    pub fn unexpected_char( c: char, span: TextSpan ) -> LexError {
        LexError::new(
            LexErrorKind::UnexpectedChar {
                character: c,
                codepoint: c as u16,
            },
            span
        )
    }

    pub fn unexpected_eoi( reason: &'static str, span: TextSpan ) -> LexError {
        LexError::new(
            LexErrorKind::UnexpectedEOI { reason },
            span
        )
    }
}

#[derive( Clone )]
pub struct Scanner {
    iter: BufferedPeekable<char>,
    markers: Vec<Location>,
    index: usize,
    line: usize,
    column: usize,
}

impl Scanner {
    pub fn new( source: String, index: usize, line: usize, column: usize ) -> Scanner {
        Scanner {
            iter: BufferedPeekable::new( source.chars().collect() ),
            markers: Vec::new(),
            index,
            line,
            column,
        }
    }

    pub fn current_pos( &self ) -> Location {
        Location {
            index: self.index,
            line: self.line,
            column: self.column,
        }
    }

    pub fn push_mark( &mut self ) {
        let here = self.current_pos();
        self.markers.push( here );
    }

    pub fn pop_mark( &mut self ) -> Option<Location> {
        self.markers.pop()
    }

    pub fn pop_span( &mut self ) -> Option<TextSpan> {
        let start = self.pop_mark()?;
        let end = self.current_pos();

        Some( TextSpan {
            start: start,
            end: end
        } )
    }

    pub fn is_empty( &mut self ) -> bool {
        self.iter.is_empty()
    }

    pub fn peek( &mut self ) -> Option<char> {
        Some( *self.iter.peek()? )
    }

    pub fn peek_ahead( &mut self, distance: usize ) -> Option<char> {
        Some( *self.iter.peek_ahead( distance )? )
    }

    pub fn consume( &mut self ) -> Option<char> {
        let c = self.iter.consume()?;

        if c == '\n' {
            self.line += 1;
            self.column = 0;
        }

        self.index += 1;
        self.column += 1;

        Some( c )
    }

    pub fn is_next( &mut self, s: &str ) -> bool {
        for ( i, c ) in s.chars().enumerate() {
            let found = match self.peek_ahead( i ) {
                Some( x ) if x == c => true,
                _ => false,
            };

            if !found { return false; }
        }

        true
    }

    pub fn take_if_next( &mut self, s: &str ) -> Option<String> {
        if self.is_next( s ) {
            let mut buf = String::new();
            for _ in 0 .. s.len() {
                buf.push( self.consume()? )
            }

            Some( buf )
        } else {
            None
        }
    }

    pub fn skip_while<F: Fn( char ) -> bool>( &mut self, f: F ) {
        while !self.is_empty() && f( self.peek().unwrap() ) {
            self.consume();
        }
    }

    pub fn take_while<F: Fn( char ) -> bool>( &mut self, f: F ) -> String {
        let mut s = String::new();
        while !self.is_empty() && f( self.peek().unwrap() ) {
            s.push( self.consume().unwrap() );
        }

        s
    }
}
