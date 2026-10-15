//! Start with Windows normally: a value under
//! `HKCU\...\CurrentVersion\Run`.
//!
//! Task Manager and Settings can disable the value without deleting it (under
//! `StartupApproved`); in that case it counts as off. The installer deletes
//! the value by the same name.

use std::path::Path;

use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
    RegSetKeyValueW,
};
use windows::core::{HSTRING, PCWSTR, w};

const RUN: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const APPROVED: PCWSTR =
    w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
const VALUE: PCWSTR = w!("Thumbring");

pub fn is_enabled() -> bool {
    // SAFETY: without a buffer, it only checks that the value exists.
    let exists = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN,
            VALUE,
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    exists.is_ok() && !disabled_in_settings()
}

/// The first byte under `StartupApproved` is even when enabled and odd when
/// disabled from Task Manager.
fn disabled_in_settings() -> bool {
    let mut data = [0u8; 12];
    let mut size = data.len() as u32;
    // SAFETY: `data` and `size` are locals and the size given is the buffer's.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            APPROVED,
            VALUE,
            RRF_RT_REG_BINARY,
            None,
            Some(data.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    result.is_ok() && size > 0 && data[0] & 1 == 1
}

pub fn enable(exe: &Path) -> Result<(), String> {
    let command = HSTRING::from(format!("\"{}\"", exe.display()));
    let bytes = (command.len() + 1) * size_of::<u16>();
    // SAFETY: `command` ends with a NUL and `bytes` counts it.
    let result = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            RUN,
            VALUE,
            REG_SZ.0,
            Some(command.as_ptr() as *const _),
            bytes as u32,
        )
    };
    check(result)?;
    // Clears an old "disabled" mark left by Task Manager.
    // SAFETY: deletes a value; if it does not exist, it just returns an error.
    let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, APPROVED, VALUE) };
    Ok(())
}

pub fn disable() -> Result<(), String> {
    // SAFETY: deletes a value; if it does not exist, it just returns an error.
    let result = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, VALUE) };
    check(result).or_else(|e| if is_enabled() { Err(e) } else { Ok(()) })
}

fn check(result: WIN32_ERROR) -> Result<(), String> {
    result.ok().map_err(|e| e.to_string())
}
