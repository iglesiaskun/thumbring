//! The icons a ring item can use, for the interface to draw them as the
//! engine does: the same Lucide files the engine embeds, from
//! `engine/assets/icons`, and the SVG or PNG files of the user's own. A test
//! keeps the list of built-in icons equal to the engine's.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Serialize;
use thumbring_core::ring::{IconFormat, IconSource, MAX_ICON_FILE_BYTES};

macro_rules! ring_icons {
    ($($name:literal),* $(,)?) => {
        /// Name and SVG text of each icon, in the order of
        /// `thumbring_core::ring::ICONS`.
        pub const RING_ICONS: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../../engine/assets/icons/", $name, ".svg")))),*
        ];
    };
}

ring_icons![
    "calculator",
    "calendar",
    "clipboard",
    "clipboard-list",
    "clipboard-paste",
    "code",
    "copy",
    "folder-open",
    "globe",
    "headphones",
    "layout-grid",
    "lock",
    "mail",
    "music",
    "play",
    "redo-2",
    "scan",
    "scissors",
    "settings",
    "skip-back",
    "skip-forward",
    "smile",
    "sticky-note",
    "terminal",
    "undo-2",
    "volume-1",
    "volume-2",
    "volume-x",
    "x",
];

/// An icon file of the user's own, for the interface to draw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "format", rename_all = "lowercase")]
pub enum IconFile {
    Svg { text: String },
    Png { bytes: Vec<u8> },
}

/// The icon file a path from the settings names, checked as the engine
/// checks it, with the same messages: its `%VARIABLES%` expanded, a full path
/// to a file that exists and is not too big.
pub fn check_file(path: &str) -> Result<PathBuf, String> {
    let file = PathBuf::from(expand(path));
    if !file.is_absolute() {
        return Err(format!(
            "\"{path}\" is not a full path, such as C:\\Users\\you\\Pictures\\star.svg"
        ));
    }
    match fs::metadata(&file) {
        Ok(meta) if !meta.is_file() => Err(format!("{} is not a file", file.display())),
        Ok(meta) if meta.len() > MAX_ICON_FILE_BYTES => Err(format!(
            "{} is bigger than {} MB",
            file.display(),
            MAX_ICON_FILE_BYTES / 1024 / 1024
        )),
        Ok(_) => Ok(file),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(format!("{} was not found", file.display()))
        }
        Err(e) => Err(format!("could not open {}: {e}", file.display())),
    }
}

/// Reads an icon file for the interface.
pub fn read_file(path: &str) -> Result<IconFile, String> {
    let Some(IconSource::File(_, format)) = IconSource::of(path) else {
        return Err(format!("\"{path}\" is not an .svg or .png file"));
    };
    let file = check_file(path)?;
    let bytes = fs::read(&file).map_err(|e| format!("could not read {}: {e}", file.display()))?;
    Ok(match format {
        IconFormat::Svg => IconFile::Svg {
            text: String::from_utf8(bytes)
                .map_err(|_| format!("{} is not a text file", file.display()))?,
        },
        IconFormat::Png => IconFile::Png { bytes },
    })
}

/// Replaces each `%NAME%` with the value of that environment variable, as
/// Windows does for the engine. Unknown names stay as they are.
pub fn expand(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(length) = after.find('%') else {
            break;
        };
        let name = &after[..length];
        match std::env::var(name).ok().filter(|_| !name.is_empty()) {
            Some(value) => {
                out.push_str(&rest[..start]);
                out.push_str(&value);
                rest = &after[length + 1..];
            }
            None => {
                out.push_str(&rest[..=start]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_icons_as_the_engine() {
        let names: Vec<&str> = RING_ICONS.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, thumbring_core::ring::ICONS);
        for (name, svg) in RING_ICONS {
            assert!(svg.contains("<svg"), "{name}");
        }
    }
}
