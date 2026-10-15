//! Fullscreen: decides whether the focused window covers the whole monitor.
//!
//! The engine gathers the facts (rectangles, shell state); the rule lives here
//! so it can be tested.

/// Rectangle in physical pixels, with exclusive `right` and `bottom`, as in
/// Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// `true` if this rectangle covers all of `other`.
    pub fn covers(&self, other: &Rect) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && self.right >= other.right
            && self.bottom >= other.bottom
    }
}

/// What the Windows shell says about the moment
/// (`SHQueryUserNotificationState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellState {
    Normal,
    /// A fullscreen app or presentation mode.
    Busy,
    /// A game in exclusive (Direct3D) fullscreen.
    ExclusiveFullscreen,
}

/// Facts about the focused window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFacts {
    pub rect: Rect,
    /// Monitor that holds most of the window.
    pub monitor: Rect,
    pub maximized: bool,
    /// A window of the Windows shell: the desktop, the taskbar, Task View,
    /// Alt+Tab or the switch between virtual desktops. Several of them cover
    /// the monitor for a moment, but none is an app in fullscreen.
    pub is_shell: bool,
}

/// Is the focused window fullscreen?
///
/// - A window of the Windows shell never is.
/// - Exclusive (Direct3D) fullscreen always is.
/// - Otherwise the window must cover the whole monitor. A maximized window
///   only counts if the shell also says it is busy: with an auto-hiding
///   taskbar, an ordinary maximized window covers the monitor.
pub fn is_fullscreen(shell: ShellState, window: Option<&WindowFacts>) -> bool {
    let Some(w) = window else {
        return shell == ShellState::ExclusiveFullscreen;
    };
    if w.is_shell {
        return false;
    }
    if shell == ShellState::ExclusiveFullscreen {
        return true;
    }
    w.rect.covers(&w.monitor) && (!w.maximized || shell == ShellState::Busy)
}
