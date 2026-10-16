use std::fs;
use std::path::PathBuf;

use thumbring_core::ring::MAX_ICON_FILE_BYTES;
use thumbring_settings::icons::{IconFile, check_file, expand, read_file};

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("thumbring-icon-files-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn variables_are_expanded_and_unknown_ones_stay() {
    let path = std::env::var("PATH").unwrap();
    assert_eq!(expand("%PATH%"), path);
    assert_eq!(expand("a%PATH%b"), format!("a{path}b"));
    assert_eq!(
        expand(r"%NO_SUCH_THUMBRING_VAR%\star.svg"),
        r"%NO_SUCH_THUMBRING_VAR%\star.svg"
    );
    assert_eq!(expand("100%"), "100%");
}

#[test]
fn an_icon_file_needs_a_full_path_to_a_file_that_exists() {
    assert!(
        check_file("star.svg")
            .unwrap_err()
            .contains("is not a full path")
    );
    let dir = folder("missing");
    let missing = dir.join("star.svg");
    let error = check_file(missing.to_str().unwrap()).unwrap_err();
    assert!(error.ends_with("was not found"), "{error}");
    let error = check_file(dir.to_str().unwrap()).unwrap_err();
    assert!(error.ends_with("is not a file"), "{error}");
}

#[test]
fn a_huge_icon_file_is_refused() {
    let big = folder("big").join("big.png");
    fs::write(&big, vec![0u8; MAX_ICON_FILE_BYTES as usize + 1]).unwrap();
    let error = check_file(big.to_str().unwrap()).unwrap_err();
    assert!(error.ends_with("is bigger than 2 MB"), "{error}");
}

#[test]
fn svg_comes_as_text_and_png_as_bytes() {
    let dir = folder("read");
    let svg = dir.join("Star.SVG");
    fs::write(&svg, "<svg viewBox=\"0 0 24 24\"></svg>").unwrap();
    assert_eq!(
        read_file(svg.to_str().unwrap()),
        Ok(IconFile::Svg {
            text: "<svg viewBox=\"0 0 24 24\"></svg>".into()
        })
    );
    let png = dir.join("dot.png");
    fs::write(&png, [0x89, b'P', b'N', b'G']).unwrap();
    assert_eq!(
        read_file(png.to_str().unwrap()),
        Ok(IconFile::Png {
            bytes: vec![0x89, b'P', b'N', b'G']
        })
    );
    let other = dir.join("notes.txt");
    fs::write(&other, "hi").unwrap();
    assert!(read_file(other.to_str().unwrap()).is_err());
}
