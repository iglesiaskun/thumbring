//! The `[appearance]` section and icon files: defaults (the ring as it always
//! looked), lenient reading (an invalid value falls back to its default with a
//! warning) and the ring size limits.

use thumbring_core::color::Color;
use thumbring_core::config::{Appearance, Config, LabelStyle, Loaded};
use thumbring_core::ring::{Action, IconFormat, IconSource, Slot};

fn load(text: &str) -> (Config, Vec<String>) {
    let loaded = Config::from_toml(text).expect("the test TOML is valid");
    let keys = loaded.warnings.iter().map(|w| w.key.clone()).collect();
    (loaded.config, keys)
}

fn appearance(body: &str) -> (Appearance, Vec<String>) {
    let (config, warnings) = load(&format!("[appearance]\n{body}"));
    (config.appearance, warnings)
}

fn color(text: &str) -> Color {
    text.parse().expect("the test color is valid")
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

#[test]
fn without_the_section_the_ring_looks_as_it_always_did() {
    let (config, warnings) = load("version = 1\n[gestures]\nup = \"F5\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    let a = config.appearance;
    assert_eq!(a, Appearance::default());
    assert_eq!(a.item_color, color("#FFFFFF"));
    assert_eq!(a.item_opacity, 0.96);
    assert_eq!(a.icon_color, color("#2A2B31"));
    assert_eq!(a.highlight_color, color("#121214"));
    assert_eq!(a.highlight_icon_color, color("#FFFFFF"));
    assert_eq!(a.cancel_color, color("#E64054"));
    assert_eq!(a.item_size_px, 52.0);
    assert_eq!(a.ring_radius_px, 93.6);
    assert!(a.shadow);
    assert_eq!(a.label, LabelStyle::Light);
}

#[test]
fn the_default_file_writes_out_the_default_appearance() {
    let (config, warnings) = load(thumbring_core::config::DEFAULT_FILE);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.appearance, Appearance::default());
    let table: toml::Table = thumbring_core::config::DEFAULT_FILE.parse().unwrap();
    assert!(table.contains_key("appearance"));
}

/// The looks suggested in the settings file and in the settings app.
#[test]
fn every_suggested_look_loads_without_warnings() {
    let looks = [
        r##"item_color = "#1F2026"
            item_opacity = 0.94
            icon_color = "#E8E9EE"
            highlight_color = "#FFFFFF"
            highlight_icon_color = "#16171B"
            cancel_color = "#FF5A6E"
            label = "dark""##,
        r##"item_opacity = 0.55
            icon_color = "#16171B"
            shadow = false"##,
        r##"highlight_color = "#2F5BD3""##,
    ];
    for body in looks {
        let (_, warnings) = appearance(body);
        assert!(warnings.is_empty(), "{warnings:?}\n{body}");
    }
    let (dark, _) = appearance(looks[0]);
    assert_eq!(dark.item_color, Color::rgb(0x1F, 0x20, 0x26));
    assert_eq!(dark.label, LabelStyle::Dark);
    let (glass, _) = appearance(looks[1]);
    assert_eq!(glass.item_opacity, 0.55);
    assert!(!glass.shadow);
    // What a look does not set keeps its default.
    assert_eq!(glass.highlight_color, Appearance::default().highlight_color);
}

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

#[test]
fn colors_take_hex_digits_in_either_case() {
    let (a, warnings) = appearance(r##"item_color = "#1f2026""##);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(a.item_color, Color::rgb(0x1F, 0x20, 0x26));
    assert_eq!(a.item_color.to_string(), "#1F2026");
}

#[test]
fn an_invalid_color_falls_back_to_its_default() {
    let loaded = Config::from_toml(
        r##"
        [appearance]
        item_color = "white"
        icon_color = "#FFF"
        highlight_color = "2F5BD3"
        highlight_icon_color = 0xFFFFFF
        cancel_color = "#E6405"
        "##,
    )
    .expect("valid TOML");
    let keys: Vec<_> = loaded.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "appearance.item_color",
            "appearance.icon_color",
            "appearance.highlight_color",
            "appearance.highlight_icon_color",
            "appearance.cancel_color",
        ]
    );
    assert_eq!(loaded.config.appearance, Appearance::default());
    assert_eq!(
        loaded.warnings[0].to_string(),
        "appearance.item_color: \"white\": a color is written as \"#RRGGBB\", such as \
         \"#2F5BD3\"; using the default #FFFFFF"
    );
    assert!(
        loaded.warnings[3]
            .message
            .contains("expected a color as text")
    );
}

// ---------------------------------------------------------------------------
// Numbers, switches and unknown keys
// ---------------------------------------------------------------------------

#[test]
fn range_limits_are_inclusive() {
    for body in [
        "item_opacity = 0.3\nitem_size_px = 32",
        "item_opacity = 1\nitem_size_px = 96",
    ] {
        let (_, warnings) = appearance(body);
        assert!(warnings.is_empty(), "{warnings:?}\n{body}");
    }
    let (a, _) = appearance("item_opacity = 1\nitem_size_px = 96");
    assert_eq!((a.item_opacity, a.item_size_px), (1.0, 96.0));
}

#[test]
fn out_of_range_values_fall_back_to_the_default() {
    let (a, warnings) = appearance("item_opacity = 0.1\nitem_size_px = 20");
    assert_eq!(
        warnings,
        ["appearance.item_opacity", "appearance.item_size_px"]
    );
    assert_eq!(a, Appearance::default());

    let (_, warnings) = appearance("item_opacity = nan\nitem_size_px = -1\nring_radius_px = inf");
    assert_eq!(warnings.len(), 3, "{warnings:?}");
}

#[test]
fn wrong_types_fall_back_to_the_default() {
    let (a, warnings) = appearance(
        r#"item_opacity = "high"
           item_size_px = true
           shadow = "yes"
           label = "grey""#,
    );
    assert_eq!(
        warnings,
        [
            "appearance.item_opacity",
            "appearance.item_size_px",
            "appearance.shadow",
            "appearance.label"
        ]
    );
    assert_eq!(a, Appearance::default());
}

#[test]
fn shadow_and_label_are_read() {
    let (a, warnings) = appearance("shadow = false\nlabel = \"dark\"");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(!a.shadow);
    assert_eq!(a.label, LabelStyle::Dark);
}

#[test]
fn unknown_keys_and_a_section_with_the_wrong_shape_are_reported() {
    let (_, warnings) = appearance("item_colour = \"#000000\"");
    assert_eq!(warnings, ["appearance.item_colour"]);
    let (config, warnings) = load("appearance = \"dark\"");
    assert_eq!(warnings, ["appearance"]);
    assert_eq!(config.appearance, Appearance::default());
}

// ---------------------------------------------------------------------------
// Item size and ring radius
// ---------------------------------------------------------------------------

#[test]
fn without_a_ring_radius_it_follows_the_item_size() {
    let (a, warnings) = appearance("item_size_px = 60");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(a.ring_radius_px, 108.0);
    let (a, _) = appearance("item_size_px = 41");
    assert_eq!(a.ring_radius_px, 73.8);
}

#[test]
fn the_ring_radius_goes_from_1_5_to_3_item_sizes() {
    let (a, warnings) = appearance("item_size_px = 40\nring_radius_px = 60");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(a.ring_radius_px, 60.0);
    let (a, warnings) = appearance("item_size_px = 40\nring_radius_px = 120");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(a.ring_radius_px, 120.0);
    // The default item size: 78 to 156.
    let (a, warnings) = appearance("ring_radius_px = 156");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(a.ring_radius_px, 156.0);
}

#[test]
fn a_ring_radius_where_the_items_would_touch_falls_back_to_the_default() {
    let loaded = Config::from_toml("[appearance]\nitem_size_px = 60\nring_radius_px = 80\n")
        .expect("valid TOML");
    let keys: Vec<_> = loaded.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(keys, ["appearance.ring_radius_px"]);
    // The default that goes with the item size, not the 93.6 of 52 px items.
    assert_eq!(loaded.config.appearance.item_size_px, 60.0);
    assert_eq!(loaded.config.appearance.ring_radius_px, 108.0);
    assert_eq!(
        loaded.warnings[0].to_string(),
        "appearance.ring_radius_px: 80 is outside the range from 90 to 180 (1.5 to 3 times \
         item_size_px, so the items never touch); using the default 108"
    );

    let (a, warnings) = appearance("item_size_px = 40\nring_radius_px = 121");
    assert_eq!(warnings, ["appearance.ring_radius_px"]);
    assert_eq!(a.ring_radius_px, 72.0);
}

#[test]
fn an_invalid_item_size_takes_the_ring_radius_limits_of_the_default_one() {
    // 20 px goes back to 52 px; 70 is below 1.5 × 52 = 78.
    let (a, warnings) = appearance("item_size_px = 20\nring_radius_px = 70");
    assert_eq!(
        warnings,
        ["appearance.item_size_px", "appearance.ring_radius_px"]
    );
    assert_eq!((a.item_size_px, a.ring_radius_px), (52.0, 93.6));
}

#[test]
fn the_ring_radius_limits_read_well() {
    assert_eq!(Appearance::ring_radius_px_range(52.0), 78.0..=156.0);
    assert_eq!(Appearance::ring_radius_px_range(52.3), 78.5..=156.9);
    assert_eq!(Appearance::default_ring_radius_px(52.3), 94.1);
}

// ---------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------

#[test]
fn a_written_appearance_reads_back_identical() {
    let config = Config {
        appearance: Appearance {
            item_color: Color::rgb(0x1F, 0x20, 0x26),
            item_opacity: 0.5,
            icon_color: Color::rgb(0xE8, 0xE9, 0xEE),
            highlight_color: Color::WHITE,
            highlight_icon_color: Color::rgb(0x16, 0x17, 0x1B),
            cancel_color: Color::rgb(0xFF, 0x5A, 0x6E),
            item_size_px: 44.0,
            ring_radius_px: 110.0,
            shadow: false,
            label: LabelStyle::Dark,
        },
        ..Config::default()
    };
    let text = config.to_toml().expect("serializes");
    assert!(text.contains("item_color = \"#1F2026\""), "{text}");
    let loaded = Config::from_toml(&text).expect("reads back");
    assert!(loaded.warnings.is_empty(), "{:?}\n{text}", loaded.warnings);
    assert_eq!(loaded.config, config, "\n{text}");
}

// ---------------------------------------------------------------------------
// Icon files
// ---------------------------------------------------------------------------

#[test]
fn an_icon_is_a_built_in_name_or_an_svg_or_png_file() {
    assert_eq!(IconSource::of("play"), Some(IconSource::Builtin("play")));
    let svg = r"C:\Users\you\Pictures\star.svg";
    assert_eq!(
        IconSource::of(svg),
        Some(IconSource::File(svg, IconFormat::Svg))
    );
    for png in [r"%USERPROFILE%\icons\Mail.PNG", "D:/icons/a.png", "a.png"] {
        assert_eq!(
            IconSource::of(png),
            Some(IconSource::File(png, IconFormat::Png)),
            "{png}"
        );
    }
    for other in [
        "rocket",
        "",
        ".svg",
        r"C:\icons\.png",
        "icons/.svg",
        "star.jpg",
        "star.svg.txt",
        "svg",
    ] {
        assert_eq!(IconSource::of(other), None, "{other:?}");
    }
}

#[test]
fn icon_files_are_accepted_when_the_settings_are_read() {
    let (config, warnings) = load(
        r#"
        [[ring.slots]]
        type = "action"
        name = "Star"
        icon = 'C:\Users\you\Pictures\star.svg'

        [[ring.slots]]
        type = "folder"
        name = "Mail"
        icon = '%USERPROFILE%\icons\mail.png'
        items = [{ name = "Inbox", icon = "C:/icons/inbox.SVG" }, { name = "Photo", icon = "photo.jpg" }]
        "#,
    );
    assert_eq!(warnings, ["ring.slots[1].items[1].icon"]);
    let Slot::Action(star) = &config.ring.slots[0] else {
        panic!("expected an action")
    };
    assert_eq!(
        star.icon.as_deref(),
        Some(r"C:\Users\you\Pictures\star.svg")
    );
    let Slot::Folder { icon, items, .. } = &config.ring.slots[1] else {
        panic!("expected a folder")
    };
    assert_eq!(icon.as_deref(), Some(r"%USERPROFILE%\icons\mail.png"));
    assert_eq!(items[0].icon.as_deref(), Some("C:/icons/inbox.SVG"));
    assert_eq!(items[1].icon, None);
}

/// A ring with an icon file in a slot and in a folder, and built-in icons.
fn ring_with_files() -> Loaded {
    Config::from_toml(
        r#"
        [[ring.slots]]
        type = "action"
        name = "Star"
        icon = 'C:\missing\star.svg'

        [[ring.slots]]
        type = "action"
        name = "Play"
        icon = "play"

        [[ring.slots]]
        type = "folder"
        name = "Pictures"
        icon = 'C:\ok\folder.png'
        items = [{ name = "Copy", icon = "copy" }, { name = "Moon", icon = 'C:\missing\moon.png' }]
        "#,
    )
    .expect("valid TOML")
}

#[test]
fn icon_files_that_cannot_be_used_are_dropped_with_a_warning() {
    let mut loaded = ring_with_files();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let mut checked = Vec::new();
    loaded.check_icon_files(|path, format| {
        checked.push((path.to_owned(), format));
        if path.starts_with(r"C:\missing") {
            Err(format!("{path} was not found"))
        } else {
            Ok(())
        }
    });
    // Built-in icons are never checked.
    assert_eq!(
        checked,
        [
            (r"C:\missing\star.svg".to_owned(), IconFormat::Svg),
            (r"C:\ok\folder.png".to_owned(), IconFormat::Png),
            (r"C:\missing\moon.png".to_owned(), IconFormat::Png),
        ]
    );
    let text: Vec<String> = loaded.warnings.iter().map(ToString::to_string).collect();
    assert_eq!(
        text,
        [
            r"ring.slots[0].icon: C:\missing\star.svg was not found; the item shows the first letter of its name",
            r"ring.slots[2].items[1].icon: C:\missing\moon.png was not found; the item shows the first letter of its name",
        ]
    );
    let slots = &loaded.config.ring.slots;
    assert_eq!(
        slots[0],
        Slot::Action(Action {
            name: "Star".into(),
            icon: None,
            command: None
        })
    );
    let Slot::Folder { icon, items, .. } = &slots[2] else {
        panic!("expected a folder")
    };
    assert_eq!(icon.as_deref(), Some(r"C:\ok\folder.png"));
    assert_eq!(items[0].icon.as_deref(), Some("copy"));
    assert_eq!(items[1].icon, None);
}

#[test]
fn icon_files_that_open_stay() {
    let mut loaded = ring_with_files();
    let before = loaded.config.clone();
    loaded.check_icon_files(|_, _| Ok(()));
    assert!(loaded.warnings.is_empty());
    assert_eq!(loaded.config, before);
}
