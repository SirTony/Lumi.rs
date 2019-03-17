#[cfg( windows )]
pub mod windows;

#[cfg( not( windows ) )]
pub mod linux;

#[cfg( windows )]
pub use self::windows::*;

#[cfg( not( windows ) )]
pub use self::linux::*;
