//! The window the ring is drawn on.
//!
//! It covers the monitor where the ring opens, stays above everything, never
//! takes focus (the focused app keeps it the whole time) and lets the mouse
//! through: while the ring is open, the engine reads the clicks from its own
//! hook. Its pixels come from DirectComposition, with per-pixel transparency.
//!
//! The engine creates it once, hidden, and moves it to the monitor under the
//! cursor every time the ring opens.

use std::sync::Once;

use thumbring_core::screen::Rect;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HTTRANSPARENT, HWND_TOPMOST, LWA_ALPHA,
    MA_NOACTIVATE, RegisterClassExW, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WM_MOUSEACTIVATE,
    WM_NCHITTEST, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use super::render::Renderer;

const CLASS: PCWSTR = w!("Thumbring.Ring");

pub struct Overlay {
    window: HWND,
    /// Monitor the window covers, in physical pixels, and its scale.
    monitor: Rect,
    scale: f64,
    pub renderer: Renderer,
}

impl Overlay {
    /// Creates the window over `monitor` (physical pixels), whose scale is
    /// `scale` (1.0 = 100%), still hidden.
    pub fn create(monitor: Rect, scale: f64) -> windows::core::Result<Self> {
        register_class()?;
        let (width, height) = size(monitor);
        // SAFETY: plain window creation; the window is destroyed in `drop`.
        let window = unsafe {
            let instance = GetModuleHandleW(None)?;
            let window = CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT,
                CLASS,
                w!("Thumbring ring"),
                WS_POPUP,
                monitor.left,
                monitor.top,
                width as i32,
                height as i32,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            // A layered window shows nothing until it gets an opacity.
            SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA)?;
            window
        };
        let renderer = match Renderer::new(window, width, height, scale) {
            Ok(renderer) => renderer,
            Err(e) => {
                // SAFETY: the window was created above and is not used again.
                let _ = unsafe { DestroyWindow(window) };
                return Err(e);
            }
        };
        Ok(Self {
            window,
            monitor,
            scale,
            renderer,
        })
    }

    /// Creates the window and shows it right away.
    pub fn open(monitor: Rect, scale: f64) -> windows::core::Result<Self> {
        let overlay = Self::create(monitor, scale)?;
        overlay.show();
        Ok(overlay)
    }

    /// Moves the window to cover `monitor`, if it covers another one.
    pub fn cover(&mut self, monitor: Rect, scale: f64) -> windows::core::Result<()> {
        if monitor == self.monitor && scale == self.scale {
            return Ok(());
        }
        let (width, height) = size(monitor);
        // SAFETY: moves and resizes a window owned by this thread.
        unsafe {
            SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                monitor.left,
                monitor.top,
                width as i32,
                height as i32,
                SWP_NOACTIVATE,
            )?;
        }
        self.renderer.resize(width, height, scale)?;
        self.monitor = monitor;
        self.scale = scale;
        Ok(())
    }

    /// Shows the window above every other topmost window, without taking
    /// focus.
    pub fn show(&self) {
        // SAFETY: changes only the z-order and visibility of our own window.
        unsafe {
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    /// Monitor the window covers, and its scale.
    pub fn placement(&self) -> (Rect, f64) {
        (self.monitor, self.scale)
    }

    pub fn hide(&self) {
        // SAFETY: hides our own window.
        unsafe {
            let _ = ShowWindow(self.window, SW_HIDE);
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        // SAFETY: the window belongs to this thread and is destroyed once.
        let _ = unsafe { DestroyWindow(self.window) };
    }
}

fn size(monitor: Rect) -> (u32, u32) {
    (
        (monitor.right - monitor.left).max(1) as u32,
        (monitor.bottom - monitor.top).max(1) as u32,
    )
}

fn register_class() -> windows::core::Result<()> {
    static REGISTER: Once = Once::new();
    let mut result = Ok(());
    REGISTER.call_once(|| {
        // SAFETY: registers a class with a static window procedure.
        result = unsafe {
            GetModuleHandleW(None).and_then(|instance| {
                let class = WNDCLASSEXW {
                    cbSize: size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(wndproc),
                    hInstance: instance.into(),
                    lpszClassName: CLASS,
                    ..Default::default()
                };
                if RegisterClassExW(&class) == 0 {
                    Err(windows::core::Error::from_thread())
                } else {
                    Ok(())
                }
            })
        };
    });
    result
}

unsafe extern "system" fn wndproc(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // The mouse goes to whatever is below.
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        // SAFETY: passes the same arguments along.
        _ => unsafe { DefWindowProcW(window, msg, wparam, lparam) },
    }
}
