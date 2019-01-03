extern crate yansi;
extern crate whoami;
extern crate dirs;
extern crate atty;
extern crate crossterm;

#[macro_use]
extern crate failure;

#[cfg( windows )]
extern crate winapi;

#[cfg( windows )]
extern crate winreg;

mod peek;
mod parsing;
mod shell;
mod kernel;
mod empty;

use shell::repl::Repl;
use shell::config::Config;

use shell::parsing::*;

fn main() {
    let config = Config::default();
    Repl::new( &config ).run();
}
