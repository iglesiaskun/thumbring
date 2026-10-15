//! What happens on a bug (a panic): the engine writes it to the log, gives
//! the mouse back, says so and quits.
//!
//! Without this, a panic would end only the thread where it happened. The
//! others would keep running: the hook could keep taking the Back button with
//! no worker left to act on it, and the release build would show nothing.

use std::path::PathBuf;
use std::thread;

use windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR;

use crate::log::error;
use crate::{hook, text, tray};

/// The usual exit code of a Rust program that panicked.
const EXIT_CODE: i32 = 101;

/// Replaces the default panic handler. `log_file` goes in the message, so the
/// person knows where to look.
pub fn install(log_file: PathBuf) {
    std::panic::set_hook(Box::new(move |info| {
        let current = thread::current();
        let name = current.name().unwrap_or("unnamed");
        // `info` has the message and the file and line.
        error!("bug in the {name} thread, the engine is stopping: {info}");
        hook::give_up();
        tray::remove_now();
        crate::alert(&text::alert::crashed(&log_file), MB_ICONERROR);
        // Ends every thread at once; Windows removes whatever hook is left.
        std::process::exit(EXIT_CODE);
    }));
}
