//! Item icons: the Lucide icons (ISC license, see `assets/icons/LICENSE-LUCIDE`)
//! embedded in the executable, and SVG or PNG files of the user's own.
//!
//! The Lucide files are used as they come; [`svg`] only swaps the stroke color
//! and width, which Direct2D cannot take from the context. [`user_svg`] does
//! the same for an SVG file of the user's own.

use std::fs;
use std::io;
use std::path::PathBuf;

/// Icon files bigger than this are refused: an icon needs far less, and a
/// huge picture would hold up the ring while it loads.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../../assets/icons/", $name, ".svg")))),*
        ];
    };
}

icons![
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

/// The SVG of an icon drawn `size` pixels wide, with this stroke color (like
/// `#2A2B31`) and width, or `None` if there is no icon with that name.
///
/// Lucide files say `width="24" height="24"`, and Direct2D draws at that size
/// whatever viewport it gets: the size goes into the file itself so the icon
/// fills exactly the square it is centered on.
pub fn svg(name: &str, color: &str, stroke_width: f32, size: f32) -> Option<String> {
    let (_, source) = ICONS.iter().find(|(n, _)| *n == name)?;
    // The license comment stays in the files; the parser does not need it.
    let body = source
        .find("<svg")
        .map_or(*source, |start| &source[start..]);
    Some(
        body.replace("stroke=\"currentColor\"", &format!("stroke=\"{color}\""))
            .replace(
                "stroke-width=\"2\"",
                &format!("stroke-width=\"{stroke_width}\""),
            )
            .replace("width=\"24\"", &format!("width=\"{size}\""))
            .replace("height=\"24\"", &format!("height=\"{size}\"")),
    )
}

/// The icon file a path from the settings names, with its `%VARIABLES%`
/// expanded, if it can be used: a full path to a file that exists and is not
/// too big. Whether its picture can be drawn shows only when it is drawn.
pub fn file(path: &str) -> Result<PathBuf, String> {
    let file = PathBuf::from(crate::actions::expand(path));
    // The engine has no folder of its own that a relative path could start from.
    if !file.is_absolute() {
        return Err(format!(
            "\"{path}\" is not a full path, such as C:\\Users\\you\\Pictures\\star.svg"
        ));
    }
    match fs::metadata(&file) {
        Ok(meta) if !meta.is_file() => Err(format!("{} is not a file", file.display())),
        Ok(meta) if meta.len() > MAX_FILE_BYTES => Err(format!(
            "{} is bigger than {} MB",
            file.display(),
            MAX_FILE_BYTES / 1024 / 1024
        )),
        Ok(_) => Ok(file),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(format!("{} was not found", file.display()))
        }
        Err(e) => Err(format!("could not open {}: {e}", file.display())),
    }
}

/// An SVG file of the user's own, ready for Direct2D: drawn `size` pixels
/// wide, with `currentColor`, and the shapes that set no color, in `color`.
/// Its other colors stay. `None` if it has no `<svg>` element.
pub fn user_svg(source: &str, color: &str, size: f32) -> Option<String> {
    // What comes before the root element (an XML declaration, comments, a
    // DOCTYPE) is left out: Direct2D does not need it and may refuse a DOCTYPE.
    let (start, _) = source.match_indices("<svg").find(|&(i, _)| {
        source[i + 4..]
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace() || c == '>' || c == '/')
    })?;
    let body = &source[start..];
    let end = tag_end(body)?;
    let (tag, rest) = body.split_at(end);
    let (tag, closing) = match tag.strip_suffix('/') {
        Some(tag) => (tag, "/"),
        None => (tag, ""),
    };

    // Like the Lucide icons, the size goes into the file: Direct2D draws at
    // the size the file says, whatever viewport it gets. The viewBox keeps
    // the picture scaling to that size.
    let mut root = String::from("<svg");
    let (mut width, mut height) = (None, None);
    let (mut view_box, mut fill) = (false, false);
    for (name, raw) in attributes(&tag[4..]) {
        match name {
            "width" => width = length(raw),
            "height" => height = length(raw),
            _ => {
                view_box |= name == "viewBox";
                fill |= name == "fill";
                root.push(' ');
                root.push_str(name);
                if !raw.is_empty() {
                    root.push('=');
                    root.push_str(raw);
                }
            }
        }
    }
    if let (false, Some(w), Some(h)) = (view_box, width, height) {
        root.push_str(&format!(" viewBox=\"0 0 {w} {h}\""));
    }
    root.push_str(&format!(" width=\"{size}\" height=\"{size}\""));
    // Shapes without a color of their own take the root's: black in a
    // browser, the icon color here, so a one-color icon follows the theme.
    if !fill {
        root.push_str(&format!(" fill=\"{color}\""));
    }
    root.push_str(closing);
    Some(replace_ignoring_case(&(root + rest), "currentcolor", color))
}

/// Where the start tag at the beginning of `text` ends: the index of its
/// `>`, skipping any inside quoted attribute values.
fn tag_end(text: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// The attributes of a start tag, as names and values with their quotes.
fn attributes(mut text: &str) -> Vec<(&str, &str)> {
    let mut found = Vec::new();
    loop {
        text = text.trim_start();
        let name_end = text
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(text.len());
        if name_end == 0 {
            return found;
        }
        let name = &text[..name_end];
        text = text[name_end..].trim_start();
        let Some(after) = text.strip_prefix('=') else {
            found.push((name, ""));
            continue;
        };
        text = after.trim_start();
        let value_end = match text.chars().next() {
            Some(q @ ('"' | '\'')) => text[1..].find(q).map_or(text.len(), |i| i + 2),
            _ => text.find(char::is_whitespace).unwrap_or(text.len()),
        };
        found.push((name, &text[..value_end]));
        text = &text[value_end..];
    }
}

/// A `width` or `height` in plain pixels (`48`, `"48px"`); `None` for
/// percentages and other units.
fn length(raw: &str) -> Option<f64> {
    let value = raw.trim_matches(['"', '\'']).trim();
    let number = value.strip_suffix("px").unwrap_or(value).trim();
    number.parse::<f64>().ok().filter(|v| *v > 0.0)
}

fn replace_ignoring_case(text: &str, lower_pattern: &str, with: &str) -> String {
    // ASCII lower case keeps every byte where it was.
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (at, _) in lower.match_indices(lower_pattern) {
        out.push_str(&text[copied..at]);
        out.push_str(with);
        copied = at + lower_pattern.len();
    }
    out.push_str(&text[copied..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_icon_is_an_svg() {
        for (name, source) in ICONS {
            assert!(source.contains("<svg"), "{name}");
            assert!(source.contains("stroke=\"currentColor\""), "{name}");
            assert!(source.contains("stroke-width=\"2\""), "{name}");
            assert!(source.contains("width=\"24\""), "{name}");
            assert!(source.contains("height=\"24\""), "{name}");
            assert!(source.contains("viewBox=\"0 0 24 24\""), "{name}");
        }
    }

    /// The settings file accepts the names in `thumbring_core::ring::ICONS`:
    /// exactly the icons embedded here.
    #[test]
    fn the_settings_accept_exactly_the_embedded_icons() {
        let embedded: Vec<&str> = ICONS.iter().map(|(name, _)| *name).collect();
        assert_eq!(embedded, thumbring_core::ring::ICONS);
    }

    #[test]
    fn color_width_and_size_are_swapped() {
        let svg = svg("play", "#FFFFFF", 1.9, 20.8).expect("play exists");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("stroke=\"#FFFFFF\""));
        assert!(svg.contains("stroke-width=\"1.9\""));
        assert!(!svg.contains("currentColor"));
        // The drawn size follows the request; the 24-unit viewBox scales to it.
        assert!(svg.contains("width=\"20.8\""));
        assert!(svg.contains("height=\"20.8\""));
        assert!(!svg.contains("width=\"24\""));
        assert!(svg.contains("viewBox=\"0 0 24 24\""));
    }

    #[test]
    fn unknown_icons_are_none() {
        assert!(svg("no-such-icon", "#000", 2.0, 20.0).is_none());
    }

    #[test]
    fn a_user_svg_takes_the_size_and_the_icon_color() {
        let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M2 2h20" fill="currentColor" stroke="none"/></svg>"#;
        let svg = user_svg(source, "#E8E9EE", 20.8).expect("has an svg element");
        assert_eq!(
            svg,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="20.8" height="20.8" fill="#E8E9EE"><path d="M2 2h20" fill="#E8E9EE" stroke="none"/></svg>"##
        );
    }

    #[test]
    fn a_user_svg_keeps_its_own_colors() {
        let source = r##"<svg viewBox="0 0 10 10" fill="none" stroke="CurrentColor"><rect fill="#FF0000" width="5" height="5"/></svg>"##;
        let svg = user_svg(source, "#2A2B31", 20.0).unwrap();
        // The root says fill="none": no fill is added. CurrentColor is
        // replaced in any case; the red stays.
        assert!(
            svg.starts_with(
                r##"<svg viewBox="0 0 10 10" fill="none" stroke="#2A2B31" width="20" height="20">"##
            ),
            "{svg}"
        );
        assert!(
            svg.contains(r##"<rect fill="#FF0000" width="5" height="5"/>"##),
            "{svg}"
        );
    }

    #[test]
    fn a_user_svg_without_a_view_box_gets_one_from_its_size() {
        let source = "<svg width='48px' height=\"32\"><circle r='4'/></svg>";
        let svg = user_svg(source, "#000000", 20.0).unwrap();
        assert!(
            svg.starts_with(r##"<svg viewBox="0 0 48 32" width="20" height="20" fill="#000000">"##),
            "{svg}"
        );
        // A percentage cannot become a viewBox.
        let svg = user_svg("<svg width='100%' height='100%'/>", "#000000", 20.0).unwrap();
        assert_eq!(svg, r##"<svg width="20" height="20" fill="#000000"/>"##);
    }

    #[test]
    fn what_comes_before_the_root_is_left_out() {
        let source = "\u{feff}<?xml version=\"1.0\"?>\n<!-- made with <svg-editor> -->\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\">\n<svg\n  viewBox='0 0 4 4' data-note='a > b'>\n<path d='M0 0'/></svg>\n";
        let svg = user_svg(source, "#000000", 16.0).unwrap();
        assert!(
            svg.starts_with("<svg viewBox='0 0 4 4' data-note='a > b' width=\"16\""),
            "{svg}"
        );
        assert!(svg.ends_with("<path d='M0 0'/></svg>\n"), "{svg}");
    }

    #[test]
    fn a_file_without_an_svg_element_is_none() {
        assert!(user_svg("<html><body/></html>", "#000000", 20.0).is_none());
        assert!(user_svg("<svgx></svgx>", "#000000", 20.0).is_none());
        assert!(user_svg("<svg viewBox='0 0 1 1'", "#000000", 20.0).is_none());
    }

    #[test]
    fn icon_files_need_a_full_path_that_exists() {
        assert!(file("star.svg").unwrap_err().contains("is not a full path"));
        assert!(
            file(r"\icons\star.svg")
                .unwrap_err()
                .contains("is not a full path")
        );
        let missing = std::env::temp_dir().join("thumbring-no-such-icon.svg");
        let missing = missing.to_str().unwrap();
        assert!(file(missing).unwrap_err().ends_with("was not found"));
        let folder = std::env::temp_dir();
        assert!(
            file(folder.to_str().unwrap())
                .unwrap_err()
                .ends_with("is not a file")
        );
    }

    #[test]
    fn icon_files_may_use_variables_and_must_be_small() {
        // TEMP is set on every Windows session.
        let folder = format!("thumbring-icons-{}", std::process::id());
        let dir = PathBuf::from(std::env::var("TEMP").unwrap()).join(&folder);
        fs::create_dir_all(&dir).unwrap();
        let small = dir.join("small.svg");
        fs::write(&small, "<svg/>").unwrap();
        let big = dir.join("big.png");
        fs::write(&big, vec![0u8; MAX_FILE_BYTES as usize + 1]).unwrap();

        let with_variable = format!("%TEMP%\\{folder}\\small.svg");
        assert_eq!(file(&with_variable).unwrap(), small);
        assert!(
            file(big.to_str().unwrap())
                .unwrap_err()
                .ends_with("is bigger than 2 MB")
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
