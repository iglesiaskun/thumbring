//! Tray icon: the mark from [`art`](crate::art), turned into an `HICON` at
//! runtime, in the color of the taskbar theme.

use windows::Win32::Graphics::Gdi::{
    BI_BITFIELDS, BITMAPINFO, BITMAPV5HEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};
use windows::core::w;

use crate::art::{self, Look};

/// Is the taskbar using the light theme?
pub fn light_taskbar() -> bool {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `value` and `size` are locals with the size of a DWORD.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    result.is_ok() && value != 0
}

/// Creates the icon. The caller must free it with `DestroyIcon`.
pub fn create(size: i32, look: Look, light_taskbar: bool) -> windows::core::Result<HICON> {
    let pixels = bgra(art::tray(size as u32, look, light_taskbar));
    let header = BITMAPV5HEADER {
        bV5Size: size_of::<BITMAPV5HEADER>() as u32,
        bV5Width: size,
        // Negative height: rows go top to bottom, as in `art`.
        bV5Height: -size,
        bV5Planes: 1,
        bV5BitCount: 32,
        bV5Compression: BI_BITFIELDS,
        bV5RedMask: 0x00FF_0000,
        bV5GreenMask: 0x0000_FF00,
        bV5BlueMask: 0x0000_00FF,
        bV5AlphaMask: 0xFF00_0000,
        ..Default::default()
    };
    // All-zero monochrome mask: with per-pixel alpha it only has to exist.
    let mask_bits = vec![0u8; (size as usize).div_ceil(16) * 2 * size as usize];
    let mut bits = std::ptr::null_mut();
    // SAFETY: the V5 header starts with the same fields as BITMAPINFO; the
    // returned buffer holds `size * size * 4` bytes; the bitmaps are freed
    // after being copied into the icon.
    unsafe {
        let color = CreateDIBSection(
            None,
            &header as *const BITMAPV5HEADER as *const BITMAPINFO,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
        let mask = CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr() as *const _));
        let info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info);
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}

/// The bitmap wants blue first.
fn bgra(mut rgba: Vec<u8>) -> Vec<u8> {
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blue_comes_first() {
        assert_eq!(bgra(vec![1, 2, 3, 4, 5, 6, 7, 8]), [3, 2, 1, 4, 7, 6, 5, 8]);
    }
}
