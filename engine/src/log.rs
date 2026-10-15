//! Log to a file and, when the engine runs in a console, to the screen too.
//!
//! On every start the previous log becomes `engine.log.1`, so the last run can
//! still be inspected without the file growing forever.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;

use windows::Win32::System::SystemInformation::GetLocalTime;

static FILE: Mutex<Option<File>> = Mutex::new(None);

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
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
    Ok(())
}

pub fn write(level: Level, args: fmt::Arguments) {
    // SAFETY: only reads the system clock.
    let t = unsafe { GetLocalTime() };
    let level = match level {
        Level::Info => "info ",
        Level::Warn => "warn ",
        Level::Error => "error",
    };
    let line = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {level} {args}\n",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    );
    // Without a console (release build), writing to stderr does nothing.
    let _ = io::stderr().write_all(line.as_bytes());
    if let Some(file) = FILE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let _ = file.write_all(line.as_bytes());
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
