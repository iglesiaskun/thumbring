//! Names that let the engine and the settings window find each other.
//!
//! Each program holds a named object (a mutex, on Windows) while it runs. The
//! settings window asks whether an engine uses the same settings file, to tell
//! whether a save is already in effect, and the engine asks whether the
//! settings window is open, to leave the "saved" message to it.

use std::path::Path;

/// Held by the engine: only one engine runs in a session.
pub const ENGINE: &str = r"Local\Thumbring.Engine";

/// Held by the engine for the settings file it reads.
pub fn engine_for(config_file: &Path) -> String {
    format!(r"{ENGINE}.{}", file_key(config_file))
}

/// Held by the settings window for the settings file it edits.
pub fn settings_for(config_file: &Path) -> String {
    format!(r"Local\Thumbring.Settings.{}", file_key(config_file))
}

/// A short key for a settings file. The path must be absolute. Windows paths
/// ignore case and take either slash, so `C:\Users\Me\config.toml` and
/// `c:/users/me/config.toml` give the same key.
pub fn file_key(config_file: &Path) -> String {
    let path = config_file
        .to_string_lossy()
        .replace('/', r"\")
        .to_lowercase();
    // FNV-1a, 64 bits: the same in every build and version, unlike the
    // standard library's hasher.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}
