//! Thumbring Settings: a window that edits the settings file.
//!
//! The interface is the HTML, CSS and JavaScript in `ui/`, shown by WebView2
//! and embedded in the executable. It works on a JSON copy of the settings
//! and calls the commands below to read and save them.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value as Json;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use thumbring_core::instance::file_key;
use thumbring_settings::apps::{self, RunningApp};
use thumbring_settings::session::{OpenError, SaveError, Session, Snapshot};
use thumbring_settings::{engine, file, icons};

const WINDOW: &str = "main";

struct State {
    path: PathBuf,
    session: Mutex<Option<Session>>,
    /// The interface has changes that are not saved.
    dirty: AtomicBool,
    /// Closing was already confirmed: do not ask again.
    closing: AtomicBool,
}

/// Reads the file again, dropping what the interface had.
#[tauri::command]
fn open_settings(state: tauri::State<'_, State>) -> Result<Snapshot, OpenError> {
    let session = Session::open(&state.path)?;
    let snapshot = session.snapshot();
    *state.session.lock().expect("no panics while locked") = Some(session);
    state.dirty.store(false, Ordering::SeqCst);
    Ok(snapshot)
}

#[tauri::command]
fn save_settings(
    state: tauri::State<'_, State>,
    settings: Json,
    overwrite: bool,
) -> Result<Snapshot, SaveError> {
    let mut session = state.session.lock().expect("no panics while locked");
    let session = session
        .as_mut()
        .ok_or_else(|| SaveError::Io("the settings were not read yet".into()))?;
    let snapshot = session.save(&settings, overwrite)?;
    state.dirty.store(false, Ordering::SeqCst);
    Ok(snapshot)
}

/// Asks for a file or a folder with the system's own dialog: `kind` is
/// "app", "file" or "folder". `None` when cancelled.
#[tauri::command]
async fn browse(window: WebviewWindow, kind: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = window.dialog().file().set_parent(&window);
    let picked = match kind.as_str() {
        "folder" => dialog.blocking_pick_folder(),
        "app" => dialog
            .add_filter("Programs", &["exe", "com", "bat", "cmd", "lnk"])
            .blocking_pick_file(),
        _ => dialog.blocking_pick_file(),
    };
    let path = picked?.into_path().ok()?;
    Some(path.display().to_string())
}

/// The apps with a window on the screen now, to pick from.
#[tauri::command]
fn running_apps() -> Vec<RunningApp> {
    apps::running_apps()
}

/// The name of an app as the settings file writes it, from a name or a path.
#[tauri::command]
fn app_name(text: String) -> Option<String> {
    apps::app_name(&text)
}

/// The ring's icons, as name and SVG text, in the engine's order.
#[tauri::command]
fn ring_icons() -> &'static [(&'static str, &'static str)] {
    icons::RING_ICONS
}

/// Whether the engine runs and reads this file: then a save is in effect at
/// once.
#[tauri::command]
fn engine_status(state: tauri::State<'_, State>) -> engine::Status {
    engine::status(&state.path)
}

#[tauri::command]
fn set_dirty(state: tauri::State<'_, State>, dirty: bool) {
    state.dirty.store(dirty, Ordering::SeqCst);
}

/// The interface is drawn: show the window (it starts hidden, so it never
/// flashes white).
#[tauri::command]
fn ready(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

/// Closes without asking: the interface already asked.
#[tauri::command]
fn close_window(window: WebviewWindow, state: tauri::State<'_, State>) {
    state.closing.store(true, Ordering::SeqCst);
    let _ = window.close();
}

/// Shown when WebView2, which draws the window on Windows, is missing.
#[cfg(windows)]
const NO_WEBVIEW: &str = "Thumbring Settings needs the WebView2 Runtime, which comes with \
    Windows 11 and with the updates of Windows 10. Install it from \
    https://developer.microsoft.com/microsoft-edge/webview2/ and open the settings again.\n\n\
    Thumbring itself keeps working without it.";

/// A message box, for problems that happen before the window exists (a
/// release build has no console).
#[cfg(windows)]
fn alert(text: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::{HSTRING, w};
    // SAFETY: both strings live until the call returns.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            w!("Thumbring Settings"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn alert(text: &str) {
    eprintln!("{text}");
}

/// The settings file a second launch asked for. Its `--dir` may be relative
/// to the folder it started in, `cwd`.
fn asked_file(args: Vec<String>, cwd: &str) -> Option<PathBuf> {
    let mut args = args.into_iter().skip(1);
    let mut resolved = Vec::new();
    while let Some(arg) = args.next() {
        let dir = (arg == "--dir").then(|| args.next()).flatten();
        resolved.push(OsString::from(arg));
        if let Some(dir) = dir {
            resolved.push(Path::new(cwd).join(dir).into_os_string());
        }
    }
    file::config_path(resolved).ok()
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let testing = args.iter().any(|a| a == "--dir");
    let paths =
        file::config_path(args).and_then(|path| Ok((file::window_data_dir(&path, testing)?, path)));
    let (data_dir, path) = match paths {
        Ok(paths) => paths,
        Err(e) => {
            alert(&e);
            return ExitCode::from(2);
        }
    };
    #[cfg(windows)]
    if tauri::webview_version().is_err() {
        alert(NO_WEBVIEW);
        return ExitCode::FAILURE;
    }
    // While it lives, the engine leaves the "reloaded" message to this window.
    let _mark = engine::mark(&path);

    let result = tauri::Builder::default()
        // First: a second launch only brings this window to the front.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            if let Some(window) = app.get_webview_window(WINDOW) {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            // Asked for another file (another --dir): this window keeps its
            // own, and says so.
            if let Some(asked) = asked_file(args, &cwd) {
                if file_key(&asked) != file_key(&app.state::<State>().path) {
                    let _ = app.emit("other-file", asked.display().to_string());
                }
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(State {
            path,
            session: Mutex::new(None),
            dirty: AtomicBool::new(false),
            closing: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            open_settings,
            save_settings,
            ring_icons,
            running_apps,
            app_name,
            browse,
            engine_status,
            set_dirty,
            ready,
            close_window
        ])
        .setup(move |app| {
            WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html".into()))
                .title("Thumbring Settings")
                .inner_size(1120.0, 780.0)
                .min_inner_size(760.0, 560.0)
                .data_directory(data_dir.clone())
                .visible(false)
                .build()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<State>();
                if state.dirty.load(Ordering::SeqCst) && !state.closing.load(Ordering::SeqCst) {
                    // The interface asks whether to save first.
                    api.prevent_close();
                    let _ = window.emit("close-requested", ());
                }
            }
        })
        .run(tauri::generate_context!());

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            alert(&format!(
                "Thumbring Settings could not open its window:\n\n{e}"
            ));
            ExitCode::FAILURE
        }
    }
}
