//! Apps for the exclusion list and the profiles: the name the settings file
//! uses for an app, and the apps that are open now, to pick from.

use serde::Serialize;
use thumbring_core::apps::AppName;

/// An app with a window on the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunningApp {
    /// Its executable, as the settings file writes it: `browser.exe`.
    pub exe: String,
    /// The title of one of its windows, to recognize it.
    pub title: String,
}

/// The name of an app as the settings file writes it, from a name or a path:
/// `Browser`, `browser.exe` and `C:\...\browser.exe` all give `browser.exe`.
pub fn app_name(text: &str) -> Option<String> {
    AppName::new(text).map(|app| app.to_string())
}

/// The apps with a window on the screen now, one entry per app, by name.
#[cfg(windows)]
pub fn running_apps() -> Vec<RunningApp> {
    windows_apps::running()
}

/// Only the Windows engine exists so far.
#[cfg(not(windows))]
pub fn running_apps() -> Vec<RunningApp> {
    Vec::new()
}

#[cfg(windows)]
mod windows_apps {
    use std::collections::BTreeMap;

    use thumbring_core::apps::{AppName, hosted_app};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetWindow, GetWindowLongPtrW,
        GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, WS_EX_TOOLWINDOW,
    };
    use windows::core::{BOOL, PWSTR};

    use super::RunningApp;

    pub fn running() -> Vec<RunningApp> {
        let mut windows: Vec<(HWND, String)> = Vec::new();
        unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> BOOL {
            // SAFETY: `data` points to `windows` during the whole enumeration.
            let windows = unsafe { &mut *(data.0 as *mut Vec<(HWND, String)>) };
            if let Some(title) = app_window_title(window) {
                windows.push((window, title));
            }
            true.into()
        }
        // SAFETY: the callback only uses `windows`, which outlives the call.
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut windows as *mut _ as isize)) };

        // SAFETY: no arguments.
        let own = unsafe { GetCurrentProcessId() };
        let mut apps = BTreeMap::new();
        for (window, title) in windows {
            let Some(pid) = window_pid(window).filter(|pid| *pid != own) else {
                continue;
            };
            let Some(app) = process_app(pid) else {
                continue;
            };
            // Store apps: the window belongs to their host; the app is a child.
            let app = hosted_app(
                app,
                child_pids(window, pid).into_iter().filter_map(process_app),
            );
            apps.entry(app.to_string()).or_insert(title);
        }
        apps.into_iter()
            .map(|(exe, title)| RunningApp { exe, title })
            .collect()
    }

    /// The title of a window that Alt+Tab would show: visible and not
    /// cloaked (hidden by the system), with no owner, not a tool window, and
    /// with a title.
    fn app_window_title(window: HWND) -> Option<String> {
        // SAFETY: plain queries on a window handle from EnumWindows.
        unsafe {
            if !IsWindowVisible(window).as_bool()
                || GetWindow(window, GW_OWNER).is_ok_and(|owner| !owner.is_invalid())
                || GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0
                || is_cloaked(window)
            {
                return None;
            }
            let mut buffer = [0u16; 256];
            let length = GetWindowTextW(window, &mut buffer);
            (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
        }
    }

    fn is_cloaked(window: HWND) -> bool {
        let mut cloaked = 0u32;
        // SAFETY: `cloaked` is a local of the size the attribute asks for.
        let result = unsafe {
            DwmGetWindowAttribute(
                window,
                DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut _,
                size_of::<u32>() as u32,
            )
        };
        result.is_ok() && cloaked != 0
    }

    fn window_pid(window: HWND) -> Option<u32> {
        let mut pid = 0u32;
        // SAFETY: `pid` is a valid local variable.
        unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
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
    fn child_pids(window: HWND, host: u32) -> Vec<u32> {
        struct Search {
            host: u32,
            pids: Vec<u32>,
        }
        unsafe extern "system" fn visit(child: HWND, data: LPARAM) -> BOOL {
            // SAFETY: `data` points to the `Search` below during the whole enumeration.
            let search = unsafe { &mut *(data.0 as *mut Search) };
            if let Some(pid) = window_pid(child)
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
        // SAFETY: the callback only uses `search`, which outlives the call.
        let _ = unsafe {
            EnumChildWindows(
                Some(window),
                Some(visit),
                LPARAM(&mut search as *mut _ as isize),
            )
        };
        search.pids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_names_follow_the_engine() {
        assert_eq!(app_name("Browser").as_deref(), Some("browser.exe"));
        assert_eq!(
            app_name(r"C:\Apps\Editor.EXE").as_deref(),
            Some("editor.exe")
        );
        assert_eq!(app_name("  "), None);
    }
}
