//! The icons a ring item can use, for the interface to draw them as the
//! engine does: the same Lucide files the engine embeds, from
//! `engine/assets/icons`. A test keeps this list equal to the engine's.

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

#[cfg(test)]
mod tests {
    use super::RING_ICONS;

    #[test]
    fn the_same_icons_as_the_engine() {
        let names: Vec<&str> = RING_ICONS.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, thumbring_core::ring::ICONS);
        for (name, svg) in RING_ICONS {
            assert!(svg.contains("<svg"), "{name}");
        }
    }
}
