//! Administrator rights: whether Thumbring has them, and running a program
//! with them after the UAC prompt.
//!
//! Windows keeps programs without administrator rights away from the ones
//! that have them (User Interface Privilege Isolation, UIPI): their mouse
//! hooks and simulated input do not reach windows that run as administrator,
//! and their window messages do not reach such windows either.

use std::path::Path;
use std::sync::OnceLock;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_CANCELLED, HANDLE, HLOCAL, HWND, LocalFree, WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_ELEVATION_TYPE, TOKEN_INFORMATION_CLASS,
    TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenElevationType, TokenElevationTypeDefault,
    TokenUser,
};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{
    SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{ChangeWindowMessageFilterEx, MSGFLT_ALLOW, SW_HIDE};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

use crate::log::warning;

/// Whether this process runs with administrator rights. It never changes
/// while the process runs, so it is read once.
pub fn is_elevated() -> bool {
    static ELEVATED: OnceLock<bool> = OnceLock::new();
    *ELEVATED.get_or_init(|| {
        let mut elevation = TOKEN_ELEVATION::default();
        // SAFETY: `elevation` is a local of the size given.
        let read = unsafe { read_token(TokenElevation, &mut elevation) };
        read && elevation.TokenIsElevated != 0
    })
}

/// Whether this account can get administrator rights by accepting the UAC
/// prompt, or has them already. A standard account cannot: the prompt asks
/// for another person's password, and the rights would be theirs.
pub fn can_elevate() -> bool {
    let mut kind = TOKEN_ELEVATION_TYPE::default();
    // SAFETY: `kind` is a local of the size given.
    let read = unsafe { read_token(TokenElevationType, &mut kind) };
    // "Default" is an account without the split rights that UAC gives an
    // administrator, so it has administrator rights only if it is elevated.
    read && (kind != TokenElevationTypeDefault || is_elevated())
}

/// The security identifier of the user running Thumbring, such as
/// `S-1-5-21-...-1001`.
pub fn user_sid() -> Result<String, String> {
    // TOKEN_USER is followed by the SID itself; 256 bytes fit any SID.
    let mut buffer = [0u64; 32];
    // SAFETY: the buffer is a local, aligned for TOKEN_USER, of the size given.
    let read = unsafe { read_token(TokenUser, &mut buffer) };
    if !read {
        return Err("could not read the user of this process".into());
    }
    // SAFETY: GetTokenInformation filled the buffer with a TOKEN_USER whose
    // SID points inside the same buffer; the string from
    // ConvertSidToStringSidW is freed with LocalFree once copied.
    unsafe {
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).map_err(|e| e.to_string())?;
        let sid = text.to_string().map_err(|e| e.to_string());
        let _ = LocalFree(Some(HLOCAL(text.0 as *mut _)));
        sid
    }
}

/// Reads one piece of information about this process's access token.
///
/// # Safety
/// `T` must be the type Windows writes for `class`, or a buffer big enough
/// for it.
unsafe fn read_token<T>(class: TOKEN_INFORMATION_CLASS, value: &mut T) -> bool {
    let mut token = HANDLE::default();
    // SAFETY: the pseudo-handle of this process needs no closing; the token
    // handle is closed below.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut size = 0;
        let result = GetTokenInformation(
            token,
            class,
            Some(value as *mut T as *mut _),
            size_of::<T>() as u32,
            &mut size,
        );
        let _ = CloseHandle(token);
        result.is_ok()
    }
}

/// Why [`run_as_administrator`] did not finish.
#[derive(Debug)]
pub enum Failure {
    /// The person answered "No" to the UAC prompt.
    Cancelled,
    Error(String),
}

/// Runs `program` with administrator rights, hidden, waits for it and returns
/// its exit code. Windows shows the UAC prompt first, unless Thumbring has
/// administrator rights already. Blocks until the program ends, so it must not
/// run on the thread of a window.
pub fn run_as_administrator(program: &Path, args: &str, owner: HWND) -> Result<u32, Failure> {
    let program = HSTRING::from(program);
    let args = HSTRING::from(args);
    // The shell may use COM, which must be initialized on the thread.
    // SAFETY: initialized and uninitialized on this same thread.
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        hwnd: owner,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(program.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` and the strings it points to live until the call
    // returns; the process handle it gives back is closed below.
    let result = unsafe {
        match ShellExecuteExW(&mut info) {
            Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Err(Failure::Cancelled),
            Err(e) => Err(Failure::Error(e.to_string())),
            Ok(()) if info.hProcess.is_invalid() => {
                Err(Failure::Error("Windows did not start the program".into()))
            }
            Ok(()) => {
                let process = info.hProcess;
                let mut code = 0;
                let ended = WaitForSingleObject(process, 60_000) == WAIT_OBJECT_0
                    && GetExitCodeProcess(process, &mut code).is_ok();
                let _ = CloseHandle(process);
                if ended {
                    Ok(code)
                } else {
                    Err(Failure::Error("the program did not finish in time".into()))
                }
            }
        }
    };
    if com.is_ok() {
        // SAFETY: pairs with the CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    result
}

/// Lets `messages` reach `window` from programs without administrator rights,
/// which Windows blocks while Thumbring runs as administrator.
pub fn accept_from_programs_without_rights(window: HWND, messages: &[u32]) {
    for &message in messages {
        // SAFETY: `window` belongs to this process; it only changes which
        // messages Windows lets through to it.
        if let Err(e) = unsafe { ChangeWindowMessageFilterEx(window, message, MSGFLT_ALLOW, None) }
        {
            warning!("could not let message {message:#x} through to the tray window: {e}");
        }
    }
}
