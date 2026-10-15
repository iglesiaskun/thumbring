//! The engine, seen from the settings window: whether it runs and reads the
//! same settings file, so the window can tell whether a save is in effect.
//! While the window is open, it marks the file it edits, and the engine leaves
//! the "saved" message to the window.

use std::path::Path;

use serde::Serialize;

/// Where the engine stands, for the file this window edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// It runs and reads this file: it reloads a save right away.
    SameFile,
    /// It runs, but reads another file.
    OtherFile,
    NotRunning,
    /// This system cannot tell.
    Unknown,
}

#[cfg(windows)]
pub use windows_impl::{Mark, mark, status};

#[cfg(not(windows))]
pub struct Mark;

#[cfg(not(windows))]
pub fn mark(_config_file: &Path) -> Option<Mark> {
    None
}

#[cfg(not(windows))]
pub fn status(_config_file: &Path) -> Status {
    Status::Unknown
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use thumbring_core::instance;
    use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
    use windows::Win32::System::Threading::{
        CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE,
    };
    use windows::core::HSTRING;

    /// Tells the engine this window edits a file, until dropped.
    pub struct Mark(HANDLE);

    impl Drop for Mark {
        fn drop(&mut self) {
            // SAFETY: the handle is ours and is closed only once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    /// Marks `config_file` as open in this window. `None` if Windows refused:
    /// the engine then shows its own "reloaded" message too.
    pub fn mark(config_file: &Path) -> Option<Mark> {
        let name = HSTRING::from(instance::settings_for(config_file));
        // SAFETY: creates (or opens) a named mutex, kept until the mark drops.
        unsafe { CreateMutexW(None, false, &name) }.ok().map(Mark)
    }

    pub fn status(config_file: &Path) -> Status {
        if exists(&instance::engine_for(config_file)) {
            Status::SameFile
        } else if exists(instance::ENGINE) {
            Status::OtherFile
        } else {
            Status::NotRunning
        }
    }

    fn exists(name: &str) -> bool {
        let name = HSTRING::from(name);
        // SAFETY: only opens a named mutex to see whether it exists, and
        // closes it.
        match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &name) } {
            Ok(handle) => {
                // SAFETY: the handle was just opened here.
                let _ = unsafe { CloseHandle(handle) };
                true
            }
            // It exists, made by an engine running as administrator.
            Err(e) => e.code() == ERROR_ACCESS_DENIED.to_hresult(),
        }
    }
}
