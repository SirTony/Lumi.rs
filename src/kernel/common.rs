// Gets terminal's colour depth.
#[derive( PartialEq, Eq, Debug )]
pub enum ColorSupport {
    // Terminal does not support colour. Used if STDOUT is not a TTY.
    None,

    // Terminal has basic 8/16-color support.
    Default,

    // Terminal supports 256 colours.
    Colors256,

    // Terminal supports full 24-bit RGB colours.
    TrueColor,
}
