//! The app icon as a Windows `.ico` file, with one PNG image per size drawn
//! by [`art::app`].
//!
//! Only build scripts use it: the engine's and the settings app's, so both
//! executables embed the same icon. They include this file, `art.rs` and
//! `ring/png.rs` with `#[path]`, as modules of the same names.

use super::{art, png};

/// Sizes Windows picks from, from the tray and small icons (16 px at 100%
/// scaling) up to Explorer's extra large icons (256 px).
const SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

/// The `.ico` file. `zlib` compresses the rows of each PNG image.
pub fn app_icon(zlib: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = SIZES
        .iter()
        .map(|&size| {
            (
                size,
                png::encode_rgba_with(size, size, &art::app(size), &zlib),
            )
        })
        .collect();

    // ICONDIR: reserved, type (1 = icon), number of images.
    let mut ico = Vec::new();
    for field in [0u16, 1, images.len() as u16] {
        ico.extend_from_slice(&field.to_le_bytes());
    }
    // One ICONDIRENTRY per image, then the images themselves.
    let mut offset = 6 + 16 * images.len();
    for (size, png) in &images {
        // A width and height of 0 mean 256.
        let side = if *size >= 256 { 0 } else { *size as u8 };
        // Width, height, palette size, reserved.
        ico.extend_from_slice(&[side, side, 0, 0]);
        // Color planes and bits per pixel.
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        ico.extend_from_slice(&png);
    }
    ico
}
