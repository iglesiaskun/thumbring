//! Low-level mouse hook (`WH_MOUSE_LL`) on a thread of its own, plus a
//! keyboard hook (`WH_KEYBOARD_LL`) that only exists while the ring is open.
//!
//! Windows calls the callbacks on the thread that installed the hooks, through
//! its message queue, so that thread needs a message loop. A callback has
//! little time: if it is slow, Windows skips it and, if that keeps happening,
//! removes the hook without notice. So the callbacks only decide whether to
//! block an event and send a note over a channel; they never run an action or
//! do I/O.
//!
//! What the mouse hook takes:
//! - the gesture button (Back unless the settings say otherwise);
//! - the ring button (Forward unless the settings say otherwise), only when
//!   the ring has items;
//! - while the ring is open, every other click and the wheel, so the app below
//!   never sees them, and Esc on the keyboard.
//!
//! Blocking is always symmetric: a release is blocked only if its press was.
//! The hook remembers what each blocked press was for, and its release follows
//! that, whatever the settings say by then.
//!
//! The hooks come and go while the engine runs (pause, excluded app,
//! fullscreen): without a hook, Windows does not even call the engine on every
//! mouse event. They are also put back after sleep and the lock screen, when
//! Windows may have removed them (see [`reinstall`]).

use std::error::Error;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use thumbring_core::config::MouseButton;
use thumbring_core::gesture::Point;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLMHF_INJECTED,
    MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_APP, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_USER, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
};

use crate::elevation;
use crate::input::SIGNATURE;

/// A mouse button, as the hook sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl From<MouseButton> for Button {
    fn from(button: MouseButton) -> Self {
        match button {
            MouseButton::Back => Button::Back,
            MouseButton::Forward => Button::Forward,
            MouseButton::Middle => Button::Middle,
        }
    }
}

/// What the hooks send to the worker thread.
#[derive(Debug, Clone, Copy)]
pub enum HookEvent {
    /// The gesture button was pressed. The event was blocked.
    GestureDown {
        button: MouseButton,
        pos: Point,
        time: u32,
    },
    /// The cursor moved while the gesture or the ring button was held, or
    /// while the ring is open. The movement is not blocked.
    Move { pos: Point },
    /// The gesture button was released. The event was blocked.
    GestureUp { pos: Point, time: u32 },
    /// The ring button was pressed. The event was blocked.
    TriggerDown { button: MouseButton },
    /// The ring button was released. The event was blocked.
    TriggerUp,
    /// Another mouse button was pressed while the ring is open. The click was
    /// blocked, and so will its release be.
    Click { button: Button, pos: Point },
    /// Esc was pressed while the ring is open. The key was blocked.
    Esc,
    /// The hook is back after having been removed.
    Installed,
    /// The hook was removed: the buttons work as usual.
    Removed,
    /// Windows refused to install the hook again (HRESULT code).
    InstallFailed(i32),
}

type Sink = Box<dyn Fn(HookEvent) + Send + Sync>;

// The callbacks are `extern "system"` functions with no context parameter, so
// everything they use must be global.
static SINK: OnceLock<Sink> = OnceLock::new();
/// Hook thread, so other threads can post messages to it.
static THREAD_ID: AtomicU32 = AtomicU32::new(0);
/// What each button's press was blocked for ([`Role`]), one per [`Button`],
/// until the matching release.
static HELD: [AtomicU8; 5] = [const { AtomicU8::new(Role::Free as u8) }; 5];
/// The buttons the settings ask for ([`Buttons::pack`]).
static WANTED_BUTTONS: AtomicU8 = AtomicU8::new(Buttons::DEFAULT.pack());
/// The buttons in use. They follow [`WANTED_BUTTONS`] only at a press while no
/// blocked press waits for its release and the ring is closed, so a button
/// never changes its job in the middle of a gesture or with the ring open.
static BUTTONS: AtomicU8 = AtomicU8::new(Buttons::DEFAULT.pack());
/// Between an Esc press we blocked and its release.
static ESC_HELD: AtomicBool = AtomicBool::new(false);
/// Whether the engine wants the hooks right now. The worker thread decides.
static WANTED: AtomicBool = AtomicBool::new(true);
/// The ring button opens the ring (the ring has at least one item).
static RING_ENABLED: AtomicBool = AtomicBool::new(false);
/// The ring is open and takes every click, the wheel and Esc.
static RING_OPEN: AtomicBool = AtomicBool::new(false);
/// The engine hit a bug and is stopping: every event passes through.
static GAVE_UP: AtomicBool = AtomicBool::new(false);

/// Thread message: "check whether the hooks should be installed".
const RECONCILE: u32 = WM_APP;
/// Thread message: "install the hooks that are in place again".
const REINSTALL: u32 = WM_APP + 1;

/// What a blocked press was for.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Role {
    /// No blocked press waits for its release.
    Free,
    Gesture,
    Trigger,
    /// A click while the ring was open.
    Click,
}

impl Role {
    fn from_byte(byte: u8) -> Role {
        match byte {
            1 => Role::Gesture,
            2 => Role::Trigger,
            3 => Role::Click,
            _ => Role::Free,
        }
    }
}

/// Which button runs the gestures and which opens the ring.
#[derive(Clone, Copy)]
struct Buttons {
    gestures: MouseButton,
    ring: MouseButton,
}

impl Buttons {
    const DEFAULT: Buttons = Buttons {
        gestures: MouseButton::Back,
        ring: MouseButton::Forward,
    };

    /// In one byte, so both buttons always change together.
    const fn pack(self) -> u8 {
        code(self.gestures) | code(self.ring) << 4
    }

    fn unpack(byte: u8) -> Buttons {
        Buttons {
            gestures: from_code(byte & 0xF),
            ring: from_code(byte >> 4),
        }
    }
}

const fn code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Back => 0,
        MouseButton::Forward => 1,
        MouseButton::Middle => 2,
    }
}

fn from_code(code: u8) -> MouseButton {
    match code {
        0 => MouseButton::Back,
        1 => MouseButton::Forward,
        _ => MouseButton::Middle,
    }
}

/// Installs the mouse hook on a new thread and waits for it to be active.
///
/// `sink` receives the events inside the callbacks, so it must be quick (send
/// over a channel).
pub fn install(
    sink: impl Fn(HookEvent) + Send + Sync + 'static,
) -> Result<JoinHandle<()>, Box<dyn Error>> {
    SINK.set(Box::new(sink))
        .map_err(|_| "the mouse hook is already installed")?;
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread = thread::Builder::new()
        .name("hook-mouse".into())
        .spawn(move || run(ready_tx))?;
    // The first `?` covers the thread having died; the second, the Windows error.
    ready_rx.recv()??;
    Ok(thread)
}

/// Turns the hooks on or off. Can be called from any thread. With a blocked
/// button held down, the removal waits for its release, so no app sees only
/// half of a click.
pub fn set_enabled(enabled: bool) {
    WANTED.store(enabled, Ordering::SeqCst);
    post(RECONCILE);
}

/// Which button runs the gestures and which opens the ring. Can be called from
/// any thread. The change waits until no blocked press is held and the ring is
/// closed: a press already blocked is released the way it started.
pub fn set_buttons(gestures: MouseButton, ring: MouseButton) {
    WANTED_BUTTONS.store(Buttons { gestures, ring }.pack(), Ordering::SeqCst);
}

/// Whether the ring button opens the ring. Without items, it should keep
/// doing what it always did.
pub fn set_ring_enabled(enabled: bool) {
    RING_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Whether the ring is open and takes clicks, the wheel and Esc.
pub fn set_ring_open(open: bool) {
    RING_OPEN.store(open, Ordering::SeqCst);
    post(RECONCILE);
}

/// Asks the hook thread to leave its loop and remove the hooks. Can be called
/// from any thread.
pub fn request_stop() {
    post(WM_QUIT);
}

/// Replaces the hooks in place with new ones. Windows removes a hook without
/// notice when its thread is too slow to answer, which can happen while the
/// computer wakes up from sleep or the session is locked; there is no way to
/// ask whether the hook is still there, and a new one costs nothing. Can be
/// called from any thread.
pub fn reinstall() {
    post(REINSTALL);
}

/// After a bug, while the engine stops: from now on every event passes
/// through, and the hook thread removes the hooks as soon as it gets the
/// chance. Can be called from any thread.
pub fn give_up() {
    GAVE_UP.store(true, Ordering::SeqCst);
    post(WM_QUIT);
}

fn post(message: u32) {
    let id = THREAD_ID.load(Ordering::SeqCst);
    if id != 0 {
        // SAFETY: only posts a message; an invalid id just returns an error.
        let _ = unsafe { PostThreadMessageW(id, message, WPARAM(0), LPARAM(0)) };
    }
}

/// The hooks currently installed.
#[derive(Default)]
struct Hooks {
    mouse: Option<HHOOK>,
    keyboard: Option<HHOOK>,
}

fn run(ready: Sender<windows::core::Result<()>>) {
    let mut msg = MSG::default();
    // SAFETY: Win32 calls with pointers to valid local variables. The hooks are
    // removed on this same thread before it ends.
    unsafe {
        // Makes sure the message queue exists before publishing the thread
        // id; without a queue, a message posted early would be lost.
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
        THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);

        let mut hooks = Hooks::default();
        match set_hook(WH_MOUSE_LL) {
            Ok(hook) => hooks.mouse = Some(hook),
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        }
        let _ = ready.send(Ok(()));

        // The callbacks run in here, while the thread waits for messages.
        // GetMessageW returns 0 on WM_QUIT and -1 on error.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            match msg.message {
                RECONCILE => reconcile(&mut hooks),
                REINSTALL => reinstall_now(&mut hooks),
                _ => {}
            }
        }

        for hook in [hooks.mouse, hooks.keyboard].into_iter().flatten() {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
}

type Callback = unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT;

/// Installs the mouse hook (`WH_MOUSE_LL`) or the keyboard hook.
fn set_hook(kind: WINDOWS_HOOK_ID) -> windows::core::Result<HHOOK> {
    let callback: Callback = if kind == WH_MOUSE_LL {
        mouse_proc
    } else {
        keyboard_proc
    };
    // SAFETY: the callbacks live for the whole program; the module is the
    // executable itself.
    unsafe {
        let module = GetModuleHandleW(None)?;
        SetWindowsHookExW(kind, Some(callback), Some(module.into()), 0)
    }
}

/// The hooks must stay: a blocked button is still down (its release must be
/// blocked too), or the ring is open (it takes every click until it closes).
fn must_stay() -> bool {
    held(|role| role != Role::Free) || RING_OPEN.load(Ordering::SeqCst)
}

/// Some button's press was blocked for a role that `matches`, and the release
/// has not come yet.
fn held(matches: impl Fn(Role) -> bool) -> bool {
    HELD.iter()
        .any(|role| matches(Role::from_byte(role.load(Ordering::SeqCst))))
}

/// Leaves the hooks the way the worker and ring threads asked. Runs outside
/// the callbacks.
fn reconcile(hooks: &mut Hooks) {
    let wanted = WANTED.load(Ordering::SeqCst);
    match hooks.mouse {
        None if wanted => match set_hook(WH_MOUSE_LL) {
            Ok(h) => {
                hooks.mouse = Some(h);
                send(HookEvent::Installed);
            }
            Err(e) => send(HookEvent::InstallFailed(e.code().0)),
        },
        // With a blocked button held down, the press was already blocked: the
        // release must be blocked too. The callback (or the ring, when it
        // closes) asks for another check after it.
        Some(h) if !wanted && !must_stay() => {
            // SAFETY: `h` was installed on this thread and not removed yet.
            let _ = unsafe { UnhookWindowsHookEx(h) };
            hooks.mouse = None;
            send(HookEvent::Removed);
        }
        _ => {}
    }

    // The keyboard hook only exists while the ring is open, for Esc, and
    // until the release of an Esc it blocked.
    let keyboard_wanted = hooks.mouse.is_some()
        && (RING_OPEN.load(Ordering::SeqCst) || ESC_HELD.load(Ordering::SeqCst));
    match hooks.keyboard {
        None if keyboard_wanted => hooks.keyboard = set_hook(WH_KEYBOARD_LL).ok(),
        Some(h) if !keyboard_wanted => {
            // SAFETY: `h` was installed on this thread and not removed yet.
            let _ = unsafe { UnhookWindowsHookEx(h) };
            hooks.keyboard = None;
            ESC_HELD.store(false, Ordering::SeqCst);
        }
        _ => {}
    }
}

/// Installs each hook in place again. The new hook goes in before the old one
/// comes out, so no event slips through in between; the callbacks only run
/// when this thread waits for messages, so the two never both see an event.
fn reinstall_now(hooks: &mut Hooks) {
    if let Some(old) = hooks.mouse {
        match set_hook(WH_MOUSE_LL) {
            Ok(new) => {
                // SAFETY: `old` was installed on this thread. If Windows had
                // already removed it, this just returns an error.
                let _ = unsafe { UnhookWindowsHookEx(old) };
                hooks.mouse = Some(new);
                send(HookEvent::Installed);
            }
            Err(e) => send(HookEvent::InstallFailed(e.code().0)),
        }
    }
    if let Some(old) = hooks.keyboard {
        if let Ok(new) = set_hook(WH_KEYBOARD_LL) {
            // SAFETY: as above.
            let _ = unsafe { UnhookWindowsHookEx(old) };
            hooks.keyboard = Some(new);
        }
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && !GAVE_UP.load(Ordering::Relaxed) {
        // SAFETY: with WH_MOUSE_LL and HC_ACTION, lparam points to a valid
        // MSLLHOOKSTRUCT during this call.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if should_block(wparam.0 as u32, info) {
            return LRESULT(1);
        }
    }
    // SAFETY: passes on the same arguments we received.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Decides the fate of a mouse event. `true` blocks it: no app sees the event.
fn should_block(message: u32, info: &MSLLHOOKSTRUCT) -> bool {
    // The clicks and keys the engine itself sends pass straight through.
    if info.flags & LLMHF_INJECTED != 0 && info.dwExtraInfo == SIGNATURE {
        return false;
    }
    // While Thumbring runs as administrator, so do clicks that any other
    // program simulates: a program without those rights must not get
    // Thumbring to press keys in windows it cannot reach itself. Windows does
    // not say which program simulated a click, so only the real mouse counts.
    if info.flags & LLMHF_INJECTED != 0 && elevation::is_elevated() {
        return false;
    }
    let pos = Point::new(info.pt.x, info.pt.y);
    let ring_open = RING_OPEN.load(Ordering::Relaxed);
    match button_event(message, info) {
        Some((button, true)) => press(button, pos, info.time),
        Some((button, false)) => release(button, pos, info.time),
        None => match message {
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => ring_open,
            WM_MOUSEMOVE => {
                if ring_open || held(|role| matches!(role, Role::Gesture | Role::Trigger)) {
                    send(HookEvent::Move { pos });
                }
                false
            }
            _ => false,
        },
    }
}

/// A button went down: blocks it if it runs the gestures, opens the ring, or
/// is a click while the ring is open.
fn press(button: Button, pos: Point, time: u32) -> bool {
    // Nothing in progress: the buttons from the latest settings take over.
    if !must_stay() {
        BUTTONS.store(WANTED_BUTTONS.load(Ordering::Relaxed), Ordering::Relaxed);
    }
    let buttons = Buttons::unpack(BUTTONS.load(Ordering::Relaxed));
    // Turned off (waiting for the hook thread to remove the hook): new
    // presses pass through; releases of blocked presses are still blocked.
    let wanted = WANTED.load(Ordering::Relaxed);
    let (role, event) = if wanted && button == buttons.gestures.into() {
        let button = buttons.gestures;
        (Role::Gesture, HookEvent::GestureDown { button, pos, time })
    } else if wanted && button == buttons.ring.into() && RING_ENABLED.load(Ordering::Relaxed) {
        let button = buttons.ring;
        (Role::Trigger, HookEvent::TriggerDown { button })
    } else if RING_OPEN.load(Ordering::Relaxed) {
        // Even the ring button, when the ring has no items any more: the ring
        // takes every click until it closes.
        (Role::Click, HookEvent::Click { button, pos })
    } else {
        return false;
    };
    HELD[button as usize].store(role as u8, Ordering::Relaxed);
    send(event);
    true
}

/// A button went up: blocked only if its press was, and reported as the press
/// was.
fn release(button: Button, pos: Point, time: u32) -> bool {
    let role = HELD[button as usize].swap(Role::Free as u8, Ordering::Relaxed);
    match Role::from_byte(role) {
        Role::Free => return false,
        Role::Gesture => send(HookEvent::GestureUp { pos, time }),
        Role::Trigger => send(HookEvent::TriggerUp),
        Role::Click => {}
    }
    released();
    true
}

/// After the release of a blocked press: a removal may have been waiting.
fn released() {
    if !WANTED.load(Ordering::Relaxed) {
        post(RECONCILE);
    }
}

/// The button of a press (`true`) or release (`false`), if the message is one.
fn button_event(message: u32, info: &MSLLHOOKSTRUCT) -> Option<(Button, bool)> {
    let event = match message {
        WM_LBUTTONDOWN => (Button::Left, true),
        WM_LBUTTONUP => (Button::Left, false),
        WM_RBUTTONDOWN => (Button::Right, true),
        WM_RBUTTONUP => (Button::Right, false),
        WM_MBUTTONDOWN => (Button::Middle, true),
        WM_MBUTTONUP => (Button::Middle, false),
        WM_XBUTTONDOWN | WM_XBUTTONUP => {
            // The high word of `mouseData` says which extra button was used.
            let button = match (info.mouseData >> 16) as u16 {
                XBUTTON1 => Button::Back,
                XBUTTON2 => Button::Forward,
                _ => return None,
            };
            (button, message == WM_XBUTTONDOWN)
        }
        _ => return None,
    };
    Some(event)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && !GAVE_UP.load(Ordering::Relaxed) {
        // SAFETY: with WH_KEYBOARD_LL and HC_ACTION, lparam points to a valid
        // KBDLLHOOKSTRUCT during this call.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let ours = info.flags.0 & LLKHF_INJECTED.0 != 0 && info.dwExtraInfo == SIGNATURE;
        if info.vkCode == u32::from(VK_ESCAPE.0) && !ours {
            match wparam.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN if RING_OPEN.load(Ordering::Relaxed) => {
                    // Auto-repeat sends more presses: report only the first.
                    if !ESC_HELD.swap(true, Ordering::Relaxed) {
                        send(HookEvent::Esc);
                    }
                    return LRESULT(1);
                }
                WM_KEYUP | WM_SYSKEYUP if ESC_HELD.swap(false, Ordering::Relaxed) => {
                    // The ring is probably closed by now: the keyboard hook
                    // can go.
                    post(RECONCILE);
                    return LRESULT(1);
                }
                _ => {}
            }
        }
    }
    // SAFETY: passes on the same arguments we received.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn send(event: HookEvent) {
    if let Some(sink) = SINK.get() {
        sink(event);
    }
}
