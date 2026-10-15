//! Mouse and keyboard events sent with `SendInput`.
//!
//! Every event carries [`SIGNATURE`] in `dwExtraInfo`.

use thumbring_core::command::{Chord, Key, Modifiers};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, MOUSE_EVENT_FLAGS, MOUSEEVENTF_XDOWN,
    MOUSEEVENTF_XUP, MOUSEINPUT, MapVirtualKeyW, SendInput, VIRTUAL_KEY, VK_APPS, VK_BACK,
    VK_BROWSER_BACK, VK_BROWSER_FORWARD, VK_BROWSER_REFRESH, VK_CONTROL, VK_DELETE, VK_DOWN,
    VK_END, VK_ESCAPE, VK_F1, VK_HOME, VK_INSERT, VK_LEFT, VK_LWIN, VK_MEDIA_NEXT_TRACK,
    VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_STOP, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN,
    VK_RIGHT, VK_SHIFT, VK_SNAPSHOT, VK_SPACE, VK_TAB, VK_UP, VK_VOLUME_DOWN, VK_VOLUME_MUTE,
    VK_VOLUME_UP, VkKeyScanExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, XBUTTON1, XBUTTON2,
};

/// Signature the engine writes into `dwExtraInfo` of the events it injects,
/// so its own hook recognizes them and lets them through. These are the ASCII
/// bytes of "THMB".
pub const SIGNATURE: usize = 0x5448_4D42;

/// Sends a Back button click (press and release) again at the current position.
pub fn click_back() -> Result<(), String> {
    click_extra(XBUTTON1)
}

/// Sends a Forward button click (press and release) again at the current
/// position.
pub fn click_forward() -> Result<(), String> {
    click_extra(XBUTTON2)
}

fn click_extra(button: u16) -> Result<(), String> {
    send(&[
        extra_button(button, MOUSEEVENTF_XDOWN),
        extra_button(button, MOUSEEVENTF_XUP),
    ])
}

fn extra_button(button: u16, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: u32::from(button),
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

/// Presses and releases a shortcut: the modifiers, the key, and then the
/// releases in reverse order. Everything goes in a single `SendInput`, so no
/// real key press can get in between.
pub fn send_chord(chord: &Chord) -> Result<(), String> {
    let (key, needs) = virtual_key(chord.key)?;
    let modifiers = chord.modifiers.with(needs);
    let mut downs: Vec<VIRTUAL_KEY> = modifier_keys(modifiers).collect();
    downs.push(key);

    let mut inputs: Vec<INPUT> = downs.iter().map(|&vk| keyboard(vk, false)).collect();
    inputs.extend(downs.iter().rev().map(|&vk| keyboard(vk, true)));
    send(&inputs)
}

/// In the same order as the text: Ctrl, Win, Alt, Shift.
fn modifier_keys(m: Modifiers) -> impl Iterator<Item = VIRTUAL_KEY> {
    [
        (m.ctrl, VK_CONTROL),
        (m.win, VK_LWIN),
        (m.alt, VK_MENU),
        (m.shift, VK_SHIFT),
    ]
    .into_iter()
    .filter_map(|(on, vk)| on.then_some(vk))
}

/// Key code, plus the modifiers the character needs in the current layout
/// (for example, `+` needs Shift on many layouts).
fn virtual_key(key: Key) -> Result<(VIRTUAL_KEY, Modifiers), String> {
    let vk = match key {
        // Letters and digits have the same code in every layout.
        Key::Char(c) if c.is_ascii_alphanumeric() => VIRTUAL_KEY(c.to_ascii_uppercase() as u16),
        Key::Char(c) => return char_key(c),
        Key::F(n) => VIRTUAL_KEY(VK_F1.0 + u16::from(n) - 1),
        Key::Left => VK_LEFT,
        Key::Right => VK_RIGHT,
        Key::Up => VK_UP,
        Key::Down => VK_DOWN,
        Key::Tab => VK_TAB,
        Key::Enter => VK_RETURN,
        Key::Esc => VK_ESCAPE,
        Key::Space => VK_SPACE,
        Key::Backspace => VK_BACK,
        Key::Delete => VK_DELETE,
        Key::Insert => VK_INSERT,
        Key::Home => VK_HOME,
        Key::End => VK_END,
        Key::PageUp => VK_PRIOR,
        Key::PageDown => VK_NEXT,
        Key::PrintScreen => VK_SNAPSHOT,
        Key::Apps => VK_APPS,
        Key::VolumeUp => VK_VOLUME_UP,
        Key::VolumeDown => VK_VOLUME_DOWN,
        Key::VolumeMute => VK_VOLUME_MUTE,
        Key::PlayPause => VK_MEDIA_PLAY_PAUSE,
        Key::NextTrack => VK_MEDIA_NEXT_TRACK,
        Key::PrevTrack => VK_MEDIA_PREV_TRACK,
        Key::MediaStop => VK_MEDIA_STOP,
        Key::BrowserBack => VK_BROWSER_BACK,
        Key::BrowserForward => VK_BROWSER_FORWARD,
        Key::BrowserRefresh => VK_BROWSER_REFRESH,
    };
    Ok((vk, Modifiers::NONE))
}

/// Key that types `c` in the keyboard layout of the focused app.
fn char_key(c: char) -> Result<(VIRTUAL_KEY, Modifiers), String> {
    let mut units = [0u16; 2];
    let [unit] = c.encode_utf16(&mut units) else {
        return Err(format!("the character \"{c}\" does not fit in one key"));
    };
    // SAFETY: queries only; a null window gives the current thread's layout.
    let scan = unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        VkKeyScanExW(*unit, GetKeyboardLayout(thread))
    };
    if scan == -1 {
        return Err(format!("the current keyboard layout has no \"{c}\" key"));
    }
    // Low byte: key code. High byte: 1 = Shift, 2 = Ctrl, 4 = Alt.
    let state = (scan >> 8) & 0xFF;
    let needs = Modifiers {
        shift: state & 1 != 0,
        ctrl: state & 2 != 0,
        alt: state & 4 != 0,
        win: false,
    };
    Ok((VIRTUAL_KEY((scan & 0xFF) as u16), needs))
}

/// Keys that Windows tells apart from their numeric keypad twins (or that
/// only exist as "extended" keys). Without the flag, Ctrl+Win+Left could turn
/// into Ctrl+Win+4 on the keypad.
fn is_extended(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_LEFT
            | VK_RIGHT
            | VK_UP
            | VK_DOWN
            | VK_INSERT
            | VK_DELETE
            | VK_HOME
            | VK_END
            | VK_PRIOR
            | VK_NEXT
            | VK_LWIN
            | VK_APPS
            | VK_SNAPSHOT
            | VK_VOLUME_UP
            | VK_VOLUME_DOWN
            | VK_VOLUME_MUTE
            | VK_MEDIA_PLAY_PAUSE
            | VK_MEDIA_NEXT_TRACK
            | VK_MEDIA_PREV_TRACK
            | VK_MEDIA_STOP
            | VK_BROWSER_BACK
            | VK_BROWSER_FORWARD
            | VK_BROWSER_REFRESH
    )
}

fn keyboard(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if is_extended(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    // SAFETY: only converts a key code.
    let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> Result<(), String> {
    // SAFETY: `inputs` is a valid slice and the size given is INPUT's.
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!(
            "Windows accepted {sent} of {} events: {}",
            inputs.len(),
            windows::core::Error::from_thread()
        ))
    }
}
