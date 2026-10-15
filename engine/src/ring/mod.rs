pub mod controller;
pub mod icons;
pub mod overlay;
mod png;
pub mod preview;
pub mod render;

use thumbring_core::gesture::Point;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Cursor position, in physical pixels.
fn cursor() -> Point {
    let mut pos = POINT::default();
    // SAFETY: `pos` is a valid local. On failure it stays at (0, 0).
    let _ = unsafe { GetCursorPos(&mut pos) };
    Point::new(pos.x, pos.y)
}
