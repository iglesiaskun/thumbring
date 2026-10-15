//! Build script: embeds the app icon, the version information and the
//! application manifest into thumbring.exe.
//!
//! The icon is drawn by the same code as the tray icon (`src/art.rs`), so
//! there is no image file to keep in sync. The version comes from
//! `Cargo.toml`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// Shared with the engine; the build script uses only part of each.
#[allow(dead_code)]
#[path = "src/art.rs"]
mod art;
#[allow(dead_code)]
#[path = "src/ring/png.rs"]
mod png;

/// Sizes Windows picks from, from the tray and small icons (16 px at 100%
/// scaling) up to Explorer's extra large icons (256 px).
const ICON_SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/art.rs");
    println!("cargo:rerun-if-changed=src/ring/png.rs");
    println!("cargo:rerun-if-changed=thumbring.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let package_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets it"));
    let manifest = package_dir.join("thumbring.manifest");
    let icon = out_dir.join("thumbring.ico");
    fs::write(&icon, icon_file()).expect("could not write the app icon");

    // FileVersion and ProductVersion come from the package version.
    // FileDescription is the name Task Manager shows for the process.
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(utf8(&icon))
        .set_manifest_file(utf8(&manifest))
        .set("ProductName", "Thumbring")
        .set("FileDescription", "Thumbring")
        .set("CompanyName", "Thumbring")
        .set("LegalCopyright", "Copyright (c) 2026 Pedro Iglesias")
        .set("OriginalFilename", "thumbring.exe")
        .set("InternalName", "thumbring");
    resource
        .compile()
        .expect("could not compile the Windows resources (is the Windows SDK installed?)");
}

fn utf8(path: &Path) -> &str {
    path.to_str().expect("the build folders have Unicode paths")
}

/// An `.ico` file with one PNG image per size.
fn icon_file() -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = ICON_SIZES
        .iter()
        .map(|&size| {
            let png = png::encode_rgba_with(size, size, &art::app(size), |raw| {
                miniz_oxide::deflate::compress_to_vec_zlib(raw, 10)
            });
            (size, png)
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
