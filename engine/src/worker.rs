//! Worker thread: owns all of the engine's state.
//!
//! It receives, over a channel, the hook events, focus changes, reloaded
//! settings and tray requests. It decides whether gestures are on, recognizes
//! gestures and runs commands. It is the only thread that runs actions: the
//! hook callback only decides and forwards, and the ring thread sends back the
//! command picked on the ring.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use thumbring_core::apps::{Activity, AppName, Foreground};
use thumbring_core::command::Command;
use thumbring_core::config::{Config, MouseButton};
use thumbring_core::gesture::{Direction, Outcome, Point, Tracker};

use crate::foreground::{self, Resolver};
use crate::hook::{self, HookEvent};
use crate::log::{error, info, warning};
use crate::ring::controller::{self, Input};
use crate::{actions, dpi, tray};

pub enum Msg {
    Hook(HookEvent),
    /// The focused window changed (the HWND as a number).
    Foreground(usize),
    Config(Box<Config>),
    /// Command picked on the ring (or "native", for a quick tap), to run now.
    /// `button` is the one that opened the ring: "native" sends its click.
    RingCommand {
        command: Command,
        button: MouseButton,
    },
    TogglePause,
    Quit,
}

/// How often to re-check fullscreen (a video going fullscreen does not change
/// the focused window) and the focused window itself, in case a notification
/// was missed: Windows does not always report focus coming back to an app
/// after the taskbar or a virtual desktop switch had it for a moment.
const POLL: Duration = Duration::from_millis(250);

/// Gesture in progress, from pressing the gesture button to releasing it.
struct Pressed {
    /// The button held: "native" sends its click.
    button: MouseButton,
    tracker: Tracker,
    down_time: u32,
    /// Were gestures on at the press? If not, the hook had not been removed
    /// yet when the button was pressed, and the button should have worked as
    /// usual: on release, the native click is sent again.
    active: bool,
    /// The profile follows the app where the gesture started.
    app: Option<AppName>,
}

struct Worker {
    config: Config,
    paused: bool,
    window: usize,
    foreground: Foreground,
    resolver: Resolver,
    /// `None` until the first decision.
    activity: Option<Activity>,
    pressed: Option<Pressed>,
    /// `None` if the ring could not start: the ring button keeps its own
    /// function.
    ring: Option<controller::Handle>,
}

pub fn run(events: Receiver<Msg>, config: Config, ring: Option<controller::Handle>) {
    let mut worker = Worker {
        config,
        paused: false,
        window: 0,
        foreground: Foreground::default(),
        resolver: Resolver::new(),
        activity: None,
        pressed: None,
        ring,
    };
    worker.assign_buttons();
    worker.enable_ring();
    worker.refresh(foreground::current());
    loop {
        match events.recv_timeout(POLL) {
            Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(msg) => worker.handle(msg),
            Err(RecvTimeoutError::Timeout) => worker.poll(),
        }
    }
}

impl Worker {
    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Hook(event) => self.on_hook(event),
            Msg::Foreground(window) => self.refresh(window),
            Msg::Config(config) => {
                let buttons = |c: &Config| (c.gestures.button, c.ring.button);
                let new_buttons = buttons(&config) != buttons(&self.config);
                self.config = *config;
                if new_buttons {
                    self.assign_buttons();
                }
                self.send_ring(Input::Settings(Box::new(controller::Settings::of(
                    &self.config,
                ))));
                self.enable_ring();
                // The exclusion list and the fullscreen option may have changed.
                self.refresh(self.window);
            }
            Msg::RingCommand { command, button } => execute(&command, button),
            Msg::TogglePause => {
                self.paused = !self.paused;
                self.refresh(foreground::current());
            }
            Msg::Quit => {}
        }
    }

    fn poll(&mut self) {
        let window = foreground::current();
        // A UWP app that is still starting shows up as its frame host until
        // its own child window appears: keep asking until then.
        let unresolved = self
            .foreground
            .app
            .as_ref()
            .is_some_and(AppName::is_frame_host);
        if window != self.window || unresolved {
            self.refresh(window);
        } else if self.config.apps.disable_in_fullscreen && !self.paused {
            let fullscreen = foreground::is_fullscreen(window);
            if fullscreen != self.foreground.fullscreen {
                self.foreground.fullscreen = fullscreen;
                self.update_activity();
            }
        }
    }

    /// Reads who has focus again and decides again.
    fn refresh(&mut self, window: usize) {
        // The tray menu belongs to the engine itself (and has focus while it is
        // open): it does not change the focused app, but pausing must still
        // take effect.
        if !self.resolver.is_own(window) {
            if window != self.window {
                self.send_ring(Input::FocusLost);
            }
            self.window = window;
            let app = self.resolver.app_of(window);
            if app != self.foreground.app {
                // Also helps to find the .exe name for the settings file.
                let name = app.as_ref().map_or("unknown app", AppName::as_str);
                info!("focus: {name}");
            }
            self.foreground = Foreground {
                app,
                fullscreen: self.config.apps.disable_in_fullscreen
                    && foreground::is_fullscreen(window),
            };
        }
        self.update_activity();
    }

    fn update_activity(&mut self) {
        let activity = self.config.activity(self.paused, &self.foreground);
        if self.activity.as_ref() == Some(&activity) {
            return;
        }
        info!("gestures {}", describe(&activity));
        hook::set_enabled(activity.is_active());
        if !activity.is_active() {
            self.send_ring(Input::FocusLost);
        }
        tray::show(activity.clone());
        self.activity = Some(activity);
    }

    fn on_hook(&mut self, event: HookEvent) {
        match event {
            HookEvent::GestureDown { button, pos, time } => {
                // The gesture belongs to the app focused right now, even if
                // Windows has not reported the last focus change yet.
                let window = foreground::current();
                if window != self.window {
                    self.refresh(window);
                }
                let threshold = f64::from(self.config.gestures.threshold_px) * dpi::scale_at(pos);
                self.pressed = Some(Pressed {
                    button,
                    tracker: Tracker::new(pos, threshold),
                    down_time: time,
                    active: self.activity.as_ref().is_some_and(Activity::is_active),
                    app: self.foreground.app.clone(),
                });
            }
            HookEvent::Move { pos } => {
                if let Some(p) = self.pressed.as_mut() {
                    p.tracker.moved(pos);
                }
                self.send_ring(Input::Moved(pos));
            }
            HookEvent::GestureUp { pos, time } => {
                if let Some(p) = self.pressed.take() {
                    self.release(p, pos, time);
                }
            }
            HookEvent::TriggerDown { button } => self.send_ring(Input::TriggerDown(button)),
            HookEvent::TriggerUp => self.send_ring(Input::TriggerUp),
            HookEvent::Click { button, pos } => self.send_ring(Input::Click { button, pos }),
            HookEvent::Esc => self.send_ring(Input::Esc),
            HookEvent::Installed => info!("mouse hook installed again"),
            HookEvent::Removed => info!("mouse hook removed: the button works as usual"),
            HookEvent::InstallFailed(code) => {
                error!("Windows refused to install the mouse hook again (0x{code:08X})")
            }
        }
    }

    fn send_ring(&self, input: Input) {
        if let Some(ring) = &self.ring {
            ring.send(input);
        }
    }

    /// Tells the hook which buttons do what. It takes them at the next press
    /// with no button held and the ring closed.
    fn assign_buttons(&self) {
        let (gestures, ring) = (self.config.gestures.button, self.config.ring.button);
        info!("buttons: gestures = {gestures}, ring = {ring}");
        hook::set_buttons(gestures, ring);
    }

    /// The ring button opens the ring only when the ring has items.
    fn enable_ring(&self) {
        hook::set_ring_enabled(self.ring.is_some() && !self.config.ring.is_empty());
    }

    fn release(&self, p: Pressed, pos: Point, time: u32) {
        let origin = p.tracker.origin();
        // `time` comes from the system clock in ms, which wraps every ~49 days.
        let held_ms = time.wrapping_sub(p.down_time);
        let outcome = p
            .tracker
            .release(pos, held_ms, self.config.behavior.hold_to_open_ms);
        let app = p.app.as_ref().map_or("unknown app", AppName::as_str);

        if !p.active {
            info!("button pressed while gestures were off: native click sent again");
            execute(&Command::Native, p.button);
            return;
        }
        let (dx, dy) = (pos.x - origin.x, pos.y - origin.y);
        match self.config.gesture_command(p.app.as_ref(), outcome) {
            Some(command) => {
                info!(
                    "{} in {app}: {command}  (dx {dx:+}, dy {dy:+}, {held_ms} ms)",
                    label(outcome)
                );
                execute(command, p.button);
            }
            None => info!("{} in {app} ({held_ms} ms)", label(outcome)),
        }
    }
}

/// `button` is the one that asked for the command: "native" sends its click.
fn execute(command: &Command, button: MouseButton) {
    if let Err(e) = actions::run(command, button) {
        warning!("the command \"{command}\" failed: {e}");
    }
}

fn describe(activity: &Activity) -> String {
    match activity {
        Activity::Active => "on".into(),
        Activity::Paused => "paused from the tray".into(),
        Activity::Excluded(app) => format!("off in {app} (exclusion list)"),
        Activity::Fullscreen => "off: app in fullscreen".into(),
    }
}

fn label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Tap => "quick tap",
        Outcome::Swipe(Direction::Up) => "gesture up",
        Outcome::Swipe(Direction::Down) => "gesture down",
        Outcome::Swipe(Direction::Left) => "gesture left",
        Outcome::Swipe(Direction::Right) => "gesture right",
        Outcome::HeldStill => "held without dragging, nothing to do",
        Outcome::Cancelled => "came back to the start, gesture cancelled",
    }
}
