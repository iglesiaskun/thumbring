//! Focused window: which app it belongs to and whether it is fullscreen.

use std::collections::HashMap;
use std::sync::OnceLock;

use thumbring_core::apps::{AppName, hosted_app};
use thumbring_core::screen::{self, Rect, ShellState, WindowFacts};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, EnumChildWindows, GetClassNameW, GetForegroundWindow, GetShellWindow,
    GetWindowRect, GetWindowThreadProcessId, IsZoomed, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};
use windows::core::{BOOL, PWSTR};

type OnChange = Box<dyn Fn(usize) + Send + Sync>;
static ON_CHANGE: OnceLock<OnChange> = OnceLock::new();

/// Removes the focus change notification when dropped.
pub struct Watch(HWINEVENTHOOK);

impl Drop for Watch {
    fn drop(&mut self) {
        // SAFETY: the hook was registered on this thread and not removed yet.
        let _ = unsafe { UnhookWinEvent(self.0) };
    }
}

/// Asks Windows for a notification on every change of the focused window.
/// Must run on a thread with a message loop (the tray's): the notification
/// arrives through its queue, outside the focused app's process. The engine's
/// own windows (the tray menu) do not count.
pub fn watch(on_change: impl Fn(usize) + Send + Sync + 'static) -> Result<Watch, String> {
    ON_CHANGE
        .set(Box::new(on_change))
        .map_err(|_| "the focus change notification is already registered")?;
    // SAFETY: `on_event` lives for the whole program.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(on_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.is_invalid() {
        return Err("Windows refused the focus change notification".into());
    }
    Ok(Watch(hook))
}

unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if let Some(on_change) = ON_CHANGE.get() {
        on_change(hwnd.0 as usize);
    }
}

/// Focused window right now, as a number (0 if none).
pub fn current() -> usize {
    // SAFETY: query only.
    unsafe { GetForegroundWindow() }.0 as usize
}

fn hwnd(value: usize) -> HWND {
    HWND(value as *mut _)
}

/// Finds the app of each window, with a cache.
///
/// The key is the window together with its process: a window never changes
/// process, while a process number alone can be reused by Windows after the
/// app exits.
pub struct Resolver {
    cache: HashMap<(usize, u32), AppName>,
    own_pid: u32,
}

/// Cache limit. Past it, start over (closed windows pile up).
const CACHE_LIMIT: usize = 512;

impl Resolver {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            // SAFETY: query only.
            own_pid: unsafe { GetCurrentProcessId() },
        }
    }

    /// A window of the engine itself (the tray menu, for example).
    pub fn is_own(&self, window: usize) -> bool {
        window_pid(window) == Some(self.own_pid)
    }

    pub fn app_of(&mut self, window: usize) -> Option<AppName> {
        let pid = window_pid(window)?;
        if let Some(app) = self.cache.get(&(window, pid)) {
            return Some(app.clone());
        }
        let mut app = process_app(pid)?;
        if app.is_frame_host() {
            // UWP app: the frame belongs to the host; the real app owns a
            // child window.
            let children = child_pids(window, pid).into_iter().filter_map(process_app);
            app = hosted_app(app, children);
        }
        // Host without a child: the app is still starting. Not cached, so the
        // next lookup tries again.
        if !app.is_frame_host() {
            if self.cache.len() >= CACHE_LIMIT {
                self.cache.clear();
            }
            self.cache.insert((window, pid), app.clone());
        }
        Some(app)
    }
}

fn window_pid(window: usize) -> Option<u32> {
    if window == 0 {
        return None;
    }
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid local variable.
    unsafe { GetWindowThreadProcessId(hwnd(window), Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

/// Executable name of a process. Fails for protected system processes.
fn process_app(pid: u32) -> Option<AppName> {
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: the handle is closed right after; buffer and size are locals.
    let path = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        String::from_utf16_lossy(&buffer[..length as usize])
    };
    AppName::new(&path)
}

/// Processes that own the child windows, except `host` itself.
fn child_pids(window: usize, host: u32) -> Vec<u32> {
    struct Search {
        host: u32,
        pids: Vec<u32>,
    }
    unsafe extern "system" fn visit(child: HWND, data: LPARAM) -> BOOL {
        // SAFETY: `data` points to the `Search` below during the whole enumeration.
        let search = unsafe { &mut *(data.0 as *mut Search) };
        if let Some(pid) = window_pid(child.0 as usize)
            && pid != search.host
            && !search.pids.contains(&pid)
        {
            search.pids.push(pid);
        }
        true.into()
    }
    let mut search = Search {
        host,
        pids: Vec::new(),
    };
    // SAFETY: `search` lives until the enumeration ends, and it is synchronous.
    unsafe {
        let _ = EnumChildWindows(
            Some(hwnd(window)),
            Some(visit),
            LPARAM(&mut search as *mut Search as isize),
        );
    }
    search.pids
}

/// Is the window fullscreen? Combines what the shell says with the window and
/// monitor rectangles; the rule is in [`thumbring_core::screen`].
pub fn is_fullscreen(window: usize) -> bool {
    // SAFETY: query only.
    let shell = match unsafe { SHQueryUserNotificationState() } {
        Ok(QUNS_RUNNING_D3D_FULL_SCREEN) => ShellState::ExclusiveFullscreen,
        Ok(QUNS_BUSY | QUNS_PRESENTATION_MODE) => ShellState::Busy,
        _ => ShellState::Normal,
    };
    screen::is_fullscreen(shell, window_facts(window).as_ref())
}

fn window_facts(window: usize) -> Option<WindowFacts> {
    if window == 0 {
        return None;
    }
    let w = hwnd(window);
    let mut rect = RECT::default();
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: pointers to valid local variables.
    unsafe {
        GetWindowRect(w, &mut rect).ok()?;
        let monitor = MonitorFromWindow(w, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        Some(WindowFacts {
            rect: to_rect(rect),
            monitor: to_rect(info.rcMonitor),
            maximized: IsZoomed(w).as_bool(),
            is_shell: is_shell(w),
        })
    }
}

fn to_rect(r: RECT) -> Rect {
    Rect::new(r.left, r.top, r.right, r.bottom)
}

/// A window of the Windows shell: one of the shell process (the desktop,
/// the taskbar, Task View, Alt+Tab, the virtual desktop switch), or one of the
/// desktop and taskbar classes if the shell is restarting.
fn is_shell(w: HWND) -> bool {
    // SAFETY: queries only, with a local buffer.
    unsafe {
        let shell = GetShellWindow();
        if w == shell {
            return true;
        }
        if !shell.is_invalid() && window_pid(w.0 as usize) == window_pid(shell.0 as usize) {
            return true;
        }
        let mut buffer = [0u16; 64];
        let length = GetClassNameW(w, &mut buffer).max(0) as usize;
        let class = String::from_utf16_lossy(&buffer[..length]);
        matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        )
    }
}
