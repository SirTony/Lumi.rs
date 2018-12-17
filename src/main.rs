extern crate yansi;
extern crate whoami;
extern crate dirs;

#[cfg( windows )]
extern crate winapi;

mod shell;
mod kernel;
mod empty;

use shell::repl::Repl;
use shell::config::Config;

fn main() {
    let config = Config::default();
    Repl::new( &config ).run();
}
