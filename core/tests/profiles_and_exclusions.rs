//! Apps: executable names, per-app profiles, the exclusion list, when
//! gestures are on and the fullscreen rule.

use thumbring_core::apps::{Activity, AppName, Foreground, hosted_app};
use thumbring_core::command::Command;
use thumbring_core::config::Config;
use thumbring_core::gesture::{Direction, Outcome};
use thumbring_core::screen::{Rect, ShellState, WindowFacts, is_fullscreen};

fn app(name: &str) -> AppName {
    AppName::new(name).expect("valid name")
}

fn cmd(text: &str) -> Command {
    text.parse().expect("valid command")
}

fn config(text: &str) -> Config {
    let loaded = Config::from_toml(text).expect("valid TOML");
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    loaded.config
}

// ---------------------------------------------------------------------------
// Executable name
// ---------------------------------------------------------------------------

#[test]
fn app_names_are_normalized() {
    assert_eq!(app("browser.exe").as_str(), "browser.exe");
    assert_eq!(app("Browser.EXE").as_str(), "browser.exe");
    assert_eq!(app("browser").as_str(), "browser.exe");
    assert_eq!(app("  browser.exe ").as_str(), "browser.exe");
    assert_eq!(
        app(r"C:\Program Files\Browser\browser.exe").as_str(),
        "browser.exe"
    );
    assert_eq!(app("C:/Tools/Notepad++.exe").as_str(), "notepad++.exe");
    // Another extension stays as it is.
    assert_eq!(app("Saver.SCR").as_str(), "saver.scr");
}

#[test]
fn empty_app_names_are_rejected() {
    assert_eq!(AppName::new(""), None);
    assert_eq!(AppName::new("   "), None);
    assert_eq!(AppName::new(r"C:\folder\"), None);
}

#[test]
fn uwp_apps_are_found_behind_their_frame_host() {
    let host = app(r"C:\Windows\System32\ApplicationFrameHost.exe");
    assert!(host.is_frame_host());
    // The host window has a child owned by the real app's process.
    assert_eq!(
        hosted_app(
            host.clone(),
            [app("ApplicationFrameHost.exe"), app("CalculatorApp.exe")]
        ),
        app("calculatorapp.exe")
    );
    // No child (the app is still starting): the host stays.
    assert_eq!(hosted_app(host.clone(), []), host);
    // Ordinary window: children do not matter.
    assert_eq!(
        hosted_app(app("notepad.exe"), [app("other.exe")]),
        app("notepad.exe")
    );
}

// ---------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------

const WITH_PROFILES: &str = r#"
[[profiles]]
name = "Browsers"
apps = ["browser.exe", "OTHER-BROWSER"]
gestures = { left = "Ctrl+Shift+Tab", right = "Ctrl+Tab" }

[[profiles]]
apps = ["editor.exe"]
gestures = { click = "none", up = "Ctrl+Shift+P" }
"#;

#[test]
fn a_profile_changes_only_the_gestures_it_lists() {
    let c = config(WITH_PROFILES);
    let browser = app(r"C:\x\browser.exe");
    let left = Outcome::Swipe(Direction::Left);
    let up = Outcome::Swipe(Direction::Up);

    assert_eq!(
        c.gesture_command(Some(&browser), left),
        Some(&cmd("Ctrl+Shift+Tab"))
    );
    assert_eq!(
        c.gesture_command(Some(&browser), Outcome::Swipe(Direction::Right)),
        Some(&cmd("Ctrl+Tab"))
    );
    // Gestures the profile does not change come from [gestures].
    assert_eq!(c.gesture_command(Some(&browser), up), Some(&cmd("Win+Tab")));
    assert_eq!(
        c.gesture_command(Some(&browser), Outcome::Tap),
        Some(&Command::Native)
    );
    // The name in the file was normalized: "OTHER-BROWSER" matches other-browser.exe.
    assert_eq!(
        c.gesture_command(Some(&app("other-browser.exe")), left),
        Some(&cmd("Ctrl+Shift+Tab"))
    );
}

#[test]
fn apps_without_a_profile_use_the_global_gestures() {
    let c = config(WITH_PROFILES);
    let left = Outcome::Swipe(Direction::Left);
    assert_eq!(
        c.gesture_command(Some(&app("notepad.exe")), left),
        Some(&cmd("Ctrl+Win+Right"))
    );
    // Unknown app (the process name could not be read).
    assert_eq!(c.gesture_command(None, left), Some(&cmd("Ctrl+Win+Right")));
    assert_eq!(c.profile_for(None), None);
}

#[test]
fn none_turns_a_gesture_off_in_one_app() {
    let c = config(WITH_PROFILES);
    let editor = app("editor.exe");
    assert_eq!(
        c.gesture_command(Some(&editor), Outcome::Tap),
        Some(&Command::Nothing)
    );
    assert_eq!(
        c.gesture_command(Some(&editor), Outcome::Swipe(Direction::Up)),
        Some(&cmd("Ctrl+Shift+P"))
    );
}

#[test]
fn holding_still_or_giving_up_runs_nothing() {
    let c = config(WITH_PROFILES);
    for who in [None, Some(app("browser.exe")), Some(app("notepad.exe"))] {
        assert_eq!(c.gesture_command(who.as_ref(), Outcome::HeldStill), None);
        assert_eq!(c.gesture_command(who.as_ref(), Outcome::Cancelled), None);
    }
}

#[test]
fn the_first_profile_wins_when_an_app_is_repeated() {
    let loaded = Config::from_toml(
        r#"
        [[profiles]]
        apps = ["browser.exe"]
        gestures = { left = "Alt+Left" }

        [[profiles]]
        apps = ["Browser.exe", "other-browser.exe"]
        gestures = { left = "Ctrl+Shift+Tab" }
        "#,
    )
    .expect("valid TOML");
    let keys: Vec<_> = loaded.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(keys, ["profiles[1].apps"]);
    assert!(loaded.warnings[0].message.contains("profiles[0]"));

    let c = loaded.config;
    let left = Outcome::Swipe(Direction::Left);
    assert_eq!(
        c.gesture_command(Some(&app("browser.exe")), left),
        Some(&cmd("Alt+Left"))
    );
    assert_eq!(
        c.gesture_command(Some(&app("other-browser.exe")), left),
        Some(&cmd("Ctrl+Shift+Tab"))
    );
}

// ---------------------------------------------------------------------------
// Exclusion, pause and fullscreen
// ---------------------------------------------------------------------------

fn foreground(name: Option<&str>, fullscreen: bool) -> Foreground {
    Foreground {
        app: name.map(app),
        fullscreen,
    }
}

#[test]
fn gestures_are_active_by_default() {
    let c = Config::default();
    assert_eq!(
        c.activity(false, &foreground(Some("browser.exe"), false)),
        Activity::Active
    );
    assert_eq!(
        c.activity(false, &foreground(None, false)),
        Activity::Active
    );
    assert!(Activity::Active.is_active());
}

#[test]
fn an_excluded_app_turns_gestures_off() {
    let c = config(
        r#"
        [apps]
        excluded = ["mstsc.exe", "Game"]
        "#,
    );
    let mstsc = foreground(Some(r"C:\Windows\System32\MSTSC.EXE"), false);
    assert_eq!(
        c.activity(false, &mstsc),
        Activity::Excluded(app("mstsc.exe"))
    );
    assert_eq!(
        c.activity(false, &foreground(Some("game.exe"), false)),
        Activity::Excluded(app("game.exe"))
    );
    assert_eq!(
        c.activity(false, &foreground(Some("browser.exe"), false)),
        Activity::Active
    );
    assert!(!c.activity(false, &mstsc).is_active());
}

#[test]
fn fullscreen_turns_gestures_off_only_when_the_option_is_on() {
    let on = Config::default();
    assert!(on.apps.disable_in_fullscreen);
    let movie = foreground(Some("vlc.exe"), true);
    assert_eq!(on.activity(false, &movie), Activity::Fullscreen);

    let off = config("[apps]\ndisable_in_fullscreen = false");
    assert_eq!(off.activity(false, &movie), Activity::Active);
}

#[test]
fn pause_beats_exclusion_which_beats_fullscreen() {
    let c = config("[apps]\nexcluded = [\"game.exe\"]");
    let game = foreground(Some("game.exe"), true);
    assert_eq!(c.activity(true, &game), Activity::Paused);
    assert_eq!(
        c.activity(false, &game),
        Activity::Excluded(app("game.exe"))
    );
    assert_eq!(
        c.activity(false, &foreground(Some("other.exe"), true)),
        Activity::Fullscreen
    );
    assert_eq!(
        c.activity(true, &foreground(Some("other.exe"), false)),
        Activity::Paused
    );
}

// ---------------------------------------------------------------------------
// Fullscreen rule
// ---------------------------------------------------------------------------

const MONITOR: Rect = Rect::new(0, 0, 1920, 1080);

fn window(rect: Rect, maximized: bool) -> WindowFacts {
    WindowFacts {
        rect,
        monitor: MONITOR,
        maximized,
        is_shell: false,
    }
}

#[test]
fn a_borderless_window_covering_the_monitor_is_fullscreen() {
    let w = window(MONITOR, false);
    assert!(is_fullscreen(ShellState::Normal, Some(&w)));
    assert!(is_fullscreen(ShellState::Busy, Some(&w)));
}

#[test]
fn a_normal_maximized_window_is_not_fullscreen() {
    // Maximized with the taskbar visible: the borders go a bit past the
    // sides, but the taskbar stays out.
    let w = window(Rect::new(-8, -8, 1928, 1040), true);
    assert!(!is_fullscreen(ShellState::Normal, Some(&w)));
}

#[test]
fn maximized_over_a_hidden_taskbar_only_counts_if_the_shell_is_busy() {
    let w = window(Rect::new(-8, -8, 1928, 1088), true);
    assert!(!is_fullscreen(ShellState::Normal, Some(&w)));
    assert!(is_fullscreen(ShellState::Busy, Some(&w)));
}

/// The desktop after Win+D, Task View, or the window the shell puts in front
/// while it switches virtual desktops: they cover the monitor, but turning
/// gestures off there would only get in the way of the next gesture.
#[test]
fn a_shell_window_is_never_fullscreen() {
    let desktop = WindowFacts {
        is_shell: true,
        ..window(MONITOR, false)
    };
    assert!(!is_fullscreen(ShellState::Normal, Some(&desktop)));
    assert!(!is_fullscreen(ShellState::Busy, Some(&desktop)));
    assert!(!is_fullscreen(
        ShellState::ExclusiveFullscreen,
        Some(&desktop)
    ));
}

#[test]
fn exclusive_fullscreen_is_fullscreen_whatever_the_rectangle() {
    let small = window(Rect::new(0, 0, 800, 600), false);
    assert!(is_fullscreen(ShellState::ExclusiveFullscreen, Some(&small)));
    assert!(is_fullscreen(ShellState::ExclusiveFullscreen, None));
    assert!(!is_fullscreen(ShellState::Normal, None));
    assert!(!is_fullscreen(ShellState::Busy, None));
}

#[test]
fn a_window_a_bit_smaller_than_the_monitor_is_not_fullscreen() {
    let w = window(Rect::new(0, 0, 1920, 1079), false);
    assert!(!is_fullscreen(ShellState::Normal, Some(&w)));
    assert!(!is_fullscreen(ShellState::Busy, Some(&w)));
}

#[test]
fn fullscreen_on_a_second_monitor_uses_that_monitor() {
    let second = Rect::new(1920, -200, 1920 + 2560, 1240);
    let w = WindowFacts {
        rect: second,
        monitor: second,
        maximized: false,
        is_shell: false,
    };
    assert!(is_fullscreen(ShellState::Normal, Some(&w)));
    // Covering the first monitor is not covering the second.
    let wrong = WindowFacts { rect: MONITOR, ..w };
    assert!(!is_fullscreen(ShellState::Normal, Some(&wrong)));
}
