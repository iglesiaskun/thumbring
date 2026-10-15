//! Reading the settings file, and reloading it automatically on save.
//!
//! Windows says that something changed in the folder, not what. The watcher
//! waits for the write to finish, compares the content with what it already
//! read and reloads only if it changed. So an editor that saves in several
//! steps (truncate and write, or write a temporary file and rename it) causes
//! a single reload.
//!
//! Problems in the file (invalid values, a file that cannot be read) go to the
//! log and to a notification, once: saving again with the same problems does
//! not repeat the warning. While the settings window is open on the file, a
//! reload without problems has no notification: the window says it was saved.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use thumbring_core::config::{Config, Loaded};
use windows::Win32::Foundation::{ERROR_SHARING_VIOLATION, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE,
    FindFirstChangeNotificationW, FindNextChangeNotification,
};
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};
use windows::core::HSTRING;

use crate::log::{error, info, warning};
use crate::ring::icons;
use crate::tray::{self, Notice};
use crate::{instance, text};

/// How long the folder must stay quiet before the file is read again.
const QUIET: Duration = Duration::from_millis(200);

/// Result of reading the file.
pub struct Read {
    /// Raw content, to compare with on the next change.
    pub bytes: Vec<u8>,
    /// `None` if the file could not be read as settings.
    pub loaded: Option<Loaded>,
    pub problems: Problems,
}

/// What is wrong with the file, as told in the log.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Problems {
    #[default]
    None,
    /// Values that went back to their defaults.
    Warnings(Vec<String>),
    /// The file could not be read as settings (the reason).
    Unreadable(String),
}

/// Reads and validates the file, logging every problem.
pub fn read(path: &Path) -> io::Result<Read> {
    let bytes = read_with_retry(path)?;
    let (loaded, problems) = parse(&bytes);
    Ok(Read {
        bytes,
        loaded,
        problems,
    })
}

fn parse(bytes: &[u8]) -> (Option<Loaded>, Problems) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        let reason = "the settings file is not UTF-8; save it as UTF-8".to_string();
        error!("{reason}");
        return (None, Problems::Unreadable(reason));
    };
    match Config::from_toml(text) {
        Ok(mut loaded) => {
            // Icon files are told among the other problems of the file.
            loaded.check_icon_files(|path, _| icons::file(path).map(drop));
            let warnings: Vec<String> = loaded.warnings.iter().map(|w| w.to_string()).collect();
            for warning in &warnings {
                warning!("settings: {warning}");
            }
            let problems = if warnings.is_empty() {
                Problems::None
            } else {
                Problems::Warnings(warnings)
            };
            (Some(loaded), problems)
        }
        Err(e) => {
            error!("{e}");
            (None, Problems::Unreadable(e.to_string()))
        }
    }
}

/// The editor may have the file open at that very moment: try again a few times.
fn read_with_retry(path: &Path) -> io::Result<Vec<u8>> {
    let mut attempts = 0;
    loop {
        match fs::read(path) {
            Err(e)
                if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION.0 as i32) && attempts < 10 =>
            {
                attempts += 1;
                thread::sleep(Duration::from_millis(50));
            }
            result => return result,
        }
    }
}

/// A notification about the settings file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notification {
    Reloaded,
    Warnings(usize),
    Unreadable,
}

/// What to tell when the engine starts: only problems.
fn at_start(problems: &Problems) -> Option<Notification> {
    match problems {
        Problems::None => None,
        Problems::Warnings(w) => Some(Notification::Warnings(w.len())),
        Problems::Unreadable(_) => Some(Notification::Unreadable),
    }
}

/// What to tell after a save, given the problems already told. The same
/// problems are not told twice: with the same invalid values the save is
/// confirmed as a plain reload, and the same unreadable file says nothing.
fn after_save(problems: &Problems, told: &Problems) -> Option<Notification> {
    match problems {
        Problems::None => Some(Notification::Reloaded),
        Problems::Warnings(_) if problems == told => Some(Notification::Reloaded),
        Problems::Warnings(w) => Some(Notification::Warnings(w.len())),
        Problems::Unreadable(_) if problems == told => None,
        Problems::Unreadable(_) => Some(Notification::Unreadable),
    }
}

/// Tells about the problems found when the engine started.
pub fn notify_at_start(problems: &Problems) {
    match at_start(problems) {
        Some(Notification::Warnings(count)) => tray::notify(
            Notice::Warning,
            text::notice::LOADED_WITH_WARNINGS_TITLE,
            &text::notice::reloaded_with_warnings(count),
        ),
        Some(Notification::Unreadable) => tray::notify(
            Notice::Error,
            text::notice::CONFIG_ERROR_TITLE,
            text::notice::CONFIG_ERROR_AT_START_TEXT,
        ),
        Some(Notification::Reloaded) | None => {}
    }
}

fn notify_after_save(path: &Path, problems: &Problems, told: &Problems) {
    match after_save(problems, told) {
        Some(Notification::Reloaded) if instance::settings_window_open(path) => {
            info!("the settings window is open and tells about the save itself");
        }
        Some(Notification::Reloaded) => tray::notify(
            Notice::Info,
            text::notice::RELOADED_TITLE,
            text::notice::RELOADED_TEXT,
        ),
        Some(Notification::Warnings(count)) => tray::notify(
            Notice::Warning,
            text::notice::RELOADED_WITH_WARNINGS_TITLE,
            &text::notice::reloaded_with_warnings(count),
        ),
        Some(Notification::Unreadable) => tray::notify(
            Notice::Error,
            text::notice::CONFIG_ERROR_TITLE,
            text::notice::CONFIG_ERROR_TEXT,
        ),
        None => info!("same problem as before; no new notification"),
    }
}

/// Watches the file on a thread of its own. `last` is the content already
/// loaded and `told` its problems, already told; `on_change` receives every
/// new configuration read without errors.
pub fn spawn(
    path: PathBuf,
    last: Option<Vec<u8>>,
    told: Problems,
    on_change: impl Fn(Config) + Send + 'static,
) -> io::Result<()> {
    thread::Builder::new()
        .name("config-watcher".into())
        .spawn(move || {
            if let Err(e) = watch(&path, last, told, on_change) {
                error!("stopped watching the settings file: {e}");
            }
        })?;
    Ok(())
}

fn watch(
    path: &Path,
    mut last: Option<Vec<u8>>,
    mut told: Problems,
    on_change: impl Fn(Config),
) -> windows::core::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    // SAFETY: the notification handle stays with this thread until it ends.
    let change = unsafe {
        FindFirstChangeNotificationW(
            &HSTRING::from(dir.as_os_str()),
            false,
            FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_SIZE,
        )?
    };
    let mut missing = false;
    loop {
        // SAFETY: `change` is a valid notification handle.
        unsafe {
            if WaitForSingleObject(change, INFINITE) != WAIT_OBJECT_0 {
                return Err(windows::core::Error::from_thread());
            }
            // Wait for the folder to go quiet: every change restarts the wait.
            loop {
                FindNextChangeNotification(change)?;
                if WaitForSingleObject(change, QUIET.as_millis() as u32) == WAIT_TIMEOUT {
                    break;
                }
            }
        }

        let bytes = match read_with_retry(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if !missing {
                    warning!("the settings file is gone; keeping the current settings");
                    missing = true;
                }
                continue;
            }
            Err(e) => {
                warning!("could not read the settings file: {e}");
                continue;
            }
        };
        missing = false;
        if last.as_ref() == Some(&bytes) {
            continue;
        }
        info!("the settings file changed; reloading");
        let (loaded, problems) = parse(&bytes);
        last = Some(bytes);
        match &problems {
            Problems::None => info!("settings reloaded"),
            Problems::Warnings(w) => info!("settings reloaded with {} warning(s)", w.len()),
            Problems::Unreadable(_) => {}
        }
        notify_after_save(path, &problems, &told);
        told = problems;
        if let Some(loaded) = loaded {
            on_change(loaded.config);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warnings(list: &[&str]) -> Problems {
        Problems::Warnings(list.iter().map(|w| w.to_string()).collect())
    }

    #[test]
    fn a_clean_start_says_nothing() {
        assert_eq!(at_start(&Problems::None), None);
    }

    #[test]
    fn problems_at_start_are_told() {
        assert_eq!(
            at_start(&warnings(&["a: bad", "b: bad"])),
            Some(Notification::Warnings(2))
        );
        assert_eq!(
            at_start(&Problems::Unreadable("syntax".into())),
            Some(Notification::Unreadable)
        );
    }

    #[test]
    fn every_clean_save_is_confirmed() {
        assert_eq!(
            after_save(&Problems::None, &Problems::None),
            Some(Notification::Reloaded)
        );
        // Including the save that fixes the problems.
        assert_eq!(
            after_save(&Problems::None, &warnings(&["a: bad"])),
            Some(Notification::Reloaded)
        );
    }

    #[test]
    fn the_same_warnings_are_not_told_twice() {
        let told = warnings(&["a: bad"]);
        assert_eq!(
            after_save(&warnings(&["a: bad"]), &told),
            Some(Notification::Reloaded)
        );
        // A new problem, or one more, is told.
        assert_eq!(
            after_save(&warnings(&["b: bad"]), &told),
            Some(Notification::Warnings(1))
        );
        assert_eq!(
            after_save(&warnings(&["a: bad", "b: bad"]), &told),
            Some(Notification::Warnings(2))
        );
    }

    #[test]
    fn the_same_unreadable_file_is_not_told_twice() {
        let told = Problems::Unreadable("line 3: expected =".into());
        assert_eq!(after_save(&told.clone(), &told), None);
        assert_eq!(
            after_save(&Problems::Unreadable("line 9: expected =".into()), &told),
            Some(Notification::Unreadable)
        );
        assert_eq!(
            after_save(&told, &Problems::None),
            Some(Notification::Unreadable)
        );
    }

    #[test]
    fn parse_reports_warnings_and_unreadable_files() {
        let (loaded, problems) = parse(b"[animation]\nspeed = 9.0\n");
        assert!(loaded.is_some());
        assert!(
            matches!(&problems, Problems::Warnings(w) if w.len() == 1),
            "{problems:?}"
        );

        let (loaded, problems) = parse(b"[animation\n");
        assert!(loaded.is_none());
        assert!(matches!(problems, Problems::Unreadable(_)));

        let (loaded, problems) = parse(&[0xff, 0xfe, 0x00]);
        assert!(loaded.is_none());
        assert!(matches!(problems, Problems::Unreadable(_)));

        assert_eq!(parse(b"").1, Problems::None);
    }

    #[test]
    fn a_missing_icon_file_is_a_warning_and_the_item_shows_its_letter() {
        let (loaded, problems) = parse(
            br#"
            [[ring.slots]]
            type = "action"
            name = "Star"
            icon = 'C:\no\such\folder\star.svg'
            "#,
        );
        let Problems::Warnings(warnings) = problems else {
            panic!("expected a warning, got {problems:?}")
        };
        assert_eq!(
            warnings,
            [
                r"ring.slots[0].icon: C:\no\such\folder\star.svg was not found; the item shows the first letter of its name"
            ]
        );
        let config = loaded.expect("loads").config;
        let thumbring_core::ring::Slot::Action(star) = &config.ring.slots[0] else {
            panic!("expected an action")
        };
        assert_eq!(star.icon, None);
    }
}
