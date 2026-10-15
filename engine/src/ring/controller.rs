//! The ring thread: owns the state machine, the overlay window and the timers.
//!
//! The worker forwards the trigger, the clicks, Esc, the cursor moves and the
//! focus changes through a [`Handle`]. The controller hit-tests the cursor (on
//! every move and on every frame, since folder items grow into place), feeds
//! the machine (`thumbring_core::ring_machine`) and carries out its effects:
//! it opens and closes the window, starts the timers and picks the command.
//!
//! It draws only while something moves, one frame per screen refresh. The
//! thread waits for the swap chain, its timers and its messages all at once,
//! so a click or a timer is handled on time even in the middle of an
//! animation. When nothing moves, it sleeps until a message or a timer.
//!
//! The command goes back to the worker, which runs every action, once the
//! cleared window is on the screen and hidden: a screenshot shortcut must not
//! catch the ring.

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use thumbring_core::anim::Timing;
use thumbring_core::command::Command;
use thumbring_core::config::{Animation, Behavior, Config};
use thumbring_core::gesture::Point;
use thumbring_core::layout::{Geometry, OpenFolder, Vec2};
use thumbring_core::ring::{Action, Ring, Slot};
use thumbring_core::ring_machine::{CloseReason, Effect, Event, Phase, State, Target, reduce};
use thumbring_core::screen::Rect;
use thumbring_core::view::RingView;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM,
};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, GetCurrentThreadId, INFINITE,
    SetWaitableTimer, TIMER_ALL_ACCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_NOREMOVE,
    PM_REMOVE, PeekMessageW, PostThreadMessageW, QS_ALLINPUT, SPI_GETCLIENTAREAANIMATION,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW, TranslateMessage, WM_APP, WM_QUIT,
    WM_USER,
};
use windows::core::{BOOL, PCWSTR};

use super::overlay::Overlay;
use super::render::label_text;
use crate::dpi;
use crate::hook::{self, Button};
use crate::log::{info, warning};
use crate::worker::Msg;

/// Thread message: "there is something in the channel".
const WAKE: u32 = WM_APP;

/// If the swap chain does not ask for a frame within this time, the frame is
/// drawn anyway, so the ring can never get stuck on the screen.
const FRAME_WAIT_LIMIT_MS: f64 = 250.0;

/// A frame is late when the gap since the previous one is longer than this
/// many screen refreshes: it missed at least one.
const LATE_GAP: f64 = 1.5;

/// A gap of this many screen refreshes is a stutter anyone would notice.
const LONG_GAP: f64 = 4.0;

/// What the ring needs from the settings file.
pub struct Settings {
    pub ring: Ring,
    pub behavior: Behavior,
    pub animation: Animation,
}

impl Settings {
    pub fn of(config: &Config) -> Self {
        Self {
            ring: config.ring.clone(),
            behavior: config.behavior.clone(),
            animation: config.animation.clone(),
        }
    }
}

/// What the worker sends to the ring.
pub enum Input {
    TriggerDown,
    TriggerUp,
    Moved(Point),
    /// A click the hook blocked because the ring is open.
    Click {
        button: Button,
        pos: Point,
    },
    Esc,
    /// Another window got the focus, or the engine was turned off.
    FocusLost,
    Settings(Box<Settings>),
}

/// Flags the worker and the ring thread share.
#[derive(Default)]
struct Shared {
    /// The machine is past Idle: cursor moves matter.
    listening: AtomicBool,
    /// A wake-up message is already on its way.
    woken: AtomicBool,
}

/// Sends input to the ring thread. Cheap to clone.
#[derive(Clone)]
pub struct Handle {
    inputs: Sender<Input>,
    thread: u32,
    shared: Arc<Shared>,
}

impl Handle {
    pub fn send(&self, input: Input) {
        // Cursor moves only matter from the trigger press until the ring
        // closes; during a gesture they would only wake the thread.
        if matches!(input, Input::Moved(_)) && !self.shared.listening.load(Ordering::Relaxed) {
            return;
        }
        if self.inputs.send(input).is_ok() && !self.shared.woken.swap(true, Ordering::AcqRel) {
            post(self.thread, WAKE);
        }
    }

    /// Asks the ring thread to close its window and end.
    pub fn stop(&self) {
        post(self.thread, WM_QUIT);
    }
}

fn post(thread: u32, message: u32) {
    // SAFETY: only posts a message; an invalid id just returns an error.
    let _ = unsafe { PostThreadMessageW(thread, message, WPARAM(0), LPARAM(0)) };
}

/// Starts the ring thread. Fails if the window or the GPU device cannot be
/// created; the engine then goes on without the ring.
pub fn spawn(
    settings: Settings,
    worker: Sender<Msg>,
) -> Result<(Handle, JoinHandle<()>), Box<dyn Error>> {
    let (inputs, inbox) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let shared = Arc::new(Shared::default());
    let thread_shared = shared.clone();
    let thread = thread::Builder::new()
        .name("ring".into())
        .spawn(move || run(inbox, settings, worker, thread_shared, ready_tx))?;
    let id = ready_rx.recv()??;
    Ok((
        Handle {
            inputs,
            thread: id,
            shared,
        },
        thread,
    ))
}

fn run(
    inbox: Receiver<Input>,
    settings: Settings,
    worker: Sender<Msg>,
    shared: Arc<Shared>,
    ready: Sender<Result<u32, String>>,
) {
    let mut msg = MSG::default();
    // SAFETY: makes sure the message queue exists before publishing the thread
    // id; without a queue, a message posted early would be lost.
    let id = unsafe {
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
        GetCurrentThreadId()
    };
    // The window and the shader are created up front, so the first opening
    // does not wait for them. They start on the primary monitor, whose
    // top-left corner is (0, 0).
    let origin = Point::new(0, 0);
    let overlay =
        dpi::monitor_at(origin).and_then(|monitor| Overlay::create(monitor, dpi::scale_at(origin)));
    let mut overlay = match overlay {
        Ok(overlay) => overlay,
        Err(e) => {
            let _ = ready.send(Err(format!("could not create the ring window: {e}")));
            return;
        }
    };
    let _ = ready.send(Ok(id));
    warm_up(&mut overlay, &settings.ring);

    let mut controller = Controller {
        settings,
        next_settings: None,
        overlay: Some(overlay),
        worker,
        shared,
        alarm: Alarm::new(),
        start: Instant::now(),
        state: State::default(),
        open: None,
        hold_at: None,
        intent: None,
        pending: None,
        cursor: origin,
        redraw: false,
        frame_ready: false,
    };
    controller.run(&inbox);
}

/// A ring on the screen, from its opening until its close animation ends.
struct Open {
    view: RingView,
    geometry: Geometry,
    timing: Timing,
    /// Top-left of the monitor: the window coordinates start there.
    origin: Vec2,
    /// Last target sent to the machine.
    target: Option<Target>,
    /// The window is showing. It shows once the first frame of this ring is
    /// presented, so it never puts an older frame on the screen.
    shown: bool,
    /// The close animation is over and the window was cleared. It hides once
    /// that empty frame is on the screen.
    cleared: bool,
    frames: FrameStats,
}

/// How smoothly one ring was drawn, for the log.
struct FrameStats {
    /// Time between two screen refreshes.
    period: f64,
    count: u32,
    late: u32,
    worst_gap: f64,
    /// When the previous frame was drawn, while the animation goes on without
    /// a pause.
    last: Option<f64>,
}

impl FrameStats {
    fn new(period: f64) -> Self {
        Self {
            period,
            count: 0,
            late: 0,
            worst_gap: 0.0,
            last: None,
        }
    }

    /// A frame drawn at `now`; `goes_on` if another one follows right away.
    fn record(&mut self, now: f64, goes_on: bool) {
        self.count += 1;
        if let Some(last) = self.last {
            let gap = now - last;
            if gap > LATE_GAP * self.period {
                self.late += 1;
            }
            self.worst_gap = self.worst_gap.max(gap);
        }
        self.last = goes_on.then_some(now);
    }

    /// The animation visibly stuttered: frames were late often (more than one
    /// in 20), or one gap was long. One missed refresh now and then cannot be
    /// seen.
    fn stuttered(&self) -> bool {
        self.late * 20 > self.count || self.worst_gap > LONG_GAP * self.period
    }

    /// Writes to the log only when the animation stuttered.
    fn report(&self) {
        if self.stuttered() {
            info!(
                "ring: {} of {} frames were late (worst gap {:.0} ms; the screen refreshes every {:.1} ms)",
                self.late, self.count, self.worst_gap, self.period
            );
        }
    }
}

struct Controller {
    settings: Settings,
    /// Settings that arrived while the ring was in use. They apply once it is
    /// closed, so the items never change under the cursor.
    next_settings: Option<Settings>,
    /// `None` after a drawing error: a new window (and GPU device) is created
    /// on the next opening.
    overlay: Option<Overlay>,
    worker: Sender<Msg>,
    shared: Arc<Shared>,
    alarm: Option<Alarm>,
    start: Instant,
    state: State,
    open: Option<Open>,
    /// When the hold timer fires, in ms since `start`.
    hold_at: Option<f64>,
    /// The path tolerance timer: slot and when it fires.
    intent: Option<(u8, f64)>,
    /// Command picked on the ring, to run once the ring is off the screen.
    pending: Option<Command>,
    cursor: Point,
    /// Something changed on the screen: draw a frame.
    redraw: bool,
    /// The swap chain asked for a frame, and none was drawn since.
    frame_ready: bool,
}

impl Controller {
    fn run(&mut self, inbox: &Receiver<Input>) {
        loop {
            if !pump_messages() {
                return;
            }
            self.shared.woken.store(false, Ordering::Release);
            loop {
                match inbox.try_recv() {
                    Ok(input) => self.handle(input),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            let now = self.now();
            self.fire_timers(now);
            self.refresh_target(now);
            if self.redraw && (self.frame_ready || self.overlay.is_none()) {
                self.frame(self.now());
            } else {
                self.wait(now);
            }
        }
    }

    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }

    fn handle(&mut self, input: Input) {
        let now = self.now();
        match input {
            Input::TriggerDown => self.dispatch(Event::TriggerDown, now),
            Input::TriggerUp => self.dispatch(Event::TriggerUp, now),
            Input::Moved(pos) => self.cursor = pos,
            Input::Click { button, pos } => {
                if !self.state.is_open() {
                    return;
                }
                self.cursor = pos;
                let target = match button {
                    Button::Left => self.hit(now),
                    // Only the left button picks. The others count as a click
                    // outside the ring, which closes it.
                    Button::Right | Button::Middle => Target::Outside,
                };
                self.dispatch(Event::Click(target), now);
            }
            Input::Esc => self.dispatch(Event::Esc, now),
            Input::FocusLost => self.dispatch(Event::FocusLost, now),
            Input::Settings(settings) => {
                self.next_settings = Some(*settings);
                self.apply_settings();
            }
        }
    }

    fn dispatch(&mut self, event: Event, now: f64) {
        let (state, effects) = reduce(self.state, event, &self.settings.behavior);
        self.state = state;
        for effect in effects {
            self.carry_out(effect, now);
        }
        if let Some(open) = self.open.as_mut() {
            open.view.sync(&self.state, now, &open.timing);
        }
        self.shared
            .listening
            .store(self.state.phase != Phase::Idle, Ordering::Relaxed);
        self.redraw = true;
        self.apply_settings();
    }

    fn carry_out(&mut self, effect: Effect, now: f64) {
        let behavior = &self.settings.behavior;
        match effect {
            Effect::OpenRing => self.open_ring(now),
            Effect::CloseRing(reason) => {
                hook::set_ring_open(false);
                self.intent = None;
                if reason != CloseReason::Execute {
                    info!("ring closed: {}", describe(reason));
                }
            }
            Effect::StartHold => self.hold_at = Some(now + f64::from(behavior.hold_to_open_ms)),
            Effect::CancelHold => self.hold_at = None,
            Effect::StartIntent(Target::Action { slot } | Target::Folder { slot }) => {
                self.intent = Some((slot, now + f64::from(behavior.path_tolerance_ms)));
            }
            Effect::CancelIntent => self.intent = None,
            Effect::PassNative => {
                info!("quick tap on Forward: native click sent again");
                let _ = self.worker.send(Msg::RingCommand(Command::Native));
            }
            Effect::Execute(target) => match action_of(&self.settings.ring, target) {
                Some(Action {
                    name,
                    command: Some(command),
                    ..
                }) => {
                    info!("ring: {name} ({command})");
                    self.pending = Some(command.clone());
                }
                Some(action) => info!("ring: {} has no command", action.name),
                None => {}
            },
            _ => {}
        }
        if let Some(open) = self.open.as_mut() {
            open.view.apply(&effect, now, &open.timing);
        }
    }

    fn open_ring(&mut self, now: f64) {
        // A ring still closing: its command runs before the new one opens.
        if self.open.is_some() {
            self.finish();
        }
        let cursor = super::cursor();
        self.cursor = cursor;
        let scale = dpi::scale_at(cursor);
        let shown = dpi::monitor_at(cursor).and_then(|monitor| {
            self.cover(monitor, scale)?;
            Ok(monitor)
        });
        let monitor = match shown {
            Ok(monitor) => monitor,
            Err(e) => {
                warning!("could not show the ring: {e}");
                self.overlay = None;
                // Back to idle without drawing anything.
                self.state = State {
                    trigger_held: self.state.trigger_held,
                    ..State::default()
                };
                return;
            }
        };
        let geometry = Geometry::for_monitor(scale, monitor);
        let origin = Vec2::new(f64::from(monitor.left), f64::from(monitor.top));
        let point = Vec2::new(f64::from(cursor.x), f64::from(cursor.y));
        let center = geometry.place(point, monitor) - origin;
        let period = self
            .overlay
            .as_ref()
            .map_or(1000.0 / 60.0, |o| o.renderer.refresh_period_ms());
        self.open = Some(Open {
            view: RingView::open(center, now),
            geometry,
            timing: Timing::new(&self.settings.animation, reduced_motion()),
            origin,
            target: None,
            shown: false,
            cleared: false,
            frames: FrameStats::new(period),
        });
        hook::set_ring_open(true);
    }

    /// Makes the window cover `monitor`, creating it again if a drawing error
    /// dropped it.
    fn cover(&mut self, monitor: Rect, scale: f64) -> windows::core::Result<()> {
        match self.overlay.as_mut() {
            Some(overlay) => overlay.cover(monitor, scale),
            None => {
                let mut overlay = Overlay::create(monitor, scale)?;
                warm_up(&mut overlay, &self.settings.ring);
                self.overlay = Some(overlay);
                // A new swap chain: wait for it to ask for a frame.
                self.frame_ready = false;
                Ok(())
            }
        }
    }

    /// What is under the cursor.
    fn hit(&self, now: f64) -> Target {
        let Some(open) = &self.open else {
            return Target::Outside;
        };
        let s = &self.settings;
        let cursor = Vec2::new(f64::from(self.cursor.x), f64::from(self.cursor.y));
        let offset = cursor - open.origin - open.view.center;
        let children = open.view.open_folder_children(
            &s.ring,
            &open.geometry,
            &s.animation,
            &open.timing,
            now,
        );
        let folder = children.as_ref().map(|(slot, appear)| OpenFolder {
            slot: *slot,
            appear,
        });
        open.geometry.hit_test(&s.ring, offset, folder)
    }

    /// Tells the machine when the cursor points at something else.
    fn refresh_target(&mut self, now: f64) {
        let ready =
            self.state.is_open() && self.open.as_ref().is_some_and(|o| !o.view.is_closing());
        if !ready {
            return;
        }
        let target = self.hit(now);
        let Some(open) = self.open.as_mut() else {
            return;
        };
        if open.target != Some(target) {
            open.target = Some(target);
            self.dispatch(Event::Pointer(target), now);
        }
    }

    fn fire_timers(&mut self, now: f64) {
        if self.hold_at.is_some_and(|at| at <= now) {
            self.hold_at = None;
            self.dispatch(Event::HoldElapsed, now);
        }
        if let Some((slot, at)) = self.intent {
            if at <= now {
                self.intent = None;
                self.dispatch(Event::IntentElapsed { slot }, now);
            }
        }
    }

    fn next_deadline(&self) -> Option<f64> {
        [self.hold_at, self.intent.map(|(_, at)| at)]
            .into_iter()
            .flatten()
            .reduce(f64::min)
    }

    /// Sleeps until a message arrives, the next timer is due or, with a frame
    /// to draw, the swap chain asks for it.
    fn wait(&mut self, now: f64) {
        let mut handles: Vec<HANDLE> = Vec::with_capacity(2);
        let mut timeout = INFINITE;
        let frame = match &self.overlay {
            Some(overlay) if self.redraw && !self.frame_ready => {
                handles.push(overlay.renderer.frame_waiter());
                timeout = FRAME_WAIT_LIMIT_MS as u32;
                true
            }
            _ => false,
        };
        if let Some(at) = self.next_deadline() {
            let ms = (at - now).max(0.0);
            match &self.alarm {
                Some(alarm) if alarm.set(ms) => handles.push(alarm.0),
                _ => timeout = timeout.min(ms.ceil() as u32),
            }
        }
        // SAFETY: the handles are live: the swap chain's (owned by the
        // overlay) and the timer owned by the controller.
        let woke = unsafe {
            MsgWaitForMultipleObjectsEx(Some(&handles), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        };
        // The wait on the swap chain takes the frame: one must be drawn now.
        let limit = woke == WAIT_TIMEOUT && self.now() - now >= FRAME_WAIT_LIMIT_MS;
        if frame && (woke == WAIT_OBJECT_0 || limit) {
            self.frame_ready = true;
        }
    }

    /// The swap chain takes a frame: draws the next one or, once the cleared
    /// frame is on the screen, hides the window.
    ///
    /// Only a presented frame uses up the one the swap chain gave. Otherwise
    /// the next wait would be for a frame the swap chain never gives again,
    /// and the ring would show up only when the wait limit runs out, with its
    /// opening animation over (the trigger press asks for a redraw before
    /// there is a ring to draw).
    fn frame(&mut self, now: f64) {
        self.redraw = false;
        let Some(open) = &self.open else {
            return;
        };
        if open.cleared {
            if let Some(overlay) = &self.overlay {
                overlay.hide();
            }
            self.finish();
            return;
        }
        self.frame_ready = false;
        self.draw(now);
    }

    fn draw(&mut self, now: f64) {
        self.redraw = false;
        let (Some(open), Some(overlay)) = (self.open.as_mut(), self.overlay.as_mut()) else {
            return;
        };
        let s = &self.settings;
        let scene = open
            .view
            .scene(&s.ring, &open.geometry, &s.animation, &open.timing, now);
        let label = scene
            .as_ref()
            .and_then(|scene| scene.label.as_ref())
            .and_then(|l| label_text(&s.ring, l.target));
        // With no scene, the close animation is over: the window is cleared,
        // so it does not flash the last frame the next time it shows.
        let drawn = overlay
            .renderer
            .draw(scene.as_ref(), &s.ring, open.geometry.d, label.as_ref());
        match (drawn, scene) {
            (Ok(()), Some(scene)) => {
                if !open.shown {
                    overlay.show();
                    open.shown = true;
                }
                open.frames.record(now, scene.active);
                self.redraw = scene.active;
            }
            (Ok(()), None) => {
                open.cleared = true;
                self.redraw = true;
            }
            (Err(e), _) => {
                warning!("could not draw the ring: {e}");
                self.overlay = None;
                self.frame_ready = false;
                self.abandon();
            }
        }
    }

    /// Takes the ring off the screen at once, after a drawing error.
    fn abandon(&mut self) {
        if self.state.is_open() {
            hook::set_ring_open(false);
        }
        self.state = State {
            trigger_held: self.state.trigger_held,
            ..State::default()
        };
        self.hold_at = None;
        self.intent = None;
        self.shared.listening.store(false, Ordering::Relaxed);
        self.finish();
    }

    /// The ring is off the screen: runs the command picked on it and applies
    /// the settings that were waiting.
    fn finish(&mut self) {
        if let Some(open) = self.open.take() {
            open.frames.report();
        }
        if let Some(command) = self.pending.take() {
            let _ = self.worker.send(Msg::RingCommand(command));
        }
        self.apply_settings();
    }

    fn apply_settings(&mut self) {
        if self.state.phase == Phase::Idle && self.open.is_none() {
            if let Some(settings) = self.next_settings.take() {
                self.settings = settings;
                if let Some(overlay) = self.overlay.as_mut() {
                    warm_up(overlay, &self.settings.ring);
                }
            }
        }
    }
}

/// Prepares the icons for the monitor the window covers.
fn warm_up(overlay: &mut Overlay, ring: &Ring) {
    let (monitor, scale) = overlay.placement();
    let d = Geometry::for_monitor(scale, monitor).d;
    if let Err(e) = overlay.renderer.warm_up(ring, d) {
        warning!("could not prepare the ring icons: {e}");
    }
}

/// The action of a target that can run.
fn action_of(ring: &Ring, target: Target) -> Option<&Action> {
    match target {
        Target::Action { slot } => match ring.slots.get(usize::from(slot))? {
            Slot::Action(action) => Some(action),
            _ => None,
        },
        Target::Child { slot, index } => match ring.slots.get(usize::from(slot))? {
            Slot::Folder { items, .. } => items.get(usize::from(index)),
            _ => None,
        },
        _ => None,
    }
}

fn describe(reason: CloseReason) -> &'static str {
    match reason {
        CloseReason::Center => "center",
        CloseReason::Execute => "action",
        CloseReason::Outside => "click outside",
        CloseReason::Esc => "Esc",
        CloseReason::FocusLost => "focus changed",
        CloseReason::Trigger => "trigger pressed again",
    }
}

/// Windows "Animation effects" setting is off.
fn reduced_motion() -> bool {
    let mut on = BOOL(1);
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL into `on`.
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut on as *mut BOOL as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.is_ok() && !on.as_bool()
}

/// Handles the window messages; `false` once the thread was asked to stop.
fn pump_messages() -> bool {
    let mut msg = MSG::default();
    // SAFETY: standard message pump with a local variable.
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                return false;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    true
}

/// One-shot timer with millisecond precision. A plain wait timeout follows
/// the system tick (about 16 ms), too coarse for the 50 ms path tolerance.
struct Alarm(HANDLE);

impl Alarm {
    fn new() -> Option<Self> {
        // SAFETY: creates an unnamed timer; it is closed in `drop`.
        let timer = unsafe {
            CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
        };
        timer.ok().map(Self)
    }

    /// Fires in `ms` milliseconds; replaces any earlier time.
    fn set(&self, ms: f64) -> bool {
        // Negative means relative, in units of 100 ns.
        let due = -((ms * 10_000.0) as i64).max(1);
        // SAFETY: `self.0` is a live timer handle.
        unsafe { SetWaitableTimer(self.0, &due, 0, None, None, false) }.is_ok()
    }
}

impl Drop for Alarm {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames drawn at these times, each followed right away by another.
    fn frames(period: f64, times: &[f64]) -> FrameStats {
        let mut stats = FrameStats::new(period);
        for &t in times {
            stats.record(t, true);
        }
        stats
    }

    #[test]
    fn a_frame_that_missed_a_refresh_is_late() {
        let stats = frames(10.0, &[0.0, 10.0, 20.0, 40.0, 50.0]);
        assert_eq!(stats.count, 5);
        assert_eq!(stats.late, 1);
        assert_eq!(stats.worst_gap, 20.0);
    }

    #[test]
    fn a_pause_in_the_animation_is_not_a_late_frame() {
        let mut stats = FrameStats::new(10.0);
        stats.record(0.0, true);
        stats.record(10.0, false);
        // The ring sat still; the next frame comes from a cursor move.
        stats.record(500.0, true);
        stats.record(510.0, true);
        assert_eq!(stats.late, 0);
        assert_eq!(stats.worst_gap, 10.0);
    }

    #[test]
    fn one_missed_refresh_now_and_then_is_not_a_stutter() {
        let mut times: Vec<f64> = (0..40).map(|i| f64::from(i) * 10.0).collect();
        times.push(410.0);
        let stats = frames(10.0, &times);
        assert_eq!(stats.late, 1);
        assert!(!stats.stuttered());
    }

    #[test]
    fn frequent_or_long_gaps_are_a_stutter() {
        let often = frames(10.0, &[0.0, 20.0, 40.0, 60.0]);
        assert!(often.stuttered());
        // One late frame in 41, but it took five refreshes.
        let mut times: Vec<f64> = (0..40).map(|i| f64::from(i) * 10.0).collect();
        times.push(440.0);
        let long = frames(10.0, &times);
        assert_eq!(long.late, 1);
        assert!(long.stuttered());
    }
}
