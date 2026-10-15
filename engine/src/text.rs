//! Every text the user sees (tray icon, menu, notifications, message boxes),
//! kept in one place so it can be translated later.

use thumbring_core::apps::Activity;

pub const APP_NAME: &str = "Thumbring";

pub mod menu {
    pub const PAUSE: &str = "Pause gestures";
    pub const RESUME: &str = "Resume gestures";
    pub const OPEN_SETTINGS: &str = "Open settings";
    pub const OPEN_CONFIG: &str = "Open settings file";
    pub const OPEN_LOG: &str = "Open log";
    pub const START_WITH_WINDOWS: &str = "Start with Windows";
    pub const START_AS_ADMINISTRATOR: &str = "Start with Windows as administrator";
    pub const QUIT: &str = "Quit";
}

/// Tray icon tooltip. Windows cuts it at 127 characters.
pub fn tooltip(activity: &Activity) -> String {
    match activity {
        Activity::Active => format!("{APP_NAME}: gestures on\nClick to pause"),
        Activity::Paused => format!("{APP_NAME}: paused\nClick to resume"),
        Activity::Excluded(app) => format!("{APP_NAME}: off in {app}\n(exclusion list)"),
        Activity::Fullscreen => format!("{APP_NAME}: off while an app is fullscreen"),
    }
}

pub mod notice {
    // Windows cuts titles at 63 characters and texts at 255.
    pub const FIRST_RUN_TITLE: &str = "Thumbring is running";
    pub const FIRST_RUN_TEXT: &str = "Hold Back and drag for gestures, or hold Forward for the ring. \
        The icon is in the notification area, maybe behind the ^ arrow: right-click it for \
        the menu.";
    pub const RELOADED_TITLE: &str = "Settings reloaded";
    pub const RELOADED_TEXT: &str = "The changes are already in effect.";
    pub const RELOADED_WITH_WARNINGS_TITLE: &str = "Settings reloaded with warnings";
    pub const LOADED_WITH_WARNINGS_TITLE: &str = "Settings loaded with warnings";
    pub const CONFIG_ERROR_TITLE: &str = "Settings error";
    pub const CONFIG_ERROR_TEXT: &str =
        "The file could not be read. The previous settings stay; the reason is in the log.";
    pub const CONFIG_ERROR_AT_START_TEXT: &str = "The file could not be read, so the defaults \
        are in use until it is fixed. The reason is in the log.";
    pub const AUTOSTART_ERROR_TITLE: &str = "Start with Windows";
    pub const AUTOSTART_ERROR_TEXT: &str =
        "The option could not be changed. The reason is in the log.";
    pub const ADMIN_START_TITLE: &str = "Start with Windows as administrator";
    pub const ADMIN_START_TEXT: &str = "From the next sign-in, Thumbring starts with \
        administrator rights, without asking, and also works in windows that run as \
        administrator. The apps it opens still run without those rights.";
    pub const NO_SETTINGS_WINDOW_TITLE: &str = "Settings window not found";
    pub const NO_SETTINGS_WINDOW_TEXT: &str = "thumbring-settings.exe is not next to \
        thumbring.exe, so the settings file opens in a text editor instead. Installing \
        Thumbring again brings the window back.";
    pub const NOT_OPENED_TITLE: &str = "Nothing was opened";
    pub const NOT_OPENED_TEXT: &str = "Thumbring runs as administrator and opens apps through \
        File Explorer, so they do not get those rights. File Explorer did not answer; try again \
        in a moment.";

    pub fn reloaded_with_warnings(count: usize) -> String {
        if count == 1 {
            "1 invalid value went back to its default. The details are in the log.".into()
        } else {
            format!(
                "{count} invalid values went back to their defaults. The details are in the log."
            )
        }
    }
}

pub mod alert {
    use super::APP_NAME;

    pub fn already_running() -> String {
        format!(
            "{APP_NAME} is already running. Its icon is in the notification area, next to the clock."
        )
    }

    pub fn fatal(error: &dyn std::fmt::Display) -> String {
        format!("{APP_NAME} could not continue:\n\n{error}")
    }

    pub fn crashed(log_file: &std::path::Path) -> String {
        format!(
            "{APP_NAME} stopped because of an unexpected error. The mouse buttons work as \
             usual again.\n\nThe details are at the end of the log:\n{}\n\nPlease report the \
             problem on the project page, with that part of the log.",
            log_file.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_fit_in_a_notification() {
        // Windows cuts longer titles and texts.
        for (title, text) in [
            (notice::FIRST_RUN_TITLE, notice::FIRST_RUN_TEXT),
            (notice::ADMIN_START_TITLE, notice::ADMIN_START_TEXT),
            (notice::NOT_OPENED_TITLE, notice::NOT_OPENED_TEXT),
            (
                notice::NO_SETTINGS_WINDOW_TITLE,
                notice::NO_SETTINGS_WINDOW_TEXT,
            ),
        ] {
            assert!(title.encode_utf16().count() <= 63, "{title}");
            assert!(text.encode_utf16().count() <= 255, "{text}");
        }
        assert!(notice::CONFIG_ERROR_AT_START_TEXT.encode_utf16().count() <= 255);
    }
}
