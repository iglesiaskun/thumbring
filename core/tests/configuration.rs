//! Configuration: defaults, range validation (an invalid value falls
//! back to its default with a warning, no panic) and ring rules (8 slots,
//! folders with up to 9 actions, no folder inside a folder).

use std::time::Duration;

use thumbring_core::apps::AppName;
use thumbring_core::command::Command;
use thumbring_core::config::{
    Animation, Behavior, BlobStyle, Config, ConfigError, DEFAULT_FILE, FORMAT_VERSION,
    GestureOverrides, OpenWith, Profile,
};
use thumbring_core::ring::{Action, FOLDER_MAX_ITEMS, ICONS, Ring, SLOT_COUNT, Slot};

/// Loads valid TOML and returns the configuration and the keys with warnings.
fn load(text: &str) -> (Config, Vec<String>) {
    let loaded = Config::from_toml(text).expect("the test TOML is valid");
    let keys = loaded.warnings.iter().map(|w| w.key.clone()).collect();
    (loaded.config, keys)
}

fn cmd(text: &str) -> Command {
    text.parse().expect("the test command is valid")
}

fn app(name: &str) -> AppName {
    AppName::new(name).expect("the test name is valid")
}

fn action(name: &str) -> Action {
    Action {
        name: name.into(),
        icon: None,
        command: None,
    }
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

#[test]
fn empty_file_gives_the_defaults() {
    let (config, warnings) = load("");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config, Config::default());

    let a = &config.animation;
    assert_eq!(a.style, BlobStyle::Viscous);
    assert_eq!(a.speed, 1.0);
    assert_eq!(a.ring_open_ms, 150);
    assert_eq!(a.ring_close_ms, 170);
    assert_eq!(a.blob_ms_per_item, 135);
    assert_eq!(a.viscosity, 0.85);
    assert_eq!(a.break_at, 0.95);
    assert_eq!(a.wobble, 0.0);
    assert_eq!(a.folder_items_ms, 170);
    assert_eq!(a.folder_close_ms, 100);
    assert!(!a.ignore_system_reduced_motion);

    let b = &config.behavior;
    assert_eq!(b.open_with, OpenWith::Hold);
    assert!(!b.execute_on_release);
    assert_eq!(b.hold_to_open_ms, 150);
    assert_eq!(b.path_tolerance_ms, 50);

    assert_eq!(config.ring, Ring::default());
    assert!(!config.ring.is_empty());
}

/// The default ring: its slots in order, with a command on every item.
#[test]
fn the_default_ring_has_its_slots_in_order_and_commands() {
    let ring = Ring::default();
    let names: Vec<&str> = ring
        .slots
        .iter()
        .map(|slot| match slot {
            Slot::Action(a) => a.name.as_str(),
            Slot::Folder { name, .. } => name.as_str(),
            Slot::Empty => "",
        })
        .collect();
    assert_eq!(
        names,
        [
            "Play or pause",
            "Screenshot",
            "Media",
            "Lock screen",
            "Editing",
            "Emoji",
            "File Explorer",
            "Apps",
        ]
    );
    for slot in &ring.slots {
        let (icon, actions) = match slot {
            Slot::Action(a) => (a.icon.as_deref(), std::slice::from_ref(a)),
            Slot::Folder { icon, items, .. } => (icon.as_deref(), items.as_slice()),
            Slot::Empty => unreachable!("the default ring has no empty slot"),
        };
        assert!(icon.is_some_and(|i| ICONS.contains(&i)), "{slot:?}");
        for action in actions {
            assert!(action.command.is_some(), "{} has no command", action.name);
            let icon = action.icon.as_deref();
            assert!(icon.is_some_and(|i| ICONS.contains(&i)), "{action:?}");
        }
    }
}

/// Every animation and behavior timing written out with its default value.
#[test]
fn writing_out_every_timing_gives_the_defaults() {
    let doc = r#"
[animation]
style = "viscous"          # or "elastic"
speed = 1.0                # 0.5 to 2.0
ring_open_ms = 150
ring_close_ms = 170
blob_ms_per_item = 135
viscosity = 0.85           # 0.2 to 2.0
break_at = 0.95            # fraction of the trip
wobble = 0.0               # 0 to 1
folder_items_ms = 170
folder_close_ms = 100

[behavior]
hold_to_open_ms = 150
path_tolerance_ms = 50
"#;
    let (config, warnings) = load(doc);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config, Config::default());
}

#[test]
fn elastic_style_brings_its_own_blob_shape() {
    let (config, warnings) = load("[animation]\nstyle = \"elastic\"");
    assert!(warnings.is_empty());
    let a = config.animation;
    assert_eq!(a.style, BlobStyle::Elastic);
    assert_eq!((a.viscosity, a.break_at, a.wobble), (1.2, 0.78, 0.4));
    // The rest stays the same.
    assert_eq!(a.ring_open_ms, 150);
}

#[test]
fn explicit_values_override_the_style_preset() {
    let (config, warnings) = load("[animation]\nstyle = \"elastic\"\nwobble = 0.1\nviscosity = 1");
    assert!(warnings.is_empty());
    assert_eq!(config.animation.wobble, 0.1);
    // An integer where a decimal number is expected is accepted.
    assert_eq!(config.animation.viscosity, 1.0);
}

// ---------------------------------------------------------------------------
// Validation: an invalid value falls back to its default with a warning
// ---------------------------------------------------------------------------

#[test]
fn out_of_range_values_fall_back_to_the_default() {
    let (config, warnings) = load(
        "[animation]
         speed = 5.0
         ring_open_ms = 5000
         ring_close_ms = 200
         wobble = -0.1
         break_at = 0.3",
    );
    assert_eq!(
        warnings,
        [
            "animation.speed",
            "animation.ring_open_ms",
            "animation.break_at",
            "animation.wobble"
        ]
    );
    let a = config.animation;
    assert_eq!(a.speed, 1.0);
    assert_eq!(a.ring_open_ms, 150);
    assert_eq!(a.wobble, 0.0);
    assert_eq!(a.break_at, 0.95);
    // The valid value next to it still applies.
    assert_eq!(a.ring_close_ms, 200);
}

#[test]
fn range_limits_are_inclusive() {
    let (config, warnings) = load(
        "[animation]
         speed = 0.5
         ring_open_ms = 0
         ring_close_ms = 1000
         viscosity = 2.0
         break_at = 0.5
         wobble = 1.0
         [behavior]
         hold_to_open_ms = 60
         path_tolerance_ms = 1000",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.animation.speed, 0.5);
    assert_eq!(config.animation.ring_open_ms, 0);
    assert_eq!(config.behavior.hold_to_open_ms, 60);
}

#[test]
fn warning_messages_explain_what_happened() {
    let loaded = Config::from_toml("[animation]\nspeed = 5.0\nring_open_ms = -20").unwrap();
    let text: Vec<String> = loaded.warnings.iter().map(ToString::to_string).collect();
    assert_eq!(
        text,
        [
            "animation.speed: 5 is outside the range from 0.5 to 2; using the default 1",
            "animation.ring_open_ms: -20 is outside the range from 0 to 1000; using the default 150",
        ]
    );
}

#[test]
fn wrong_types_fall_back_to_the_default() {
    let (config, warnings) = load(
        "[animation]
         style = 3
         speed = \"fast\"
         ring_open_ms = 150.5
         [behavior]
         open_with = \"always\"
         execute_on_release = \"yes\"",
    );
    assert_eq!(
        warnings,
        [
            "animation.style",
            "animation.speed",
            "animation.ring_open_ms",
            "behavior.open_with",
            "behavior.execute_on_release",
        ]
    );
    assert_eq!(config, Config::default());
}

#[test]
fn nan_and_infinity_are_out_of_range() {
    let (config, warnings) = load("[animation]\nviscosity = nan\nspeed = inf");
    assert_eq!(warnings, ["animation.speed", "animation.viscosity"]);
    assert_eq!(config.animation.viscosity, 0.85);
    assert_eq!(config.animation.speed, 1.0);
}

#[test]
fn invalid_shape_value_falls_back_to_the_style_preset() {
    let (config, warnings) = load("[animation]\nstyle = \"elastic\"\nviscosity = 9");
    assert_eq!(warnings, ["animation.viscosity"]);
    assert_eq!(config.animation.viscosity, 1.2);
}

#[test]
fn unknown_style_falls_back_to_viscous() {
    let (config, warnings) = load("[animation]\nstyle = \"jelly\"");
    assert_eq!(warnings, ["animation.style"]);
    assert_eq!(config.animation.style, BlobStyle::Viscous);
}

#[test]
fn behavior_values_are_read_and_validated() {
    let (config, warnings) = load(
        "[behavior]
         open_with = \"tap\"
         execute_on_release = true
         hold_to_open_ms = 300
         path_tolerance_ms = 0",
    );
    assert!(warnings.is_empty());
    assert_eq!(
        config.behavior,
        Behavior {
            open_with: OpenWith::Tap,
            execute_on_release: true,
            hold_to_open_ms: 300,
            path_tolerance_ms: 0
        }
    );

    let (config, warnings) = load("[behavior]\nhold_to_open_ms = 20\npath_tolerance_ms = 5000");
    assert_eq!(
        warnings,
        ["behavior.hold_to_open_ms", "behavior.path_tolerance_ms"]
    );
    assert_eq!(config.behavior, Behavior::default());
}

#[test]
fn unknown_keys_and_sections_are_reported() {
    let (config, warnings) = load(
        "[animation]
         ring_open_msx = 300
         speed = 1.5
         [colors]
         accent = \"red\"",
    );
    assert_eq!(warnings, ["colors", "animation.ring_open_msx"]);
    assert_eq!(config.animation.speed, 1.5);
}

#[test]
fn section_with_the_wrong_shape_uses_defaults() {
    let (config, warnings) = load("animation = 5\nbehavior = [1, 2]");
    assert_eq!(warnings, ["animation", "behavior"]);
    assert_eq!(config, Config::default());
}

#[test]
fn invalid_toml_is_an_error_not_a_panic() {
    let err = Config::from_toml("[animation\nspeed = ").unwrap_err();
    assert!(matches!(err, ConfigError::Syntax(_)));
    assert!(
        err.to_string()
            .starts_with("the settings file is not valid TOML")
    );
}

#[test]
fn a_messy_file_still_loads() {
    let messy = r#"
        surprise = true
        [animation]
        style = "?"
        speed = -1
        ring_open_ms = "slow"
        viscosity = 0.5
        [behavior]
        open_with = 1
        [ring]
        slots = [1, { type = "action" }, { type = "folder", name = "F", items = [{ name = "ok" }, 2] }]
    "#;
    let loaded = Config::from_toml(messy).unwrap();
    assert!(loaded.warnings.len() >= 7, "{:#?}", loaded.warnings);
    let config = loaded.config;
    assert_eq!(config.animation.viscosity, 0.5);
    assert_eq!(config.animation.speed, 1.0);
    assert_eq!(config.ring.slots[0], Slot::Empty);
    assert_eq!(config.ring.slots[1], Slot::Empty);
    assert_eq!(
        config.ring.slots[2],
        Slot::Folder {
            name: "F".into(),
            icon: None,
            items: vec![action("ok")]
        }
    );
}

// ---------------------------------------------------------------------------
// Overall speed
// ---------------------------------------------------------------------------

#[test]
fn speed_scales_animations_but_not_logic_times() {
    let (config, _) =
        load("[animation]\nspeed = 2.0\nring_open_ms = 150\n[behavior]\nhold_to_open_ms = 150");
    assert_eq!(
        config.animation.scaled(config.animation.ring_open_ms),
        Duration::from_millis(75)
    );
    // The logic timings live in another section and do not go through `scaled`.
    assert_eq!(config.behavior.hold_to_open_ms, 150);

    let slow = Animation {
        speed: 0.5,
        ..Animation::default()
    };
    assert_eq!(slow.scaled(150), Duration::from_millis(300));
}

// ---------------------------------------------------------------------------
// Ring
// ---------------------------------------------------------------------------

#[test]
fn ring_slots_are_read_in_order_and_missing_ones_stay_empty() {
    let (config, warnings) = load(
        r#"
        [[ring.slots]]
        type = "action"
        name = "Screenshot"
        icon = "scan"
        command = "Win+Shift+S"

        [[ring.slots]]
        type = "empty"

        [[ring.slots]]
        type = "folder"
        name = "Media"
        icon = "music"

        [[ring.slots.items]]
        name = "Previous track"

        [[ring.slots.items]]
        type = "action"
        name = "Next track"
        icon = "skip-forward"
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(!config.ring.is_empty());
    let slots = &config.ring.slots;
    assert_eq!(
        slots[0],
        Slot::Action(Action {
            name: "Screenshot".into(),
            icon: Some("scan".into()),
            command: Some(cmd("Win+Shift+S")),
        })
    );
    assert_eq!(slots[1], Slot::Empty);
    assert_eq!(
        slots[2],
        Slot::Folder {
            name: "Media".into(),
            icon: Some("music".into()),
            items: vec![
                action("Previous track"),
                Action {
                    icon: Some("skip-forward".into()),
                    ..action("Next track")
                },
            ],
        }
    );
    assert!(slots[3..].iter().all(|s| *s == Slot::Empty));
}

#[test]
fn folder_keeps_at_most_nine_actions() {
    let items: Vec<String> = (1..=11).map(|i| format!("{{ name = \"a{i}\" }}")).collect();
    let text = format!(
        "[[ring.slots]]\ntype = \"folder\"\nname = \"F\"\nitems = [{}]",
        items.join(", ")
    );
    let (config, warnings) = load(&text);
    assert_eq!(warnings, ["ring.slots[0].items"]);
    let Slot::Folder { items, .. } = &config.ring.slots[0] else {
        panic!("expected a folder")
    };
    assert_eq!(items.len(), FOLDER_MAX_ITEMS);
    assert_eq!(items[8].name, "a9");
}

#[test]
fn folder_with_exactly_nine_actions_is_fine() {
    let items: Vec<String> = (1..=9).map(|i| format!("{{ name = \"a{i}\" }}")).collect();
    let text = format!(
        "[[ring.slots]]\ntype = \"folder\"\nname = \"F\"\nitems = [{}]",
        items.join(", ")
    );
    let (_, warnings) = load(&text);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn folder_inside_a_folder_is_rejected() {
    let (config, warnings) = load(
        r#"
        [[ring.slots]]
        type = "folder"
        name = "Outer"

        [[ring.slots.items]]
        name = "Copy"

        [[ring.slots.items]]
        type = "folder"
        name = "Inner"

        [[ring.slots.items]]
        name = "Also inner"
        items = [{ name = "grandchild" }]

        [[ring.slots.items]]
        type = "empty"

        [[ring.slots.items]]
        name = "Paste"
        "#,
    );
    assert_eq!(
        warnings,
        [
            "ring.slots[0].items[1]",
            "ring.slots[0].items[2]",
            "ring.slots[0].items[3]"
        ]
    );
    assert_eq!(
        config.ring.slots[0],
        Slot::Folder {
            name: "Outer".into(),
            icon: None,
            items: vec![action("Copy"), action("Paste")]
        }
    );
}

#[test]
fn ring_keeps_at_most_eight_slots() {
    let slots: Vec<String> = (1..=10)
        .map(|i| format!("{{ type = \"action\", name = \"s{i}\" }}"))
        .collect();
    let (config, warnings) = load(&format!("[ring]\nslots = [{}]", slots.join(", ")));
    assert_eq!(warnings, ["ring.slots"]);
    assert_eq!(config.ring.slots.len(), SLOT_COUNT);
    assert_eq!(config.ring.slots[7], Slot::Action(action("s8")));
}

#[test]
fn without_slots_the_ring_is_the_default_one() {
    let (config, warnings) = load("[ring]\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.ring, Ring::default());

    let (config, _) = load("[gestures]\nup = \"F5\"\n");
    assert_eq!(config.ring, Ring::default());
}

#[test]
fn an_empty_list_of_slots_turns_the_ring_off() {
    let (config, warnings) = load("[ring]\nslots = []\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.ring, Ring::empty());
    assert!(config.ring.is_empty());
}

#[test]
fn slots_that_are_not_a_list_give_the_default_ring() {
    let (config, warnings) = load("[ring]\nslots = \"none\"\n");
    assert_eq!(warnings, ["ring.slots"]);
    assert_eq!(config.ring, Ring::default());
}

#[test]
fn an_unknown_icon_is_dropped_with_a_warning() {
    let (config, warnings) = load(
        r#"
        [[ring.slots]]
        type = "action"
        name = "Rocket"
        icon = "rocket"

        [[ring.slots]]
        type = "folder"
        name = "Stuff"
        icon = "box"
        items = [{ name = "Thing", icon = "gizmo" }, { name = "Copy", icon = "copy" }]
        "#,
    );
    assert_eq!(
        warnings,
        [
            "ring.slots[0].icon",
            "ring.slots[1].icon",
            "ring.slots[1].items[0].icon",
        ]
    );
    assert_eq!(config.ring.slots[0], Slot::Action(action("Rocket")));
    assert_eq!(
        config.ring.slots[1],
        Slot::Folder {
            name: "Stuff".into(),
            icon: None,
            items: vec![
                action("Thing"),
                Action {
                    icon: Some("copy".into()),
                    ..action("Copy")
                },
            ],
        }
    );
}

#[test]
fn invalid_slots_become_empty_with_a_warning() {
    let (config, warnings) = load(
        r#"
        [[ring.slots]]
        name = "no type"

        [[ring.slots]]
        type = "action"

        [[ring.slots]]
        type = "macro"
        name = "x"

        [[ring.slots]]
        type = "folder"

        [[ring.slots]]
        type = "action"
        name = "ok"
        color = "blue"
        "#,
    );
    assert_eq!(
        warnings,
        [
            "ring.slots[0].type",
            "ring.slots[1].name",
            "ring.slots[2].type",
            "ring.slots[3].name",
            "ring.slots[4].color",
        ]
    );
    assert!(config.ring.slots[..4].iter().all(|s| *s == Slot::Empty));
    assert_eq!(config.ring.slots[4], Slot::Action(action("ok")));
}

// ---------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------

#[test]
fn written_config_reads_back_identical() {
    let mut config = Config {
        animation: Animation {
            speed: 1.5,
            ring_open_ms: 200,
            ..Animation::for_style(BlobStyle::Elastic)
        },
        behavior: Behavior {
            open_with: OpenWith::Tap,
            path_tolerance_ms: 80,
            ..Behavior::default()
        },
        ..Config::default()
    };
    config.ring.slots[0] = Slot::Action(Action {
        icon: Some("lock".into()),
        ..action("Lock screen")
    });
    config.ring.slots[2] = Slot::Folder {
        name: "Editing".into(),
        icon: Some("clipboard".into()),
        items: vec![
            Action {
                command: Some(cmd("Ctrl+C")),
                ..action("Copy")
            },
            Action {
                command: Some(cmd("Ctrl+V")),
                ..action("Paste")
            },
        ],
    };

    let text = config.to_toml().expect("serializes");
    let loaded = Config::from_toml(&text).expect("reads back");
    assert!(loaded.warnings.is_empty(), "{:?}\n{text}", loaded.warnings);
    assert_eq!(loaded.config, config, "\n{text}");
}

// ---------------------------------------------------------------------------
// Default file
// ---------------------------------------------------------------------------

/// The file created on the first run has comments, but it must give exactly
/// the defaults, without any warning.
#[test]
fn the_default_file_gives_the_defaults_without_warnings() {
    let (config, warnings) = load(DEFAULT_FILE);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config, Config::default());
}

#[test]
fn default_gestures_are_the_ones_agreed() {
    let g = Config::default().gestures;
    assert_eq!(g.up, cmd("Win+Tab"));
    assert_eq!(g.down, cmd("Win+D"));
    // Dragging right brings the previous desktop into view, as if pushing
    // the screen aside.
    assert_eq!(g.left, cmd("Ctrl+Win+Right"));
    assert_eq!(g.right, cmd("Ctrl+Win+Left"));
    assert_eq!(g.click, Command::Native);
    assert_eq!(g.threshold_px, 30);

    let apps = Config::default().apps;
    assert!(apps.disable_in_fullscreen);
    assert!(apps.excluded.is_empty());
    assert!(Config::default().profiles.is_empty());
}

#[test]
fn a_utf8_bom_is_ignored() {
    let (config, warnings) = load("\u{feff}[gestures]\nup = \"F5\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.gestures.up, cmd("F5"));
}

// ---------------------------------------------------------------------------
// Format version
// ---------------------------------------------------------------------------

#[test]
fn the_default_file_declares_the_current_version() {
    let table: toml::Table = DEFAULT_FILE.parse().expect("valid TOML");
    assert_eq!(
        table.get("version").and_then(toml::Value::as_integer),
        Some(i64::from(FORMAT_VERSION))
    );
}

#[test]
fn a_file_without_version_is_read_as_version_1() {
    // Files written before the field existed.
    let (config, warnings) = load(
        r#"
        [gestures]
        up = "Win+D"
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.gestures.up, cmd("Win+D"));

    let (_, warnings) = load(
        r#"
        version = 1
        [gestures]
        up = "Win+D"
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_file_for_a_newer_version_warns_and_still_loads() {
    let text = r#"
        version = 2
        [gestures]
        up = "Win+D"
    "#;
    let loaded = Config::from_toml(text).expect("valid TOML");
    let keys: Vec<_> = loaded.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(keys, ["version"]);
    assert!(loaded.warnings[0].message.contains("newer"));
    assert_eq!(loaded.config.gestures.up, cmd("Win+D"));
}

#[test]
fn an_invalid_version_warns_and_counts_as_the_current_one() {
    for text in [
        "version = 0",
        "version = -1",
        r#"version = "1""#,
        "version = 1.5",
    ] {
        let (config, warnings) = load(text);
        assert_eq!(warnings, ["version"], "{text}");
        assert_eq!(config, Config::default(), "{text}");
    }
}

#[test]
fn a_written_file_starts_with_the_version() {
    let text = Config::default().to_toml().expect("serializes");
    assert!(text.starts_with("version = 1\n"), "{text}");
}

// ---------------------------------------------------------------------------
// Gestures
// ---------------------------------------------------------------------------

#[test]
fn gestures_are_read() {
    let (config, warnings) = load(
        r#"
        [gestures]
        up = "VolumeUp"
        down = "VolumeDown"
        left = 'open:C:\Users\you'
        right = "lock"
        click = "Alt+Left"
        threshold_px = 45
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let g = &config.gestures;
    assert_eq!(g.up, cmd("VolumeUp"));
    assert_eq!(g.down, cmd("VolumeDown"));
    assert_eq!(g.left, cmd(r"open:C:\Users\you"));
    assert_eq!(g.right, cmd("lock"));
    assert_eq!(g.click, cmd("Alt+Left"));
    assert_eq!(g.threshold_px, 45);
}

#[test]
fn an_invalid_gesture_falls_back_to_its_default() {
    let loaded = Config::from_toml(
        r#"
        [gestures]
        up = "Win+Tabb"
        down = 5
        left = ""
        right = "Ctrl+Shift"
        threshold_px = 2
        click = "native"
        "#,
    )
    .expect("valid TOML");
    let keys: Vec<_> = loaded.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "gestures.up",
            "gestures.down",
            "gestures.left",
            "gestures.right",
            "gestures.threshold_px"
        ]
    );
    assert_eq!(loaded.config.gestures, Config::default().gestures);
    assert_eq!(
        loaded.warnings[0].to_string(),
        "gestures.up: unknown key \"Tabb\"; using the default \"Win+Tab\""
    );
}

#[test]
fn threshold_limits_are_inclusive() {
    for v in [10, 200] {
        let (config, warnings) = load(&format!("[gestures]\nthreshold_px = {v}"));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.gestures.threshold_px, v);
    }
    for v in ["9", "201", "-30", "30.5", "\"30\""] {
        let (config, warnings) = load(&format!("[gestures]\nthreshold_px = {v}"));
        assert_eq!(warnings, ["gestures.threshold_px"], "value {v}");
        assert_eq!(config.gestures.threshold_px, 30);
    }
}

// ---------------------------------------------------------------------------
// Apps and exclusion
// ---------------------------------------------------------------------------

#[test]
fn the_exclusion_list_is_normalized() {
    let (config, warnings) = load(
        r#"
        [apps]
        disable_in_fullscreen = false
        excluded = ["MSTSC.EXE", "game", 'C:\Games\Other.exe', "mstsc.exe"]
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(!config.apps.disable_in_fullscreen);
    let names: Vec<_> = config.apps.excluded.iter().map(AppName::as_str).collect();
    assert_eq!(names, ["mstsc.exe", "game.exe", "other.exe"]);
}

#[test]
fn a_single_name_works_as_a_list() {
    let (config, warnings) = load("[apps]\nexcluded = \"mstsc.exe\"");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.apps.excluded, [app("mstsc.exe")]);
}

#[test]
fn bad_exclusion_entries_are_skipped_with_a_warning() {
    let (config, warnings) = load(
        r#"
        [apps]
        disable_in_fullscreen = "yes"
        excluded = ["ok.exe", 3, "", { name = "x" }]
        "#,
    );
    assert_eq!(
        warnings,
        [
            "apps.disable_in_fullscreen",
            "apps.excluded[1]",
            "apps.excluded[2]",
            "apps.excluded[3]"
        ]
    );
    assert!(config.apps.disable_in_fullscreen);
    assert_eq!(config.apps.excluded, [app("ok.exe")]);

    let (config, warnings) = load("[apps]\nexcluded = 7");
    assert_eq!(warnings, ["apps.excluded"]);
    assert!(config.apps.excluded.is_empty());
}

// ---------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------

#[test]
fn profiles_are_read() {
    let (config, warnings) = load(
        r#"
        [[profiles]]
        name = "Browsers"
        apps = ["browser.exe", "other-browser"]

        [profiles.gestures]
        left = "Ctrl+Shift+Tab"
        right = "Ctrl+Tab"

        [[profiles]]
        apps = "editor.exe"
        gestures = { click = "none" }
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.profiles.len(), 2);

    let browsers = &config.profiles[0];
    assert_eq!(browsers.name.as_deref(), Some("Browsers"));
    assert_eq!(
        browsers.apps,
        [app("browser.exe"), app("other-browser.exe")]
    );
    assert_eq!(browsers.gestures.left, Some(cmd("Ctrl+Shift+Tab")));
    assert_eq!(browsers.gestures.right, Some(cmd("Ctrl+Tab")));
    assert_eq!(browsers.gestures.up, None);

    let editor = &config.profiles[1];
    assert_eq!(editor.name, None);
    assert_eq!(editor.gestures.click, Some(Command::Nothing));
}

#[test]
fn a_profile_without_apps_is_ignored() {
    let (config, warnings) = load(
        r#"
        [[profiles]]
        name = "Forgot the apps"
        gestures = { left = "Alt+Left" }

        [[profiles]]
        apps = []
        gestures = { left = "Alt+Left" }
        "#,
    );
    assert_eq!(warnings, ["profiles[0].apps", "profiles[1].apps"]);
    assert!(config.profiles.is_empty());
}

#[test]
fn gestures_written_directly_in_the_profile_get_a_hint() {
    let loaded = Config::from_toml(
        r#"
        [[profiles]]
        apps = ["browser.exe"]
        left = "Ctrl+Shift+Tab"
        "#,
    )
    .expect("valid TOML");
    let keys: Vec<_> = loaded.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(keys, ["profiles[0].left"]);
    assert!(loaded.warnings[0].message.contains("gestures = {"));
    // The profile still applies, just without that gesture.
    assert_eq!(loaded.config.profiles.len(), 1);
    assert_eq!(loaded.config.profiles[0].gestures.left, None);
}

#[test]
fn invalid_profile_values_are_reported_and_skipped() {
    let (config, warnings) = load(
        r#"
        profiles = [
          "not a table",
          { apps = ["a.exe"], gestures = { left = "Ctrl+Nope", up = "F5", swipe = "x" }, color = "blue" },
        ]
        "#,
    );
    assert_eq!(
        warnings,
        [
            "profiles[0]",
            "profiles[1].gestures.left",
            "profiles[1].gestures.swipe",
            "profiles[1].color"
        ]
    );
    assert_eq!(config.profiles.len(), 1);
    let g = &config.profiles[0].gestures;
    assert_eq!(g.left, None);
    assert_eq!(g.up, Some(cmd("F5")));

    let (config, warnings) = load("profiles = \"x\"");
    assert_eq!(warnings, ["profiles"]);
    assert!(config.profiles.is_empty());
}

#[test]
fn an_excluded_app_in_a_profile_gets_a_warning() {
    let (config, warnings) = load(
        r#"
        [apps]
        excluded = ["game.exe"]

        [[profiles]]
        apps = ["game.exe"]
        gestures = { up = "F5" }
        "#,
    );
    assert_eq!(warnings, ["profiles[0].apps"]);
    assert_eq!(config.profiles.len(), 1);
}

// ---------------------------------------------------------------------------
// Ring commands
// ---------------------------------------------------------------------------

#[test]
fn ring_action_commands_are_validated() {
    let (config, warnings) = load(
        r#"
        [[ring.slots]]
        type = "action"
        name = "Broken"
        command = "Ctrl+Nope"

        [[ring.slots]]
        type = "folder"
        name = "Media"
        items = [{ name = "Mute", command = "VolumeMute" }, { name = "Bad", command = 3 }]
        "#,
    );
    assert_eq!(
        warnings,
        ["ring.slots[0].command", "ring.slots[1].items[1].command"]
    );
    // The action stays in the ring, just without a command.
    assert_eq!(config.ring.slots[0], Slot::Action(action("Broken")));
    let Slot::Folder { items, .. } = &config.ring.slots[1] else {
        panic!("expected a folder")
    };
    assert_eq!(items[0].command, Some(cmd("VolumeMute")));
    assert_eq!(items[1].command, None);
}

// ---------------------------------------------------------------------------
// Round trip of the new sections
// ---------------------------------------------------------------------------

#[test]
fn gestures_apps_and_profiles_read_back_identical() {
    let mut config = Config::default();
    config.gestures.up = cmd(r#"open:"C:\Program Files\App\app.exe" --x"#);
    config.gestures.click = cmd("Alt+Left");
    config.gestures.threshold_px = 50;
    config.apps.disable_in_fullscreen = false;
    config.apps.excluded = vec![app("mstsc.exe")];
    config.profiles = vec![
        Profile {
            name: Some("Browsers".into()),
            apps: vec![app("browser.exe"), app("other-browser.exe")],
            gestures: GestureOverrides {
                left: Some(cmd("Ctrl+Shift+Tab")),
                click: Some(Command::Nothing),
                ..GestureOverrides::default()
            },
        },
        Profile {
            name: None,
            apps: vec![app("editor.exe")],
            gestures: GestureOverrides::default(),
        },
    ];

    let text = config.to_toml().expect("serializes");
    let loaded = Config::from_toml(&text).expect("reads back");
    assert!(loaded.warnings.is_empty(), "{:?}\n{text}", loaded.warnings);
    assert_eq!(loaded.config, config, "\n{text}");
}
