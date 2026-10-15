//! Area and scale of each monitor, so the gesture threshold and the ring have
//! the same visual size on any screen.

use thumbring_core::gesture::Point;
use thumbring_core::screen::Rect;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
    SetProcessDpiAwarenessContext,
};

/// Without this, Windows reports 96 DPI to the process on every monitor.
pub fn enable_per_monitor_awareness() -> windows::core::Result<()> {
    // SAFETY: only changes a setting of the process itself.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
}

/// Scale factor of the monitor under `pos` (1.0 = 100% = 96 DPI).
pub fn scale_at(pos: Point) -> f64 {
    let (mut x, mut y) = (0u32, 0u32);
    // SAFETY: the pointers point to valid local variables.
    let result = unsafe {
        let monitor = MonitorFromPoint(POINT { x: pos.x, y: pos.y }, MONITOR_DEFAULTTONEAREST);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y)
    };
    match result {
        Ok(()) if x > 0 => f64::from(x) / 96.0,
        _ => 1.0,
    }
}

/// Area of the monitor under `pos` (or the nearest one), in physical pixels.
pub fn monitor_at(pos: Point) -> windows::core::Result<Rect> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid local with its size filled in.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: pos.x, y: pos.y }, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err(windows::core::Error::from_thread());
        }
    }
    let r = info.rcMonitor;
    Ok(Rect::new(r.left, r.top, r.right, r.bottom))
}
