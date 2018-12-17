use std::os::unix::process::ExitStatusExt;

pub fn clear_screen() {
    print!( "\x1B[2J\x1B[H" );
}

pub unsafe fn disable_ctrl_c() {
    // TODO
}

pub fn get_exit_code( status: ExitStatus ) -> Option<i32> {
    match status.code() {
        Some( x ) => Some( x ),
        None => status.signal()
    }
}
