#[cfg( windows )]
pub mod windows;

#[cfg( not( windows ) )]
pub mod linux;

pub mod common;

#[cfg( windows )]
pub use self::windows::*;

#[cfg( not( windows ) )]
pub use self::linux::*;

pub use self::common::*;
