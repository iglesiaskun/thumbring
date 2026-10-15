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
#[path = "src/ico.rs"]
mod ico;
#[allow(dead_code)]
#[path = "src/ring/png.rs"]
mod png;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/art.rs");
    println!("cargo:rerun-if-changed=src/ico.rs");
    println!("cargo:rerun-if-changed=src/ring/png.rs");
    println!("cargo:rerun-if-changed=thumbring.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let package_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets it"));
    let manifest = package_dir.join("thumbring.manifest");
    let icon = out_dir.join("thumbring.ico");
    let ico = ico::app_icon(|raw| miniz_oxide::deflate::compress_to_vec_zlib(raw, 10));
    fs::write(&icon, ico).expect("could not write the app icon");

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
