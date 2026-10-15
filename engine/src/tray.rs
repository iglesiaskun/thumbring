//! Tray icon: shows whether gestures are on, pauses and resumes them, opens
//! the settings file and the log, chooses how Thumbring starts with Windows
//! (normally or as administrator), and quits.
//! Its hidden window also hears about sleep and the lock screen (see
//! [`session`](crate::session)).
//!
//! Everything here runs on the main thread, which owns the hidden window that
//! receives the icon clicks. Other threads only call [`show`], [`notify`] and
//! [`request_quit`], which store the request and post a message to the window
//! (like `Control.BeginInvoke` in WinForms).

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

use thumbring_core::apps::Activity;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_ERROR, NIIF_INFO, NIIF_NOSOUND,
    NIIF_RESPECT_QUIET_TIME, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NIN_SELECT, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetMessageW, GetSystemMetrics, HICON, HMENU, MENU_ITEM_FLAGS,
    MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SM_CXSMICON, SetForegroundWindow, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx, TranslateMessage, WM_APP, WM_CLOSE,
    WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_NULL, WM_POWERBROADCAST, WM_SETTINGCHANGE,
    WM_WTSSESSION_CHANGE, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::art::Look;
use crate::autostart::{Failure, Way};
use crate::log::{error, info, warning};
use crate::worker::Msg;
use crate::{actions, autostart, elevation, hook, icon, session, text};

const CLASS: PCWSTR = w!("Thumbring.Tray");
/// Clicks on the icon.
const WM_TRAY: u32 = WM_APP + 1;
/// The worker thread changed the gesture state.
const WM_SHOW: u32 = WM_APP + 2;
/// Notifications are waiting to be shown.
const WM_NOTICE: u32 = WM_APP + 3;
/// With NOTIFYICON_VERSION_4, Enter or Space on the selected icon.
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const ICON_ID: u32 = 1;

// Menu items. They also work as WM_COMMAND, which allows testing without
// clicking.
pub const ID_PAUSE: u32 = 1;
pub const ID_OPEN_CONFIG: u32 = 2;
pub const ID_OPEN_LOG: u32 = 3;
pub const ID_AUTOSTART: u32 = 4;
pub const ID_QUIT: u32 = 5;
pub const ID_ADMIN_START: u32 = 6;
pub const ID_OPEN_SETTINGS: u32 = 7;

static WINDOW: AtomicUsize = AtomicUsize::new(0);
/// Message Explorer sends when the taskbar restarts.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static PENDING_VIEW: Mutex<Option<Activity>> = Mutex::new(None);
static PENDING_NOTICES: Mutex<Vec<(Notice, String, String)>> = Mutex::new(Vec::new());
/// How Thumbring starts with Windows is changing on another thread, which may
/// be waiting for the UAC prompt.
static CHANGING_STARTUP: AtomicBool = AtomicBool::new(false);

/// Kind of notification.
#[derive(Debug, Clone, Copy)]
pub enum Notice {
    Info,
    Warning,
    Error,
}

/// What the menu needs.
pub struct Context {
    pub events: Sender<Msg>,
    pub config_file: PathBuf,
    pub log_file: PathBuf,
    /// The folder given with `--dir`, for the settings window.
    pub dir: Option<PathBuf>,
}

struct State {
    context: Context,
    icons: Icons,
    activity: Activity,
    /// Sleep, lock screen and display notifications, while the window exists.
    _session: session::Watch,
}

thread_local! {
    // Only the main thread touches this, always through `with_state`.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Shows the gesture state. Can be called from any thread.
pub fn show(activity: Activity) {
    *PENDING_VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(activity);
    post(WM_SHOW);
}

/// Shows a balloon (a notification on Windows 11). Can be called from any
/// thread.
pub fn notify(kind: Notice, title: &str, text: &str) {
    PENDING_NOTICES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((kind, title.into(), text.into()));
    post(WM_NOTICE);
}

/// Closes the window, which ends the message loop. Can be called from any
/// thread.
pub fn request_quit() {
    post(WM_CLOSE);
}

/// Takes the icon out of the notification area at once, from any thread.
/// For when the engine stops because of a bug.
pub fn remove_now() {
    let window = WINDOW.swap(0, Ordering::SeqCst);
    if window != 0 {
        remove_icon(HWND(window as *mut _));
    }
}

fn post(message: u32) {
    let window = WINDOW.load(Ordering::SeqCst);
    if window != 0 {
        // SAFETY: only posts a message; a destroyed window just returns an error.
        let _ =
            unsafe { PostMessageW(Some(HWND(window as *mut _)), message, WPARAM(0), LPARAM(0)) };
    }
}

/// Creates the hidden window and the icon. Must run on the thread that will
/// call [`run_message_loop`].
pub fn create(context: Context) -> windows::core::Result<()> {
    // SAFETY: class registration and window creation with data that lives for
    // the whole program; `wndproc` is a static `extern "system"` function.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(w!("TaskbarCreated")),
            Ordering::SeqCst,
        );
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS,
            &HSTRING::from(text::APP_NAME),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        if elevation::is_elevated() {
            // Explorer (clicks on the icon, the taskbar coming back), the
            // Settings app (light or dark mode) and the installer (quit before
            // an update) run without administrator rights, and Windows would
            // keep their messages from reaching this window.
            elevation::accept_from_programs_without_rights(
                window,
                &[
                    WM_TRAY,
                    TASKBAR_CREATED.load(Ordering::SeqCst),
                    WM_SETTINGCHANGE,
                    WM_CLOSE,
                ],
            );
        }
        let icons = Icons::load()?;
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                context,
                icons,
                activity: Activity::Active,
                _session: session::watch(window),
            })
        });
        WINDOW.store(window.0 as usize, Ordering::SeqCst);
        add_icon(window);
        // The worker thread may have decided the state already.
        apply_pending_view(window);
        apply_pending_notices(window);
    }
    Ok(())
}

pub fn run_message_loop() {
    let mut msg = MSG::default();
    // SAFETY: standard message loop with a local variable.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY => {
            match lparam.0 as u32 & 0xFFFF {
                // With NOTIFYICON_VERSION_4, wparam carries the click position.
                WM_CONTEXTMENU => {
                    let x = (wparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    show_menu(window, x, y);
                }
                NIN_SELECT | NIN_KEYSELECT => command(window, ID_PAUSE),
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            command(window, (wparam.0 & 0xFFFF) as u32);
            LRESULT(0)
        }
        WM_SHOW => {
            apply_pending_view(window);
            LRESULT(0)
        }
        WM_NOTICE => {
            apply_pending_notices(window);
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            // The taskbar switched between light and dark: redraw.
            // SAFETY: with WM_SETTINGCHANGE, lparam is null or points to a string.
            let area = (lparam.0 != 0)
                .then(|| unsafe { PCWSTR(lparam.0 as *const u16).to_string() }.ok())
                .flatten();
            if area.as_deref() == Some("ImmersiveColorSet") {
                reload_icons(window);
            }
            // SAFETY: passes the same arguments along.
            unsafe { DefWindowProcW(window, msg, wparam, lparam) }
        }
        WM_POWERBROADCAST | WM_WTSSESSION_CHANGE => {
            if let Some(reason) = session::woke_up(msg, wparam, lparam) {
                info!("{reason}");
                hook::reinstall();
            }
            // SAFETY: passes the same arguments along.
            unsafe { DefWindowProcW(window, msg, wparam, lparam) }
        }
        WM_DESTROY => {
            remove_icon(window);
            WINDOW.store(0, Ordering::SeqCst);
            STATE.with(|s| s.borrow_mut().take());
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::SeqCst) => {
            // Explorer restarted and the notification area came back empty.
            add_icon(window);
            LRESULT(0)
        }
        // SAFETY: passes the same arguments along.
        _ => unsafe { DefWindowProcW(window, msg, wparam, lparam) },
    }
}

/// State access that never panics: while a menu is open, Windows may call
/// `wndproc` again before the previous call returns, and then the state is
/// already borrowed.
fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok()?.as_mut().map(f))
}

fn show_menu(window: HWND, x: i32, y: i32) {
    let paused = with_state(|s| s.activity == Activity::Paused).unwrap_or(false);
    let pause = if paused {
        text::menu::RESUME
    } else {
        text::menu::PAUSE
    };
    let startup = autostart::current();
    let changing = CHANGING_STARTUP.load(Ordering::SeqCst);
    let item = |way: Way, available: bool| {
        let mut flags = MF_STRING;
        if startup.has(way) {
            flags |= MF_CHECKED;
        }
        if changing || !available {
            flags |= MF_GRAYED;
        }
        flags
    };
    let normal = item(Way::Normal, true);
    // Without administrator rights on this account, the task would start
    // Thumbring with the same rights as always. A task that exists can
    // still be turned off.
    let administrator = item(
        Way::Administrator,
        elevation::can_elevate() || startup.administrator,
    );
    // SAFETY: the menu is created, shown and destroyed here; the item texts
    // live until the menu is gone.
    let chosen = unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        append(menu, MF_STRING, ID_PAUSE, pause);
        append(menu, MF_SEPARATOR, 0, "");
        append(menu, MF_STRING, ID_OPEN_SETTINGS, text::menu::OPEN_SETTINGS);
        append(menu, MF_STRING, ID_OPEN_CONFIG, text::menu::OPEN_CONFIG);
        append(menu, MF_STRING, ID_OPEN_LOG, text::menu::OPEN_LOG);
        append(menu, normal, ID_AUTOSTART, text::menu::START_WITH_WINDOWS);
        append(
            menu,
            administrator,
            ID_ADMIN_START,
            text::menu::START_AS_ADMINISTRATOR,
        );
        append(menu, MF_SEPARATOR, 0, "");
        append(menu, MF_STRING, ID_QUIT, text::menu::QUIT);
        // Without this, the menu does not close when clicking outside it.
        let _ = SetForegroundWindow(window);
        let flags = TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY;
        let chosen = TrackPopupMenuEx(menu, flags.0, x, y, window, None).0 as u32;
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        chosen
    };
    if chosen != 0 {
        command(window, chosen);
    }
}

/// Adds a menu item. The menu copies the text, so a temporary string is fine.
unsafe fn append(menu: HMENU, flags: MENU_ITEM_FLAGS, id: u32, label: &str) {
    let label = HSTRING::from(label);
    let label = if flags == MF_SEPARATOR {
        PCWSTR::null()
    } else {
        PCWSTR(label.as_ptr())
    };
    // SAFETY: `menu` is a valid menu and `label` lives during the call.
    let _ = unsafe { AppendMenuW(menu, flags, id as usize, label) };
}

fn command(window: HWND, id: u32) {
    match id {
        ID_PAUSE => {
            with_state(|s| s.context.events.send(Msg::TogglePause));
        }
        ID_OPEN_SETTINGS => {
            with_state(|s| {
                actions::open_settings_window(&s.context.config_file, s.context.dir.as_deref());
            });
        }
        ID_OPEN_CONFIG => {
            if let Some(path) = with_state(|s| s.context.config_file.clone()) {
                actions::open_text_file(&path);
            }
        }
        ID_OPEN_LOG => {
            if let Some(path) = with_state(|s| s.context.log_file.clone()) {
                actions::open_text_file(&path);
            }
        }
        ID_AUTOSTART => change_startup(window, Way::Normal),
        ID_ADMIN_START => change_startup(window, Way::Administrator),
        ID_QUIT => {
            info!("quitting from the tray menu");
            // SAFETY: the window belongs to this thread.
            let _ = unsafe { DestroyWindow(window) };
        }
        _ => {}
    }
}

/// Turns a way of starting with Windows on or off. It runs on its own thread,
/// because the UAC prompt may wait for an answer; meanwhile both menu items
/// are grayed.
fn change_startup(window: HWND, way: Way) {
    if CHANGING_STARTUP.swap(true, Ordering::SeqCst) {
        return;
    }
    // The window handle is a number that any thread may use.
    let owner = window.0 as usize;
    let spawned = thread::Builder::new()
        .name("startup".into())
        .spawn(move || {
            let result = autostart::toggle(way, HWND(owner as *mut _));
            CHANGING_STARTUP.store(false, Ordering::SeqCst);
            report_startup(way, result);
        });
    if let Err(e) = spawned {
        CHANGING_STARTUP.store(false, Ordering::SeqCst);
        report_startup(way, Err(Failure::Error(e.to_string())));
    }
}

fn report_startup(way: Way, result: Result<bool, Failure>) {
    match (way, result) {
        (Way::Normal, Ok(true)) => info!("will start with Windows"),
        (Way::Normal, Ok(false)) => info!("will no longer start with Windows"),
        (Way::Administrator, Ok(true)) => {
            info!("will start with Windows as administrator");
            notify(
                Notice::Info,
                text::notice::ADMIN_START_TITLE,
                text::notice::ADMIN_START_TEXT,
            );
        }
        (Way::Administrator, Ok(false)) => {
            info!("will no longer start with Windows as administrator")
        }
        (_, Err(Failure::Cancelled)) => {
            info!("the administrator prompt was cancelled; nothing changed")
        }
        (_, Err(Failure::Error(e))) => {
            error!("could not change how Thumbring starts with Windows: {e}");
            notify(
                Notice::Error,
                text::notice::AUTOSTART_ERROR_TITLE,
                text::notice::AUTOSTART_ERROR_TEXT,
            );
        }
    }
}

fn apply_pending_view(window: HWND) {
    let pending = PENDING_VIEW
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(activity) = pending {
        with_state(|s| s.activity = activity);
        update_icon(window);
    }
}

fn apply_pending_notices(window: HWND) {
    let mut pending =
        std::mem::take(&mut *PENDING_NOTICES.lock().unwrap_or_else(|e| e.into_inner()));
    // Back-to-back balloons run over each other: the latest one wins.
    if let Some((kind, title, text)) = pending.pop() {
        let mut data = base(window);
        data.uFlags = NIF_INFO;
        copy_wide(&mut data.szInfoTitle, &title);
        copy_wide(&mut data.szInfo, &text);
        let kind = match kind {
            Notice::Info => NIIF_INFO,
            Notice::Warning => NIIF_WARNING,
            Notice::Error => NIIF_ERROR,
        };
        data.dwInfoFlags = kind | NIIF_NOSOUND | NIIF_RESPECT_QUIET_TIME;
        // SAFETY: `data` is a complete local structure.
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }
}

fn base(window: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: ICON_ID,
        ..Default::default()
    }
}

/// Current icon and tooltip, ready for NIM_ADD or NIM_MODIFY.
fn current(window: HWND) -> Option<NOTIFYICONDATAW> {
    with_state(|s| {
        let mut data = base(window);
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = match s.activity {
            Activity::Active => s.icons.active,
            Activity::Paused => s.icons.paused,
            Activity::Excluded(_) | Activity::Fullscreen => s.icons.inactive,
        };
        copy_wide(&mut data.szTip, &text::tooltip(&s.activity));
        data
    })
}

fn add_icon(window: HWND) {
    let Some(mut data) = current(window) else {
        return;
    };
    // SAFETY: `data` is a complete local structure.
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
            // Right after logon the notification area may not be ready; the
            // TaskbarCreated message arrives later and tries again.
            warning!("the notification area did not accept the icon yet");
            return;
        }
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
    }
}

fn update_icon(window: HWND) {
    if let Some(data) = current(window) {
        // SAFETY: `data` is a complete local structure.
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }
}

fn remove_icon(window: HWND) {
    // SAFETY: `data` is a complete local structure.
    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &base(window)) };
}

fn reload_icons(window: HWND) {
    match Icons::load() {
        Ok(icons) => {
            // The old icons are freed only after the tray switches over.
            let old = with_state(|s| std::mem::replace(&mut s.icons, icons));
            update_icon(window);
            drop(old);
        }
        Err(e) => warning!("could not redraw the tray icon: {e}"),
    }
}

fn copy_wide(target: &mut [u16], text: &str) {
    let max = target.len() - 1;
    let mut n = 0;
    for unit in text.encode_utf16().take(max) {
        target[n] = unit;
        n += 1;
    }
    target[n] = 0;
}

struct Icons {
    active: HICON,
    inactive: HICON,
    paused: HICON,
}

impl Icons {
    fn load() -> windows::core::Result<Self> {
        // SAFETY: query only.
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        let light = icon::light_taskbar();
        Ok(Self {
            active: icon::create(size, Look::Active, light)?,
            inactive: icon::create(size, Look::Inactive, light)?,
            paused: icon::create(size, Look::Paused, light)?,
        })
    }
}

impl Drop for Icons {
    fn drop(&mut self) {
        for icon in [self.active, self.inactive, self.paused] {
            // SAFETY: each icon was created by `icon::create` and is freed once.
            let _ = unsafe { DestroyIcon(icon) };
        }
    }
}
