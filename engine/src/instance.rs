//! One engine per session: two hooks on the same button would fight over the
//! click.

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// While it exists, no other engine starts. Releases the mutex when dropped.
pub struct Guard(HANDLE);

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: the handle is ours and is closed only once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// `None` if an engine is already running in this session.
pub fn acquire() -> windows::core::Result<Option<Guard>> {
    // SAFETY: creates (or opens) a named mutex; the last error is read right
    // after, before any other call.
    unsafe {
        let handle = match CreateMutexW(None, false, w!(r"Local\Thumbring.Engine")) {
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
        Ok(Some(Guard(handle)))
    }
}
