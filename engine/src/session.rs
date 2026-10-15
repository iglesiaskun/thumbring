//! Sleep, the lock screen and the display turning on: after them, the hooks
//! are installed again.
//!
//! Windows removes a low-level hook without notice when its thread takes too
//! long to answer, and that is most likely right after the computer wakes up
//! or the session is unlocked. There is no way to ask whether a hook is still
//! there, so the engine simply installs it again at those moments (see
//! [`hook::reinstall`](crate::hook::reinstall)).
//!
//! The notifications arrive at the tray window, on the main thread.

use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::Power::{
    HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
    UnregisterPowerSettingNotification,
};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_WINDOW_HANDLE, PBT_APMRESUMEAUTOMATIC, PBT_POWERSETTINGCHANGE, WM_POWERBROADCAST,
    WM_WTSSESSION_CHANGE, WTS_CONSOLE_CONNECT, WTS_REMOTE_CONNECT, WTS_SESSION_UNLOCK,
};
use windows::core::GUID;

use crate::log::warning;

/// Whether the display is off (0), on (1) or dimmed (2). Defined here rather
/// than taken from the large `SystemServices` module of the `windows` crate.
const GUID_CONSOLE_DISPLAY_STATE: GUID = GUID::from_u128(0x6fe69556_704a_47a0_8f24_c28d936fda47);
const DISPLAY_OFF: u32 = 0;
const DISPLAY_ON: u32 = 1;
/// Before the first report.
const DISPLAY_UNKNOWN: u32 = u32::MAX;

/// Last display state reported.
static DISPLAY: AtomicU32 = AtomicU32::new(DISPLAY_UNKNOWN);

/// Stops the notifications when dropped.
pub struct Watch {
    window: HWND,
    session: bool,
    display: Option<HPOWERNOTIFY>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        // SAFETY: each registration was made for this window and is undone once.
        unsafe {
            if self.session {
                let _ = WTSUnRegisterSessionNotification(self.window);
            }
            if let Some(display) = self.display {
                let _ = UnregisterPowerSettingNotification(display);
            }
        }
    }
}

/// Asks for the session (lock and unlock) and display notifications. Waking
/// up from sleep is broadcast to every window without asking. A notification
/// Windows refuses only makes the engine a little less robust, so it is
/// logged and the engine goes on.
pub fn watch(window: HWND) -> Watch {
    // SAFETY: registrations for a window of this thread, undone in `drop`.
    let session = unsafe { WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) };
    if let Err(e) = &session {
        warning!("no lock screen notifications ({e}); the hooks may not come back after it");
    }
    // SAFETY: as above. Windows reports the current state right away.
    let display = unsafe {
        RegisterPowerSettingNotification(
            HANDLE(window.0),
            &GUID_CONSOLE_DISPLAY_STATE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    };
    if let Err(e) = &display {
        warning!("no display notifications ({e}); the hooks may not come back after it");
    }
    Watch {
        window,
        session: session.is_ok(),
        display: display.ok(),
    }
}

/// If the message says the computer woke up, the session was unlocked or the
/// display turned on, says which, for the log.
pub fn woke_up(message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<&'static str> {
    match (message, wparam.0 as u32) {
        (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC) => Some("the computer woke up"),
        (WM_POWERBROADCAST, PBT_POWERSETTINGCHANGE) => {
            // SAFETY: with PBT_POWERSETTINGCHANGE, lparam points to a
            // POWERBROADCAST_SETTING followed by `DataLength` bytes of data.
            let state = unsafe { display_state(lparam) }?;
            let before = DISPLAY.swap(state, Ordering::SeqCst);
            display_turned_on(before, state).then_some("the display turned on")
        }
        (WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK) => Some("the session was unlocked"),
        (WM_WTSSESSION_CHANGE, WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT) => {
            Some("the session was connected again")
        }
        _ => None,
    }
}

/// Reads the display state from a power setting notification.
///
/// # Safety
/// `lparam` must come with a `PBT_POWERSETTINGCHANGE` notification.
unsafe fn display_state(lparam: LPARAM) -> Option<u32> {
    let setting = lparam.0 as *const POWERBROADCAST_SETTING;
    // SAFETY: the caller's promise. The data is read through a raw pointer
    // because it runs past the 1-byte `Data` field of the declared type.
    unsafe {
        if (*setting).PowerSetting != GUID_CONSOLE_DISPLAY_STATE || (*setting).DataLength < 4 {
            return None;
        }
        let data = std::ptr::addr_of!((*setting).Data) as *const u32;
        Some(data.read_unaligned())
    }
}

/// Only a display coming back from off counts: dimming and the first report
/// (sent as soon as the engine registers) do not.
fn display_turned_on(before: u32, now: u32) -> bool {
    before == DISPLAY_OFF && now == DISPLAY_ON
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_display_coming_back_from_off_counts() {
        assert!(display_turned_on(DISPLAY_OFF, DISPLAY_ON));
        // The first report, at registration.
        assert!(!display_turned_on(DISPLAY_UNKNOWN, DISPLAY_ON));
        // Back from dimmed, or turning off.
        assert!(!display_turned_on(2, DISPLAY_ON));
        assert!(!display_turned_on(DISPLAY_ON, DISPLAY_OFF));
    }

    #[test]
    fn wake_up_messages_are_recognized() {
        let none = LPARAM(0);
        let resume = WPARAM(PBT_APMRESUMEAUTOMATIC as usize);
        assert!(woke_up(WM_POWERBROADCAST, resume, none).is_some());
        let unlock = WPARAM(WTS_SESSION_UNLOCK as usize);
        assert!(woke_up(WM_WTSSESSION_CHANGE, unlock, none).is_some());
        // Locking the session is not waking up.
        assert!(woke_up(WM_WTSSESSION_CHANGE, WPARAM(7), none).is_none());
    }

    #[test]
    fn display_state_is_read_from_the_notification() {
        // POWERBROADCAST_SETTING with 4 bytes of data, kept aligned.
        #[repr(C)]
        struct Setting {
            guid: GUID,
            length: u32,
            data: u32,
        }
        let on = Setting {
            guid: GUID_CONSOLE_DISPLAY_STATE,
            length: 4,
            data: DISPLAY_ON,
        };
        let lparam = LPARAM(&on as *const Setting as isize);
        // SAFETY: `on` has the layout of the notification and outlives the call.
        assert_eq!(unsafe { display_state(lparam) }, Some(DISPLAY_ON));

        let other = Setting {
            guid: GUID::from_u128(1),
            ..on
        };
        let lparam = LPARAM(&other as *const Setting as isize);
        // SAFETY: as above.
        assert_eq!(unsafe { display_state(lparam) }, None);
    }
}
