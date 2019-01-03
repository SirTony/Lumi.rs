use winapi::um::wincon::{
    COORD,
    SMALL_RECT,
    CONSOLE_SCREEN_BUFFER_INFO,
    GetConsoleScreenBufferInfo,
    FillConsoleOutputCharacterA,
    FillConsoleOutputAttribute,
    SetConsoleCursorPosition
};

use atty::Stream;
use winreg::{ RegKey, enums::* };
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winbase::STD_OUTPUT_HANDLE;
use winapi::um::processenv::GetStdHandle;
use winapi::shared::minwindef::{ DWORD, TRUE };
use winapi::um::consoleapi::SetConsoleCtrlHandler;
use std::process::ExitStatus;
use empty::Empty;
use kernel::common::{ ColorSupport, ColorSupport::* };

pub unsafe fn clear_screen() {
    let zero = COORD::empty();
    let mut buf = CONSOLE_SCREEN_BUFFER_INFO::empty();
    let handle = GetStdHandle( STD_OUTPUT_HANDLE );
    if handle == INVALID_HANDLE_VALUE { return; }

    if GetConsoleScreenBufferInfo( handle, &mut buf ) == 0 { return; }

    let count = ( buf.dwSize.X as u32 ) * ( buf.dwSize.Y as u32 );
    let mut written: DWORD = 0;

    if FillConsoleOutputCharacterA( handle, 0x20, count, zero, &mut written ) == 0 { return; }
    if FillConsoleOutputAttribute( handle, buf.wAttributes, count, zero, &mut written ) == 0 { return; }

    SetConsoleCursorPosition( handle, zero );
}

pub unsafe fn disable_ctrl_c() {
    SetConsoleCtrlHandler( Option::None, TRUE );
}

pub fn get_exit_code( status: ExitStatus ) -> Option<i32> {
    status.code()
}

pub fn get_color_support() -> ColorSupport {
    return if !atty::is( Stream::Stdout ) {
        None
    } else {
        try_get_support().unwrap_or( Default )
    };

    fn try_get_support() -> std::io::Result<ColorSupport> {
        // we have to fiddle with the registry because the winapi GetVersion()
        // is deprecated since win 8.1 and there's no suitable replacement.
        let hklm = RegKey::predef( HKEY_LOCAL_MACHINE );
        let winnt = hklm.open_subkey( "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion" )?;
        let major: u32 = winnt.get_value( "CurrentMajorVersionNumber" )?;

        if major < 10 {
            Ok( Default )
        } else {
            let build_str: String = winnt.get_value( "CurrentBuildNumber" )?;
            let build = build_str.parse::<u32>().unwrap_or( 0 );

            if build >= 14931 {
                Ok( TrueColor )
            } else if build >= 10586 {
                Ok( Colors256 )
            } else {
                Ok( Default )
            }
        }
    }
}

impl Empty for CONSOLE_SCREEN_BUFFER_INFO {
    fn empty() -> Self {
        CONSOLE_SCREEN_BUFFER_INFO {
            dwSize: COORD::empty(),
            dwCursorPosition: COORD::empty(),
            wAttributes: 0,
            srWindow: SMALL_RECT::empty(),
            dwMaximumWindowSize: COORD::empty(),
        }
    }
}

impl Empty for COORD {
    fn empty() -> Self {
        COORD { X: 0, Y: 0 }
    }
}

impl Empty for SMALL_RECT {
    fn empty() -> Self {
        SMALL_RECT { Top: 0, Right: 0, Bottom: 0, Left: 0 }
    }
}
