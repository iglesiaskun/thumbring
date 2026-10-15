//! The Thumbring mark, drawn in code: a ring with a dot at the top, like the
//! highlighted slot. Both the tray icon and the app icon come from here; the
//! build script turns the app icon into the `.ico` embedded in the
//! executable.
//!
//! Plain Rust with no Windows calls, so `build.rs` can use it too.

/// How the tray icon looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Gestures on: full ring with the dot at the top.
    Active,
    /// Off in an excluded app or in fullscreen: dimmed ring.
    Inactive,
    /// Paused from the tray: dimmed ring with a pause sign in the middle.
    Paused,
}

/// Tray icon: one color on a transparent background, as Windows 11 draws
/// its own tray icons. RGBA pixels, top to bottom, with straight (not
/// premultiplied) alpha.
pub fn tray(size: u32, look: Look, light_taskbar: bool) -> Vec<u8> {
    let s = f64::from(size);
    let center = s / 2.0;
    let ring_radius = s * 0.34;
    let half_stroke = (s * 0.075).max(0.9);
    let dot = (center, center - ring_radius);
    let dot_radius = s * 0.15;
    let ring_alpha = if look == Look::Active { 1.0 } else { 0.45 };
    // Light taskbar: dark drawing. Dark taskbar: white drawing.
    let (r, g, b) = if light_taskbar {
        (0x1b, 0x1b, 0x1f)
    } else {
        (0xff, 0xff, 0xff)
    };

    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let from_center = (px - center).hypot(py - center);
            let ring = coverage((from_center - ring_radius).abs() - half_stroke) * ring_alpha;
            let mark = match look {
                Look::Active => coverage((px - dot.0).hypot(py - dot.1) - dot_radius),
                Look::Paused => pause_bars(px - center, py - center, s),
                Look::Inactive => 0.0,
            };
            let alpha = ring.max(mark);
            out.extend_from_slice(&[r, g, b, (alpha * 255.0).round() as u8]);
        }
    }
    out
}

/// App icon (executable, Start menu, installer): the tray mark, with a
/// thinner ring, in white on a dark disc, so it reads on light and dark
/// backgrounds alike. RGBA pixels, top to bottom, with straight alpha.
// Outside the tests, only build.rs draws it.
#[allow(dead_code)]
pub fn app(size: u32) -> Vec<u8> {
    let s = f64::from(size);
    let center = s / 2.0;
    let disc_radius = s * 0.47;
    let ring_radius = s * 0.26;
    let half_stroke = (s * 0.05).max(0.8);
    let dot = (center, center - ring_radius);
    let dot_radius = s * 0.12;

    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let from_center = (px - center).hypot(py - center);
            let disc = coverage(from_center - disc_radius);
            let ring = coverage((from_center - ring_radius).abs() - half_stroke);
            let mark = ring.max(coverage((px - dot.0).hypot(py - dot.1) - dot_radius));
            // The disc is a little lighter at the top.
            let t = py / s;
            let back = |top: f64, bottom: f64| top + (bottom - top) * t;
            let back = [back(52.0, 21.0), back(52.0, 21.0), back(58.0, 24.0)];
            let [r, g, b] = back.map(|c| (c + (255.0 - c) * mark).round() as u8);
            out.extend_from_slice(&[r, g, b, (disc.max(mark) * 255.0).round() as u8]);
        }
    }
    out
}

/// Covered fraction of a pixel, from the signed distance to the shape's edge
/// (in pixels; negative inside). Gives a soft 1 px edge.
fn coverage(distance: f64) -> f64 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Two vertical bars in the middle of the ring.
fn pause_bars(dx: f64, dy: f64, s: f64) -> f64 {
    let (half_w, half_h, offset) = (s * 0.05, s * 0.15, s * 0.09);
    let bar = |cx: f64| {
        let qx = (dx - cx).abs() - half_w;
        let qy = dy.abs() - half_h;
        coverage(qx.max(qy))
    };
    bar(-offset).max(bar(offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(px: &[u8], size: u32, x: u32, y: u32) -> u8 {
        px[((y * size + x) * 4 + 3) as usize]
    }

    fn rgb(px: &[u8], size: u32, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * size + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    }

    #[test]
    fn has_one_rgba_pixel_per_point() {
        for size in [16, 20, 24, 32, 256] {
            let n = (size * size * 4) as usize;
            assert_eq!(tray(size, Look::Active, false).len(), n);
            assert_eq!(app(size).len(), n);
        }
    }

    #[test]
    fn active_ring_is_opaque_and_hollow() {
        let px = tray(32, Look::Active, false);
        // Empty center, opaque ring edge, empty corners.
        assert_eq!(alpha(&px, 32, 16, 16), 0);
        assert_eq!(alpha(&px, 32, 16 + 11, 16), 255);
        assert_eq!(alpha(&px, 32, 0, 0), 0);
        // The dot at the top is wider than the ring stroke.
        assert_eq!(alpha(&px, 32, 16 + 3, 5), 255);
        assert_eq!(alpha(&px, 32, 16 + 11, 16 + 3), 255);
    }

    #[test]
    fn inactive_and_paused_are_dimmed() {
        let inactive = tray(32, Look::Inactive, false);
        assert!((0..32 * 32).all(|i| inactive[i * 4 + 3] <= 115));
        assert!(inactive.chunks(4).any(|p| p[3] > 100));

        let paused = tray(32, Look::Paused, false);
        // The pause bars are opaque in the middle of the ring.
        assert_eq!(alpha(&paused, 32, 16 - 3, 16), 255);
        assert_eq!(alpha(&paused, 32, 16, 16), 0);
    }

    #[test]
    fn tray_color_follows_the_taskbar_theme() {
        let dark = tray(16, Look::Active, false);
        let light = tray(16, Look::Active, true);
        assert_eq!(&dark[..3], &[0xff, 0xff, 0xff]);
        assert_eq!(&light[..3], &[0x1b, 0x1b, 0x1f]);
    }

    #[test]
    fn app_icon_is_a_white_mark_on_a_dark_disc() {
        let size = 64;
        let px = app(size);
        // Transparent corners, opaque disc.
        assert_eq!(alpha(&px, size, 0, 0), 0);
        assert_eq!(alpha(&px, size, 32, 32), 255);
        assert_eq!(alpha(&px, size, 32, 60), 255);
        // Dark in the middle of the ring and near the edge of the disc.
        assert!(rgb(&px, size, 32, 32).iter().all(|&c| c < 60));
        assert!(rgb(&px, size, 32, 60).iter().all(|&c| c < 60));
        // White on the ring (right side) and on the dot (top).
        assert_eq!(rgb(&px, size, 32 + 16, 32), [255, 255, 255]);
        assert_eq!(rgb(&px, size, 32, 15), [255, 255, 255]);
    }
}
