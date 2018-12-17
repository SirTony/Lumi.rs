#[cfg( windows )]
pub mod windows;

#[cfg( not( windows ) )]
pub mod linux;

#[cfg( windows )]
pub use self::windows::{
    clear_screen,
    disable_ctrl_c,
    get_exit_code
};

#[cfg( not( windows ) )]
pub use self::linux::{
    clear_screen,
    disable_ctrl_c,
    get_exit_code
};
