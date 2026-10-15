//! Command runner. Runs on the worker thread, never inside the hook callback.
//!
//! While Thumbring runs as administrator, whatever it opens would get those
//! rights too. So it asks File Explorer, which runs without them, to open it
//! instead. If Explorer does not answer, nothing opens: the settings file that
//! names what to open can be changed by programs without administrator rights,
//! and they must not get those rights through Thumbring.

use std::path::Path;
use std::thread;

use thumbring_core::command::{Builtin, Command, Open};
use thumbring_core::config::MouseButton;
use windows::Win32::Foundation::S_FALSE;
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoUninitialize, IDispatch, IServiceProvider,
};
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::{
    ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR_EXECUTABLE, AssocQueryStringW, CSIDL_DESKTOP,
    IShellBrowser, IShellDispatch2, IShellFolderViewDual, IShellWindows, SEE_MASK_NOASYNC,
    SHELLEXECUTEINFOW, SID_STopLevelBrowser, SVGIO_BACKGROUND, SWC_DESKTOP, SWFO_NEEDDISPATCH,
    ShellExecuteExW, ShellWindows,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{BSTR, HSTRING, Interface, PCWSTR};

use crate::log::warning;
use crate::tray::{self, Notice};
use crate::{elevation, input, text};

/// Runs `command`. `button` is the one that asked for it: "native" sends its
/// own click again.
pub fn run(command: &Command, button: MouseButton) -> Result<(), String> {
    match command {
        Command::Native => input::click(button),
        Command::Nothing => Ok(()),
        Command::Keys(chord) => input::send_chord(chord),
        // Windows ignores a simulated Win+L (it is a secure key combination).
        Command::Builtin(Builtin::Lock) => {
            // SAFETY: no parameters; it only asks to lock the session.
            unsafe { LockWorkStation() }.map_err(|e| e.to_string())
        }
        Command::Builtin(builtin) => {
            let chord = builtin.chord().expect("only Lock has no shortcut");
            input::send_chord(&chord)
        }
        Command::Open(open) => {
            open_in_background(open.clone());
            Ok(())
        }
    }
}

/// Opens a text file (settings, log) in the associated editor. Windows usually
/// has no program associated with `.toml`: without one, the shell would ask
/// "How do you want to open this file?", so Notepad opens it instead.
pub fn open_text_file(path: &Path) {
    let target = path.to_string_lossy().into_owned();
    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()));
    let open = if extension.is_some_and(|e| has_association(&e)) {
        Open { target, args: None }
    } else {
        Open {
            target: "notepad.exe".into(),
            args: Some(format!("\"{target}\"")),
        }
    };
    open_in_background(open);
}

/// The settings window, shipped next to `thumbring.exe`.
const SETTINGS_EXE: &str = "thumbring-settings.exe";

/// Opens the settings window on the same settings file. Without the window,
/// the file opens in a text editor.
pub fn open_settings_window(config_file: &Path, dir: Option<&Path>) {
    let window = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(SETTINGS_EXE)))
        .filter(|path| path.is_file());
    let Some(window) = window else {
        warning!("{SETTINGS_EXE} is not next to the engine; opening the settings file instead");
        tray::notify(
            Notice::Warning,
            text::notice::NO_SETTINGS_WINDOW_TITLE,
            text::notice::NO_SETTINGS_WINDOW_TEXT,
        );
        open_text_file(config_file);
        return;
    };
    open_in_background(Open {
        target: window.to_string_lossy().into_owned(),
        args: dir.map(dir_argument),
    });
}

/// `--dir "<folder>"` for a command line. Backslashes right before the
/// closing quote are doubled, or they would escape it (`"C:\"`).
fn dir_argument(dir: &Path) -> String {
    let dir = dir.to_string_lossy();
    let trailing = dir.len() - dir.trim_end_matches('\\').len();
    format!("--dir \"{dir}{}\"", "\\".repeat(trailing))
}

/// Opens on a separate thread: `ShellExecuteEx` can take a while (a network
/// path, a slow app) and the caller must not block.
fn open_in_background(open: Open) {
    let target = open.target.clone();
    let spawned = thread::Builder::new().name("open".into()).spawn(move || {
        if let Err(e) = shell_open(&open) {
            warning!("could not open {}: {e}", open.target);
        }
    });
    if let Err(e) = spawned {
        warning!("could not open {target}: {e}");
    }
}

fn shell_open(open: &Open) -> Result<(), String> {
    // Some shell extensions use COM, which must be initialized on the thread.
    // SAFETY: initialized and uninitialized on this same thread.
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    let file = expand(&open.target);
    let args = open.args.as_deref().map(expand);
    let result = if elevation::is_elevated() {
        open_through_explorer(&file, args.as_deref()).map_err(|e| {
            tray::notify(
                Notice::Warning,
                text::notice::NOT_OPENED_TITLE,
                text::notice::NOT_OPENED_TEXT,
            );
            format!(
                "File Explorer did not open it ({e}), and Thumbring does not open it itself, \
                 or it would run as administrator"
            )
        })
    } else {
        open_here(&file, args.as_deref()).map_err(|e| e.to_string())
    };
    if com.is_ok() {
        // SAFETY: pairs with the CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    result
}

/// Opens from this process, which passes its own rights on.
fn open_here(file: &str, args: Option<&str>) -> windows::core::Result<()> {
    let file = HSTRING::from(file);
    let args = args.map(HSTRING::from);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        // The thread ends right after: the shell must finish before returning.
        fMask: SEE_MASK_NOASYNC,
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: args.as_ref().map_or(PCWSTR::null(), |a| PCWSTR(a.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` and the strings it points to live until the call returns.
    unsafe { ShellExecuteExW(&mut info) }
}

/// Asks the desktop, which belongs to File Explorer, to open `file`. What
/// opens gets Explorer's rights, the user's normal ones, even while Thumbring
/// runs as administrator. COM must be initialized on the thread.
fn open_through_explorer(file: &str, args: Option<&str>) -> windows::core::Result<()> {
    // SAFETY: COM calls with arguments that live during each call; Explorer
    // runs them in its own process.
    unsafe {
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER)?;
        let desktop = VARIANT::from(CSIDL_DESKTOP as i32);
        let mut window = 0;
        let found = windows.FindWindowSW(
            &desktop,
            &VARIANT::default(),
            SWC_DESKTOP,
            &mut window,
            SWFO_NEEDDISPATCH,
        )?;
        let browser: IShellBrowser = found
            .cast::<IServiceProvider>()?
            .QueryService(&SID_STopLevelBrowser)?;
        let view: IDispatch = browser
            .QueryActiveShellView()?
            .GetItemObject(SVGIO_BACKGROUND)?;
        let shell: IShellDispatch2 = view.cast::<IShellFolderViewDual>()?.Application()?.cast()?;
        shell.ShellExecute(
            &BSTR::from(file),
            &VARIANT::from(args.unwrap_or("")),
            &VARIANT::from(""),
            &VARIANT::from(""),
            &VARIANT::from(SW_SHOWNORMAL.0),
        )
    }
}

/// Replaces `%VARIABLE%` with its value.
pub fn expand(text: &str) -> String {
    let source = HSTRING::from(text);
    // SAFETY: first asks for the size, then writes into a buffer of that size.
    unsafe {
        let needed = ExpandEnvironmentStringsW(&source, None);
        if needed == 0 {
            return text.to_owned();
        }
        let mut buffer = vec![0u16; needed as usize];
        let written = ExpandEnvironmentStringsW(&source, Some(&mut buffer));
        if written == 0 || written > needed {
            return text.to_owned();
        }
        // `written` counts the trailing NUL.
        String::from_utf16_lossy(&buffer[..written as usize - 1])
    }
}

fn has_association(extension: &str) -> bool {
    let mut length = 0u32;
    // SAFETY: without a buffer, the function only reports the size in `length`.
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_INIT_IGNOREUNKNOWN,
            ASSOCSTR_EXECUTABLE,
            &HSTRING::from(extension),
            PCWSTR::null(),
            None,
            &mut length,
        )
    };
    // Without a buffer, success comes back as S_FALSE (with the needed size).
    result == S_FALSE || result.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_is_quoted_for_the_command_line() {
        assert_eq!(
            dir_argument(Path::new(r"S:\My tests\thumbring")),
            r#"--dir "S:\My tests\thumbring""#
        );
        // A drive's root ends in a backslash that must not escape the quote.
        assert_eq!(dir_argument(Path::new(r"D:\")), r#"--dir "D:\\""#);
    }
}
