//! Log to a file and, when the engine runs in a console, to the screen too.
//!
//! On every start the previous log becomes `engine.log.1`, so the last run can
//! still be inspected without the file growing forever.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::WriteFile;
use windows::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE};
use windows::Win32::System::SystemInformation::GetLocalTime;

static FILE: Mutex<Option<File>> = Mutex::new(None);
/// The handle of the file in `FILE`, for [`write_unlocked`]. The file is
/// never closed: `init` runs once and the file stays until the process ends.
static RAW_FILE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

#[derive(Debug, Clone, Copy)]
pub enum Level {
    Info,
    Warn,
    Error,
}

/// Opens the log file. Before that (or if it fails), the log only goes to the
/// console.
pub fn init(path: &Path) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // `rename` replaces the target if it already exists.
    let _ = fs::rename(path, path.with_extension("log.1"));
    // Append mode: every write goes to the end of the file in one piece, so
    // `write_unlocked` cannot cut a line of `write` in half.
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    RAW_FILE.store(file.as_raw_handle(), Ordering::SeqCst);
    *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
    Ok(())
}

pub fn write(level: Level, args: fmt::Arguments) {
    let mut line = String::new();
    let _ = format_line(&mut line, level, args);
    line.push('\n');
    // Without a console (release build), writing to stderr does nothing.
    let _ = io::stderr().write_all(line.as_bytes());
    if let Some(file) = FILE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Like [`write`], but without locks or the heap, for when the process is
/// crashing: the thread that crashed may hold the lock of [`write`] or the
/// heap's. A line longer than [`StackLine`] holds is cut short.
pub fn write_unlocked(level: Level, args: fmt::Arguments) {
    let mut line = StackLine::new();
    let _ = format_line(&mut line, level, args);
    let line = line.finish();
    let mut written = 0;
    // SAFETY: plain writes of local buffers. A missing or closed handle just
    // returns an error.
    unsafe {
        if let Ok(stderr) = GetStdHandle(STD_ERROR_HANDLE) {
            let _ = WriteFile(stderr, Some(line), Some(&mut written), None);
        }
        let file = RAW_FILE.load(Ordering::SeqCst);
        if !file.is_null() {
            let _ = WriteFile(HANDLE(file), Some(line), Some(&mut written), None);
        }
    }
}

/// The time and the level, then the message, without the line break.
fn format_line(out: &mut impl fmt::Write, level: Level, args: fmt::Arguments) -> fmt::Result {
    // SAFETY: only reads the system clock.
    let t = unsafe { GetLocalTime() };
    let level = match level {
        Level::Info => "info ",
        Level::Warn => "warn ",
        Level::Error => "error",
    };
    write!(
        out,
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {level} {args}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

/// A log line built on the stack. The line ends where the text stops fitting,
/// but the line break always fits.
struct StackLine {
    bytes: [u8; 512],
    len: usize,
}

impl StackLine {
    fn new() -> Self {
        Self {
            bytes: [0; 512],
            len: 0,
        }
    }

    /// The line with its line break.
    fn finish(&mut self) -> &[u8] {
        self.bytes[self.len] = b'\n';
        &self.bytes[..=self.len]
    }
}

impl fmt::Write for StackLine {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // One byte stays free for the line break.
        let room = self.bytes.len() - 1 - self.len;
        let mut n = s.len().min(room);
        // Never half a character.
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        self.bytes[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        // The error stops the formatting, so nothing after the cut gets in.
        if n < s.len() { Err(fmt::Error) } else { Ok(()) }
    }
}

macro_rules! info {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Info, format_args!($($arg)*)) };
}

// Not `warn`: that name clashes with the built-in `#[warn]` attribute.
macro_rules! warning {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Warn, format_args!($($arg)*)) };
}

macro_rules! error {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Error, format_args!($($arg)*)) };
}

pub(crate) use {error, info, warning};

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    #[test]
    fn a_stack_line_that_does_not_fit_is_cut_between_characters() {
        let mut line = StackLine::new();
        // 510 bytes, then a character of two bytes with room for only one.
        // The space after it would fit, but the line already ended.
        let cut = write!(line, "{}é and more", "x".repeat(510));
        assert!(cut.is_err());
        let bytes = line.finish();
        assert_eq!(bytes.len(), 511);
        assert!(bytes.ends_with(b"x\n"));
        assert!(std::str::from_utf8(bytes).is_ok());
    }

    #[test]
    fn a_short_stack_line_is_whole() {
        let mut line = StackLine::new();
        let name = "ring";
        write!(line, "crash in the {name} thread").unwrap();
        assert_eq!(line.finish(), b"crash in the ring thread\n");
    }
}
