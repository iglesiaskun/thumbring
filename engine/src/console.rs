//! In a console (development build), Ctrl+C, Ctrl+Break and closing the
//! console window shut the engine down cleanly: the hook goes away and the
//! tray icon disappears.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
    GetStdHandle, STD_ERROR_HANDLE, SetConsoleCtrlHandler,
};
use windows::core::BOOL;

use crate::tray;

/// Set when the cleanup in `main` is done.
static STOPPED: AtomicBool = AtomicBool::new(false);

/// Is there somewhere to write messages (a console or a redirected file)?
/// Without it (release build started from Explorer), only a window can tell.
pub fn has_stderr() -> bool {
    // SAFETY: query only.
    matches!(unsafe { GetStdHandle(STD_ERROR_HANDLE) }, Ok(h) if !h.is_invalid() && !h.0.is_null())
}

pub fn quit_on_ctrl_c() -> windows::core::Result<()> {
    // SAFETY: `handler` is an `extern "system"` function that lives for the whole program.
    unsafe { SetConsoleCtrlHandler(Some(handler), true) }
}

/// Tells the handler that the cleanup is done.
pub fn mark_stopped() {
    STOPPED.store(true, Ordering::SeqCst);
}

/// Runs on a thread created by Windows. Returning `true` says the event was
/// handled, so Windows does not end the process on its own.
unsafe extern "system" fn handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        CTRL_C_EVENT | CTRL_BREAK_EVENT => {
            tray::request_quit();
            true.into()
        }
        CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => {
            // Here Windows ends the process as soon as this function
            // returns. Wait a little for the cleanup to finish.
            tray::request_quit();
            let deadline = Instant::now() + Duration::from_secs(2);
            while !STOPPED.load(Ordering::SeqCst) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            true.into()
        }
        _ => false.into(),
    }
}
