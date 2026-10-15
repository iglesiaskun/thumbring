//! Build script: embeds the interface (ui/), the app icon, the version
//! information and the application manifest into thumbring-settings.exe.
//!
//! The icon is drawn by the engine's own code (`engine/src/art.rs`), so both
//! programs share it without an image file to keep in sync.

use std::env;
use std::fs;
use std::path::PathBuf;

// Shared with the engine; the build script uses only part of each.
#[allow(dead_code)]
#[path = "../engine/src/art.rs"]
mod art;
#[path = "../engine/src/ico.rs"]
mod ico;
#[allow(dead_code)]
#[path = "../engine/src/ring/png.rs"]
mod png;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../engine/src/art.rs");
    println!("cargo:rerun-if-changed=../engine/src/ico.rs");
    println!("cargo:rerun-if-changed=../engine/src/ring/png.rs");
    println!("cargo:rerun-if-changed=../engine/thumbring.manifest");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let icon = out_dir.join("thumbring-settings.ico");
    let ico = ico::app_icon(|raw| miniz_oxide::deflate::compress_to_vec_zlib(raw, 10));
    fs::write(&icon, ico).expect("could not write the app icon");

    // The engine's manifest: never asks for administrator rights, made for
    // Windows 10 and 11, current controls. The version comes from Cargo.toml
    // and the other version fields from tauri.conf.json.
    let windows = tauri_build::WindowsAttributes::new()
        .window_icon_path(&icon)
        .app_manifest(include_str!("../engine/thumbring.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("could not build the settings app resources");
}
