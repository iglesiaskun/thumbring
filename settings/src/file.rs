//! Where the settings file is, and reading and writing it safely.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const APP_DIR: &str = "thumbring";

/// Windows error codes that mean "someone has the file open right now"
/// (other systems let files be replaced while open).
const ACCESS_DENIED: i32 = 5;
const SHARING_VIOLATION: i32 = 32;

/// The settings file, as the engine finds it: `config.toml` in the
/// [`config_dir`], or in `<folder>` with `--dir <folder>`, the engine's
/// testing option.
pub fn config_path(args: impl IntoIterator<Item = OsString>) -> Result<PathBuf, String> {
    let mut args = args.into_iter();
    let mut dir = None;
    while let Some(arg) = args.next() {
        if arg == "--dir" {
            let value = args.next().ok_or("a folder is missing after --dir")?;
            dir = Some(PathBuf::from(value));
        } else {
            return Err(format!(
                "unknown argument: {} (the only one accepted is --dir <folder>)",
                arg.to_string_lossy()
            ));
        }
    }
    let dir = match dir {
        Some(dir) => dir,
        None => config_dir()?,
    };
    Ok(dir.join("config.toml"))
}

/// Folder for the window's own files (the web view's cache), next to the
/// engine's log, or inside the `--dir` folder.
pub fn window_data_dir(config_path: &Path, testing: bool) -> Result<PathBuf, String> {
    let base = match (testing, config_path.parent()) {
        (true, Some(dir)) => dir.to_path_buf(),
        _ => local_dir()?,
    };
    Ok(base.join("settings-window"))
}

/// Where the settings live: `%APPDATA%\thumbring` on Windows, which follows
/// the user profile. The engine runs only on Windows so far; the other
/// systems get their usual places.
fn config_dir() -> Result<PathBuf, String> {
    if cfg!(windows) {
        Ok(known_dir("APPDATA")?.join(APP_DIR))
    } else if cfg!(target_os = "macos") {
        Ok(known_dir("HOME")?
            .join("Library/Application Support")
            .join(APP_DIR))
    } else {
        Ok(xdg_dir("XDG_CONFIG_HOME", ".config")?.join(APP_DIR))
    }
}

/// Files that stay on this machine: `%LOCALAPPDATA%\thumbring` on Windows,
/// where the engine keeps its log.
fn local_dir() -> Result<PathBuf, String> {
    if cfg!(windows) {
        Ok(known_dir("LOCALAPPDATA")?.join(APP_DIR))
    } else if cfg!(target_os = "macos") {
        Ok(known_dir("HOME")?.join("Library/Caches").join(APP_DIR))
    } else {
        Ok(xdg_dir("XDG_CACHE_HOME", ".cache")?.join(APP_DIR))
    }
}

/// An XDG folder, or its usual place in the home folder.
fn xdg_dir(var: &str, fallback: &str) -> Result<PathBuf, String> {
    known_dir(var).or_else(|_| Ok(known_dir("HOME")?.join(fallback)))
}

fn known_dir(var: &str) -> Result<PathBuf, String> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("the environment variable {var} does not exist"))
}

/// The file's bytes, or `None` if it does not exist yet. The engine (or a
/// text editor) may be writing it at that very moment: tries again a few
/// times.
pub fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match retry(|| fs::read(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Replaces the file in one step: writes a temporary file next to it and
/// renames it over the old one. The engine reloads the file when it changes
/// and must never read it half written.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("the settings file has no folder"))?;
    fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("the settings file has no name"))?;
    let mut temp_name = name.to_os_string();
    temp_name.push(format!(".{}.tmp", std::process::id()));
    let temp = dir.join(temp_name);

    let written = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(bytes)?;
        // On the disk before the rename, so a crash leaves the old file or
        // the new one, never an empty one.
        file.sync_all()
    })();
    // On Windows, `rename` replaces the old file.
    let result = written.and_then(|()| retry(|| fs::rename(&temp, path)));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn retry<T>(mut f: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut attempts = 0;
    loop {
        match f() {
            Err(e)
                if cfg!(windows)
                    && matches!(e.raw_os_error(), Some(ACCESS_DENIED | SHARING_VIOLATION))
                    && attempts < 10 =>
            {
                attempts += 1;
                thread::sleep(Duration::from_millis(50));
            }
            result => return result,
        }
    }
}
