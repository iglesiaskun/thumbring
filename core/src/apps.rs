//! Apps: executable names, per-app profiles, the exclusion list and the
//! decision of when gestures are on.
//!
//! The engine finds out which app has focus and whether it is fullscreen; this
//! module decides what to do with that.

use std::fmt;

use serde::Serialize;

use crate::command::Command;
use crate::config::{Config, Profile};
use crate::gesture::Outcome;

/// Process that hosts the windows of UWP (Microsoft Store) apps. For those
/// apps the focused window belongs to this process, not to the real app.
pub const FRAME_HOST: &str = "applicationframehost.exe";

/// Normalized executable name of an app: just the file name, lowercase, with
/// `.exe` when there is no extension. `Browser`, `browser.exe` and
/// `C:\...\browser.exe` all become `browser.exe`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct AppName(String);

impl AppName {
    /// Normalizes a name or a path. `None` if nothing is left.
    pub fn new(text: &str) -> Option<Self> {
        let file = text.rsplit(['\\', '/']).next().unwrap_or(text).trim();
        if file.is_empty() {
            return None;
        }
        let mut name = file.to_lowercase();
        if !name.contains('.') {
            name.push_str(".exe");
        }
        Some(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_frame_host(&self) -> bool {
        self.0 == FRAME_HOST
    }
}

impl fmt::Display for AppName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The real app behind a window. If the window belongs to the UWP frame host,
/// the first child process that is not the host wins; with no child (the app
/// is still starting), the host stays.
pub fn hosted_app(window_app: AppName, children: impl IntoIterator<Item = AppName>) -> AppName {
    if !window_app.is_frame_host() {
        return window_app;
    }
    children
        .into_iter()
        .find(|child| !child.is_frame_host())
        .unwrap_or(window_app)
}

/// What the engine knows about the focused window.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Foreground {
    /// `None` when it could not be found out (for example, a protected system
    /// process).
    pub app: Option<AppName>,
    pub fullscreen: bool,
}

/// Whether gestures are on and, if not, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activity {
    Active,
    /// Paused from the tray.
    Paused,
    /// The focused app is on the exclusion list.
    Excluded(AppName),
    /// The focused app is fullscreen and the option to turn gestures off is on.
    Fullscreen,
}

impl Activity {
    pub fn is_active(&self) -> bool {
        *self == Activity::Active
    }
}

impl Config {
    /// First profile that lists the app.
    pub fn profile_for(&self, app: Option<&AppName>) -> Option<&Profile> {
        let app = app?;
        self.profiles.iter().find(|p| p.apps.contains(app))
    }

    /// Command for a gesture in the focused app: the profile's, if it changes
    /// that gesture, otherwise the one in `[gestures]`. `None` when the gesture
    /// asks for nothing (held still or gave up).
    pub fn gesture_command(&self, app: Option<&AppName>, outcome: Outcome) -> Option<&Command> {
        self.profile_for(app)
            .and_then(|p| p.gestures.command(outcome))
            .or_else(|| self.gestures.command(outcome))
    }

    pub fn is_excluded(&self, app: &AppName) -> bool {
        self.apps.excluded.contains(app)
    }

    /// Decides whether gestures are on. Pause beats exclusion, which beats
    /// fullscreen, so the tray shows the strongest reason.
    pub fn activity(&self, paused: bool, foreground: &Foreground) -> Activity {
        if paused {
            return Activity::Paused;
        }
        if let Some(app) = foreground.app.as_ref().filter(|a| self.is_excluded(a)) {
            return Activity::Excluded(app.clone());
        }
        if foreground.fullscreen && self.apps.disable_in_fullscreen {
            return Activity::Fullscreen;
        }
        Activity::Active
    }
}
