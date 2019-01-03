use yansi::{ Style, Paint, Color };
use std::default::Default;

use kernel::{ ColorSupport::{ Colors256, TrueColor }, get_color_support };

#[derive( Debug )]
pub enum ColorSpace<D> {
    Notice( D ),
    Warning( D ),
    Error( D ),
    Dir( D ),
    User( D ),
    Machine( D ),
}

impl<D> ColorSpace<D> {
    pub fn unwrap( self ) -> D {
        match self {
            ColorSpace::Notice( x ) => x,
            ColorSpace::Warning( x ) => x,
            ColorSpace::Error( x ) => x,
            ColorSpace::Dir( x ) => x,
            ColorSpace::User( x ) => x,
            ColorSpace::Machine( x ) => x,
        }
    }
}

#[derive( Debug )]
pub struct ColorPalette {
    notice: Style,
    warning: Style,
    error: Style,
    dir: Style,
    user: Style,
    machine: Style,
}

impl ColorPalette {
    #[cfg( windows )]
    pub fn enable_windows_ascii() -> bool {
        let support = get_color_support();
        ( support == Colors256 || support == TrueColor ) && Paint::<()>::enable_windows_ascii()
    }

    pub fn paint<D>( &self, value: ColorSpace<D> ) -> Paint<D> {
        return match value {
            ColorSpace::Notice( x ) => paint( x, self.notice ),
            ColorSpace::Warning( x ) => paint( x, self.warning ),
            ColorSpace::Error( x ) => paint( x, self.error ),
            ColorSpace::Dir( x ) => paint( x, self.dir ),
            ColorSpace::User( x ) => paint( x, self.user ),
            ColorSpace::Machine( x ) => paint( x, self.machine ),
        };

        fn paint<D>( x: D, s: Style ) -> Paint<D> {
            Paint::new( x ).with_style( s )
        }
    }
}

impl Default for ColorPalette {
    fn default() -> Self {
        // TODO: implement colour support testing for linux
        //       also implement a 256 colour variation

        let rgb = || {
            ColorPalette {
                notice: Style::new( Color::RGB( 29, 136, 241 ) ),
                warning: Style::new( Color::RGB( 249, 184, 22 ) ),
                error: Style::new( Color::RGB( 255, 67, 131 ) ),
                dir: Style::new( Color::RGB( 248, 176, 104 ) ),
                user: Style::new( Color::RGB( 80, 177, 255 ) ),
                machine: Style::new( Color::RGB( 255, 0, 255 ) ),
            }
        };

        let simple = || {
            ColorPalette {
                notice: Style::new( Color::Cyan ),
                warning: Style::new( Color::Yellow ),
                error: Style::new( Color::Red ),
                dir: Style::new( Color::Cyan ).dimmed(),
                user: Style::new( Color::Green ),
                machine: Style::new( Color::Yellow ).dimmed(),
            }
        };

        if cfg!( windows ) {
            if ColorPalette::enable_windows_ascii() {
                rgb()
            } else {
                simple()
            }
        } else {
            simple()
        }
    }
}

#[derive( Debug )]
pub enum PromptStyle {
    Lumi,
    Linux,
    Windows,
}

#[derive( Debug )]
pub struct Config {
    colors_enabled: bool,
    colors: ColorPalette,
    prompt: PromptStyle,
}

impl Config {
    pub fn prompt( &self ) -> &PromptStyle {
        &self.prompt
    }

    pub fn palette( &self ) -> Option<&ColorPalette> {
        if self.colors_enabled {
            Some( &self.colors )
        } else {
            None
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            colors_enabled: true,
            colors: ColorPalette::default(),
            prompt: PromptStyle::Lumi,
        }
    }
}
