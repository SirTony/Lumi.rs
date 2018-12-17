use yansi::{ Paint, Color };
use std::default::Default;

use kernel::clear_screen;

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
    notice: Color,
    warning: Color,
    error: Color,
    dir: Color,
    user: Color,
    machine: Color,
}

impl ColorPalette {
    pub fn apply( &self ) {
        Paint::<()>::enable_windows_ascii();

        unsafe{ clear_screen() }
    }

    pub fn paint<D>( &self, value: ColorSpace<D> ) -> Paint<D> {
        return match value {
            ColorSpace::Notice( x ) => paint( x, &self, self.notice ),
            ColorSpace::Warning( x ) => paint( x, &self, self.warning ),
            ColorSpace::Error( x ) => paint( x, &self, self.error ),
            ColorSpace::Dir( x ) => paint( x, &self, self.dir ),
            ColorSpace::User( x ) => paint( x, &self, self.user ),
            ColorSpace::Machine( x ) => paint( x, &self, self.machine ),
        };

        fn paint<D>( x: D, p: &ColorPalette, c: Color ) -> Paint<D> {
            Paint::default( x ).fg( c ).bg( Color::Default )
        }
    }
}

impl Default for ColorPalette {
    fn default() -> Self {
        return ColorPalette {
            notice: Color::RGB( 29, 136, 241 ),
            warning: Color::RGB( 249, 184, 22 ),
            error: Color::RGB( 255, 67, 131 ),
            dir: Color::RGB( 248, 176, 104 ),
            user: Color::RGB( 80, 177, 255 ),
            machine: Color::RGB( 255, 0, 255 ),
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
