//! Where the settings and the log live.
//!
//! - Settings: `%APPDATA%\thumbring\config.toml` (follows the user profile).
//! - Log: `%LOCALAPPDATA%\thumbring\engine.log` (stays on this machine).
//!
//! `--dir <folder>` puts both in one folder. Useful for testing without
//! touching the real settings.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use thumbring_core::config::DEFAULT_FILE;

const APP_DIR: &str = "thumbring";

pub struct Paths {
    pub config_file: PathBuf,
    pub log_file: PathBuf,
    /// The folder given with `--dir`, absolute, which the settings window
    /// gets too.
    pub dir: Option<PathBuf>,
}

impl Paths {
    pub fn from_args(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut dir = None;
        while let Some(arg) = args.next() {
            if arg == "--dir" {
                let value = args.next().ok_or("a folder is missing after --dir")?;
                // Absolute, because the settings window starts elsewhere.
                let value = std::path::absolute(&value)
                    .map_err(|e| format!("the folder after --dir is not valid: {e}"))?;
                dir = Some(value);
            } else {
                return Err(format!(
                    "unknown argument: {} (the only one accepted is --dir <folder>)",
                    arg.to_string_lossy()
                ));
            }
        }
        if let Some(dir) = dir {
            return Ok(Self {
                config_file: dir.join("config.toml"),
                log_file: dir.join("engine.log"),
                dir: Some(dir),
            });
        }
        Ok(Self {
            config_file: known_dir("APPDATA")?.join(APP_DIR).join("config.toml"),
            log_file: known_dir("LOCALAPPDATA")?.join(APP_DIR).join("engine.log"),
            dir: None,
        })
    }
}

fn known_dir(var: &str) -> Result<PathBuf, String> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("the environment variable {var} does not exist"))
}

/// Creates the settings file with the commented defaults if it does not exist
/// yet. Returns `true` if it created the file.
pub fn ensure_config(path: &Path) -> io::Result<bool> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // `create_new` fails if the file exists, without the race of checking first.
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            // Windows line endings, so any editor opens it well.
            let text = DEFAULT_FILE.replace("\r\n", "\n").replace('\n', "\r\n");
            file.write_all(text.as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}
