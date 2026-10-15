//! One engine per session: two hooks on the same button would fight over the
//! click. The engine also marks which settings file it reads, for the
//! settings window, and checks whether that window is open.

use std::path::Path;

use thumbring_core::instance;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE,
};
use windows::Win32::System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
use windows::core::HSTRING;

/// While it exists, no other engine starts. Releases the mutexes when dropped.
pub struct Guard {
    engine: HANDLE,
    /// Tells the settings window which file this engine reads. `None` if it
    /// could not be made: the window then says less about saving.
    file: Option<HANDLE>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: the handles are ours and are closed only once.
        unsafe {
            let _ = CloseHandle(self.engine);
            if let Some(file) = self.file {
                let _ = CloseHandle(file);
            }
        }
    }
}

/// `None` if an engine is already running in this session.
pub fn acquire(config_file: &Path) -> windows::core::Result<Option<Guard>> {
    // SAFETY: creates (or opens) a named mutex; the last error is read right
    // after, before any other call.
    unsafe {
        let handle = match CreateMutexW(None, false, &HSTRING::from(instance::ENGINE)) {
            Ok(handle) => handle,
            // An engine running as administrator made it, and this one, without
            // those rights, may not open it.
            Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => return Ok(None),
            Err(e) => return Err(e),
        };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Ok(None);
        }
        let file = CreateMutexW(
            None,
            false,
            &HSTRING::from(instance::engine_for(config_file)),
        );
        Ok(Some(Guard {
            engine: handle,
            file: file.ok(),
        }))
    }
}

/// Whether the settings window is open on `config_file`.
pub fn settings_window_open(config_file: &Path) -> bool {
    let name = HSTRING::from(instance::settings_for(config_file));
    // SAFETY: only opens a named mutex to see whether it exists, and closes it.
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &name) } {
        Ok(handle) => {
            // SAFETY: the handle was just opened here.
            let _ = unsafe { CloseHandle(handle) };
            true
        }
        // It exists, made by a program with other rights.
        Err(e) => e.code() == ERROR_ACCESS_DENIED.to_hresult(),
    }
}
