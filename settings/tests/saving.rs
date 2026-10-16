//! Saving: a whole new file in one step, written the way it was (line
//! endings, byte order mark), and never over changes made outside.

use std::fs;
use std::path::PathBuf;

use serde_json::json;
use thumbring_core::config::DEFAULT_FILE;
use thumbring_settings::session::{SaveError, Session};

/// An empty folder of its own for each test.
fn folder(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("thumbring-settings-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temporary folder");
    dir
}

fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

#[test]
fn a_missing_file_is_created_on_save() {
    let dir = folder("missing");
    let path = dir.join("config.toml");
    let mut session = Session::open(&path).expect("opens without a file");
    let snapshot = session.snapshot();
    assert!(snapshot.new_file);

    let mut settings = snapshot.settings;
    settings["behavior"]["hold_to_open_ms"] = json!(200);
    session.save(&settings, false).expect("saves");

    let written = fs::read_to_string(&path).expect("created");
    let expected = crlf(DEFAULT_FILE).replace(
        "hold_to_open_ms = 150       #",
        "hold_to_open_ms = 200       #",
    );
    assert_eq!(written, expected);
}

#[test]
fn saving_keeps_line_endings_and_the_byte_order_mark_and_leaves_no_temporary_file() {
    let dir = folder("format");
    let path = dir.join("config.toml");
    let original = format!("\u{feff}{}", crlf(DEFAULT_FILE));
    fs::write(&path, &original).expect("write");

    let mut session = Session::open(&path).expect("opens");
    let mut settings = session.snapshot().settings;
    settings["apps"]["disable_in_fullscreen"] = json!(false);
    session.save(&settings, false).expect("saves");

    let written = fs::read_to_string(&path).expect("read");
    assert!(written.starts_with('\u{feff}'));
    assert!(!written.replace("\r\n", "").contains('\n'), "only CRLF");
    assert_eq!(
        written,
        original.replace(
            "disable_in_fullscreen = true",
            "disable_in_fullscreen = false"
        )
    );
    let names: Vec<_> = fs::read_dir(&dir)
        .expect("list")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names, ["config.toml"]);
}

#[test]
fn a_file_changed_outside_is_not_overwritten_without_asking() {
    let dir = folder("outside");
    let path = dir.join("config.toml");
    fs::write(&path, crlf(DEFAULT_FILE)).expect("write");

    let mut session = Session::open(&path).expect("opens");
    let mut settings = session.snapshot().settings;
    settings["behavior"]["path_tolerance_ms"] = json!(80);

    // Meanwhile, someone changes another value by hand.
    let by_hand = crlf(DEFAULT_FILE).replace("threshold_px = 30", "threshold_px = 45");
    fs::write(&path, &by_hand).expect("write");

    assert!(matches!(
        session.save(&settings, false),
        Err(SaveError::ChangedOutside)
    ));
    assert_eq!(fs::read_to_string(&path).expect("read"), by_hand);

    // Told to go on: both changes are in the file.
    let saved = session.save(&settings, true).expect("saves");
    assert_eq!(saved.settings["gestures"]["threshold_px"], json!(45));
    assert_eq!(saved.settings["behavior"]["path_tolerance_ms"], json!(80));
}

#[test]
fn values_the_engine_would_reject_are_not_written() {
    let dir = folder("invalid");
    let path = dir.join("config.toml");
    fs::write(&path, crlf(DEFAULT_FILE)).expect("write");

    let mut session = Session::open(&path).expect("opens");
    let mut settings = session.snapshot().settings;
    // Below the minimum of 60 ms.
    settings["behavior"]["hold_to_open_ms"] = json!(5);
    match session.save(&settings, false) {
        Err(SaveError::Invalid(warnings)) => {
            assert!(
                warnings[0].starts_with("behavior.hold_to_open_ms"),
                "{warnings:?}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&path).expect("read"), crlf(DEFAULT_FILE));
}

#[test]
fn a_file_with_a_syntax_error_does_not_open() {
    let dir = folder("syntax");
    let path = dir.join("config.toml");
    fs::write(&path, "[behavior\n").expect("write");
    let error = Session::open(&path).err().expect("does not open");
    assert!(error.reason.contains("not valid TOML"), "{}", error.reason);
}

#[test]
fn a_value_set_to_null_is_left_out() {
    let dir = folder("null");
    let path = dir.join("config.toml");
    fs::write(&path, crlf(DEFAULT_FILE)).expect("write");

    let mut session = Session::open(&path).expect("opens");
    let mut settings = session.snapshot().settings;
    // The interface clears an action's icon: the item shows its first letter.
    settings["ring"]["slots"][0]["icon"] = serde_json::Value::Null;
    let saved = session.save(&settings, false).expect("saves");
    assert!(saved.settings["ring"]["slots"][0].get("icon").is_none());
    let written = fs::read_to_string(&path).expect("read");
    assert!(written.contains("name = \"Play or pause\"\r\ncommand = \"PlayPause\""));
}

#[test]
fn without_a_ring_radius_the_ring_follows_the_item_size() {
    let dir = folder("radius");
    let path = dir.join("config.toml");
    fs::write(&path, crlf(DEFAULT_FILE)).expect("write");

    let mut session = Session::open(&path).expect("opens");
    let mut settings = session.snapshot().settings;
    // The interface leaves the radius out to keep it automatic.
    settings["appearance"]["item_size_px"] = json!(60);
    settings["appearance"]["ring_radius_px"] = serde_json::Value::Null;
    let saved = session.save(&settings, false).expect("saves");
    assert_eq!(saved.settings["appearance"]["ring_radius_px"], json!(108.0));
    let written = fs::read_to_string(&path).expect("read");
    assert!(written.contains("item_size_px = 60 "));
    assert!(written.contains("# ring_radius_px = 93.6\r\n"));
    assert!(!written.contains("\r\nring_radius_px"));

    // A radius of its own is written, and taken out again.
    let mut settings = saved.settings;
    settings["appearance"]["ring_radius_px"] = json!(150);
    let saved = session.save(&settings, false).expect("saves");
    assert!(
        fs::read_to_string(&path)
            .expect("read")
            .contains("\r\nring_radius_px = 150")
    );
    let mut settings = saved.settings;
    settings["appearance"]["ring_radius_px"] = serde_json::Value::Null;
    session.save(&settings, false).expect("saves");
    assert!(
        !fs::read_to_string(&path)
            .expect("read")
            .contains("\r\nring_radius_px")
    );
}

#[test]
fn a_draft_shows_the_changes_without_writing_them() {
    let dir = folder("draft");
    let path = dir.join("config.toml");
    fs::write(&path, crlf(DEFAULT_FILE)).expect("write");

    let session = Session::open(&path).expect("opens");
    let mut settings = session.snapshot().settings;
    settings["appearance"]["item_color"] = json!("#1F2026");
    let draft = session.draft(&settings).expect("drafts");
    assert!(draft.contains("item_color = \"#1F2026\""));
    assert_eq!(fs::read_to_string(&path).expect("read"), crlf(DEFAULT_FILE));
}

#[test]
fn a_missing_icon_file_is_a_warning_as_in_the_engine() {
    let dir = folder("icon");
    let path = dir.join("config.toml");
    let missing = dir.join("star.svg");
    let text = crlf(DEFAULT_FILE).replacen(
        "icon = \"play\"",
        &format!("icon = '{}'", missing.display()),
        1,
    );
    fs::write(&path, text).expect("write");

    let session = Session::open(&path).expect("opens");
    let snapshot = session.snapshot();
    assert!(snapshot.settings["ring"]["slots"][0].get("icon").is_none());
    assert!(
        snapshot
            .warnings
            .iter()
            .any(|w| w.contains("was not found")),
        "{:?}",
        snapshot.warnings
    );
}
