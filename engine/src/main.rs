//! Engine: gestures on the mouse Back button and the action ring on Forward.
//!
//! Hold Back, drag and release: the gesture runs the command from the settings
//! file. Hold Forward: the ring opens around the cursor. A quick tap on either
//! button still does what the button always did.
//!
//! Threads:
//! - **hook**: the mouse hook; it only decides whether to block and forwards
//!   over a channel;
//! - **worker**: owns the state; recognizes gestures and runs commands;
//! - **ring**: the ring state machine, its window and its animations;
//! - **main**: tray icon and focus change notifications;
//! - **config-watcher**: reloads the settings file when it is saved.
//!
//! The release build opens no console; the log goes to a file (tray menu >
//! Open log).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod art;
mod autostart;
mod console;
mod crash;
mod dpi;
mod elevation;
mod foreground;
mod hook;
mod icon;
mod input;
mod instance;
mod log;
mod paths;
mod ring;
mod session;
mod text;
mod tray;
mod watcher;
mod worker;

use std::error::Error;
use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

use thumbring_core::config::Config;
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MESSAGEBOX_STYLE, MessageBoxW,
};
use windows::core::HSTRING;

use log::{error, info, warning};
use paths::Paths;
use worker::Msg;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e}");
            alert(&text::alert::fatal(&e), MB_ICONERROR);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    // Development aid: shows the sample ring with a fixed script and exits.
    // `--preview-ring [ms] [--save file.png]`: a number of milliseconds freezes
    // the script at that moment; `--save` writes that frame to a PNG instead.
    if args.first().is_some_and(|a| a == "--preview-ring") {
        let freeze_at = match args.get(1) {
            Some(ms) => Some(
                ms.to_string_lossy()
                    .parse::<f64>()
                    .map_err(|_| "--preview-ring takes a number of milliseconds")?,
            ),
            None => None,
        };
        let save = match (args.get(2), args.get(3)) {
            (Some(flag), Some(path)) if flag == "--save" => Some(std::path::PathBuf::from(path)),
            (None, None) => None,
            _ => return Err("usage: --preview-ring [ms] [--save file.png]".into()),
        };
        let _ = dpi::enable_per_monitor_awareness();
        return ring::preview::run(freeze_at, save.as_deref());
    }
    let paths = Paths::from_args(args)?;
    let Some(_instance) = instance::acquire()? else {
        alert(&text::alert::already_running(), MB_ICONINFORMATION);
        return Ok(());
    };
    if let Err(e) = log::init(&paths.log_file) {
        warning!(
            "could not open the log at {}: {e}",
            paths.log_file.display()
        );
    }
    crash::install(paths.log_file.clone());
    info!("engine {} starting", env!("CARGO_PKG_VERSION"));
    if elevation::is_elevated() {
        info!("running as administrator; apps open through File Explorer, without those rights");
    }
    if let Err(e) = dpi::enable_per_monitor_awareness() {
        warning!(
            "could not enable per-monitor DPI ({e}); the threshold may be smaller on scaled screens"
        );
    }

    let StartupSettings {
        config,
        bytes,
        problems,
        first_run,
    } = load_config(&paths);
    let (events, inbox) = mpsc::channel::<Msg>();

    let (ring, ring_thread) =
        match ring::controller::spawn(ring::controller::Settings::of(&config), events.clone()) {
            Ok((handle, thread)) => (Some(handle), Some(thread)),
            Err(e) => {
                warning!("the ring could not start ({e}); the Forward button works as usual");
                (None, None)
            }
        };
    let hook_events = events.clone();
    let hook_thread = hook::install(move |e| {
        let _ = hook_events.send(Msg::Hook(e));
    })?;
    let worker_ring = ring.clone();
    let worker_thread = thread::Builder::new()
        .name("worker".into())
        .spawn(move || worker::run(inbox, config, worker_ring))?;

    tray::create(tray::Context {
        events: events.clone(),
        config_file: paths.config_file.clone(),
        log_file: paths.log_file.clone(),
    })?;
    if first_run {
        // New icons hide behind the arrow of the notification area.
        tray::notify(
            tray::Notice::Info,
            text::notice::FIRST_RUN_TITLE,
            text::notice::FIRST_RUN_TEXT,
        );
    }
    watcher::notify_at_start(&problems);
    let focus_events = events.clone();
    let _focus = foreground::watch(move |window| {
        let _ = focus_events.send(Msg::Foreground(window));
    })?;
    let config_events = events.clone();
    watcher::spawn(paths.config_file.clone(), bytes, problems, move |config| {
        let _ = config_events.send(Msg::Config(Box::new(config)));
    })?;
    // Without a console there is no Ctrl+C to handle, and the error does not matter.
    let _ = console::quit_on_ctrl_c();

    info!("ready; settings at {}", paths.config_file.display());
    tray::run_message_loop();

    // The tray window was closed (Quit, Ctrl+C or end of session).
    let _ = events.send(Msg::Quit);
    hook::request_stop();
    if let Some(ring) = &ring {
        ring.stop();
    }
    worker_thread
        .join()
        .map_err(|_| "the worker thread panicked")?;
    hook_thread.join().map_err(|_| "the hook thread panicked")?;
    if let Some(thread) = ring_thread {
        thread.join().map_err(|_| "the ring thread panicked")?;
    }
    info!("hook removed; bye");
    console::mark_stopped();
    Ok(())
}

/// The settings as read on startup.
struct StartupSettings {
    config: Config,
    /// Raw content of the file, for the watcher to compare with.
    bytes: Option<Vec<u8>>,
    /// What is wrong with the file, to tell once the tray icon exists.
    problems: watcher::Problems,
    /// The file did not exist yet.
    first_run: bool,
}

/// Reads the settings on startup. If that fails, it goes on with the
/// defaults: the engine never refuses to start because of the file.
fn load_config(paths: &Paths) -> StartupSettings {
    let first_run = match paths::ensure_config(&paths.config_file) {
        Ok(created) => {
            if created {
                info!(
                    "settings file created with the defaults: {}",
                    paths.config_file.display()
                );
            }
            created
        }
        Err(e) => {
            warning!(
                "could not create {}: {e}; using the defaults",
                paths.config_file.display()
            );
            false
        }
    };
    match watcher::read(&paths.config_file) {
        Ok(read) => {
            let config = match read.loaded {
                Some(loaded) => loaded.config,
                None => {
                    warning!("using the defaults until the file is fixed");
                    Config::default()
                }
            };
            StartupSettings {
                config,
                bytes: Some(read.bytes),
                problems: read.problems,
                first_run,
            }
        }
        Err(e) => {
            warning!(
                "could not read {}: {e}; using the defaults",
                paths.config_file.display()
            );
            StartupSettings {
                config: Config::default(),
                bytes: None,
                problems: watcher::Problems::Unreadable(e.to_string()),
                first_run,
            }
        }
    }
}

/// Message for whoever started the engine without a console; with a console,
/// the log is enough.
fn alert(message: &str, style: MESSAGEBOX_STYLE) {
    if console::has_stderr() {
        eprintln!("{message}");
        return;
    }
    // SAFETY: modal message box without an owner window; the texts live until
    // it closes.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from(text::APP_NAME),
            MB_OK | style,
        );
    }
}
