//! Lucide icons (ISC license, see `assets/icons/LICENSE-LUCIDE`), embedded in
//! the executable.
//!
//! The SVG files are used as they come from Lucide; [`svg`] only swaps the
//! stroke color and width, which Direct2D cannot take from the context.

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
}
