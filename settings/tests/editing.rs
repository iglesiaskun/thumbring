//! The editor changes only what the interface changed, and what it writes
//! is exactly what the engine reads back, without warnings.

use serde_json::{Value as Json, json};
use thumbring_core::config::{Config, DEFAULT_FILE};
use thumbring_settings::edit;
use toml_edit::DocumentMut;

fn template() -> String {
    DEFAULT_FILE.replace("\r\n", "\n")
}

fn settings_of(text: &str) -> Json {
    let loaded = Config::from_toml(text).expect("valid TOML");
    serde_json::to_value(loaded.config).expect("settings as JSON")
}

/// Applies `change` to the settings of `text`, as the interface would, and
/// returns the new text after checking that the engine reads it back as
/// asked, with no warnings.
fn edited(text: &str, change: impl FnOnce(&mut Json)) -> String {
    let old = settings_of(text);
    let mut new = old.clone();
    change(&mut new);
    let mut doc: DocumentMut = text.parse().expect("toml_edit reads it");
    edit::apply(&mut doc, &old, &new).expect("the change applies");
    let out = doc.to_string();
    let loaded = Config::from_toml(&out).expect("still valid TOML");
    assert!(
        loaded.warnings.is_empty(),
        "warnings: {:?}",
        loaded.warnings
    );
    let read_back = serde_json::to_value(loaded.config).expect("settings as JSON");
    assert!(
        edit::same(&read_back, &new),
        "the engine reads something else:\n{out}"
    );
    out
}

/// The lines that differ, for texts with the same number of lines.
fn changed_lines(before: &str, after: &str) -> Vec<String> {
    assert_eq!(before.lines().count(), after.lines().count(), "{after}");
    before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .map(|(_, b)| b.to_owned())
        .collect()
}

#[test]
fn unchanged_settings_leave_the_file_as_it_is() {
    assert_eq!(edited(&template(), |_| {}), template());
}

#[test]
fn a_changed_value_keeps_its_comment_in_the_same_column() {
    let text = template();
    let out = edited(&text, |s| s["behavior"]["hold_to_open_ms"] = json!(300));
    assert_eq!(
        changed_lines(&text, &out),
        ["hold_to_open_ms = 300       # 60 to 1000"]
    );

    let out = edited(&text, |s| s["behavior"]["hold_to_open_ms"] = json!(1000));
    assert_eq!(
        changed_lines(&text, &out),
        ["hold_to_open_ms = 1000      # 60 to 1000"]
    );
}

#[test]
fn decimals_stay_decimals() {
    // JavaScript writes 2.0 as 2.
    let text = template();
    let out = edited(&text, |s| s["animation"]["speed"] = json!(2));
    assert_eq!(
        changed_lines(&text, &out),
        ["speed = 2.0                 # 0.5 to 2.0, multiplies every animation"]
    );
}

#[test]
fn paths_keep_their_backslashes() {
    let text = template();
    let out = edited(&text, |s| {
        s["gestures"]["up"] = json!(r"open:C:\Users\me\Downloads");
    });
    // Longer than the spaces before the comment: one space stays.
    assert_eq!(
        changed_lines(&text, &out),
        [r"up = 'open:C:\Users\me\Downloads' # Task View"]
    );
}

#[test]
fn the_ring_is_written_again_with_the_comments_above_it() {
    let out = edited(&template(), |s| {
        s["ring"]["slots"][0]["name"] = json!("Play");
        s["ring"]["slots"][2]["items"][0]["name"] = json!("Back one track");
    });
    let ring = out.find("# THE RING").expect("the ring's comments stay");
    let first = out.find("[[ring.slots]]").expect("the slots are tables");
    assert!(ring < first);
    assert!(out.contains("name = \"Play\"\nicon = \"play\"\ncommand = \"PlayPause\""));
    assert!(out.contains(
        "items = [\n  { name = \"Back one track\", icon = \"skip-back\", command = \"PrevTrack\" },\n"
    ));
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|l| l == line)
}

fn ring_off(text: &str) -> String {
    let empty = json!({ "type": "empty" });
    edited(text, |s| s["ring"]["slots"] = json!(vec![empty; 8]))
}

fn ring_on(text: &str) -> String {
    edited(text, |s| {
        s["ring"]["slots"][0] =
            json!({ "type": "action", "name": "Lock screen", "command": "lock" });
    })
}

/// Where each piece is, in order; `None` if missing.
fn positions(text: &str, pieces: &[&str]) -> Vec<Option<usize>> {
    pieces.iter().map(|p| text.find(p)).collect()
}

#[test]
fn an_empty_ring_is_written_as_an_empty_list_and_back() {
    let off = ring_off(&template());
    assert!(off.ends_with("\nslots = []\n"), "{off}");
    assert!(!has_line(&off, "[[ring.slots]]"));
    // Every comment stays, in its place.
    let order = positions(
        &off,
        &[
            "# THE RING",
            "[ring]",
            "# To turn the ring off",
            "slots = []",
        ],
    );
    assert!(
        order.iter().all(Option::is_some) && order.is_sorted(),
        "{off}"
    );

    let on = ring_on(&off);
    assert!(on.ends_with(
        "[[ring.slots]]\ntype = \"action\"\nname = \"Lock screen\"\ncommand = \"lock\"\n"
    ));
    let order = positions(
        &on,
        &[
            "# THE RING",
            "[ring]",
            "# To turn the ring off",
            "[[ring.slots]]",
        ],
    );
    assert!(
        order.iter().all(Option::is_some) && order.is_sorted(),
        "{on}"
    );
}

#[test]
fn a_ring_without_its_own_header_gets_one_only_while_it_is_empty() {
    // Files written before `[ring] button` existed: the slots alone.
    let older = template().replace(
        "[ring]\nbutton = \"forward\"        # \"forward\", \"back\" or \"middle\"\n",
        "",
    );
    assert!(!has_line(&older, "[ring]"));

    let off = ring_off(&older);
    let order = positions(
        &off,
        &["# THE RING", "# To turn the ring off", "[ring]\nslots = []"],
    );
    assert!(
        order.iter().all(Option::is_some) && order.is_sorted(),
        "{off}"
    );

    let on = ring_on(&off);
    assert!(!has_line(&on, "[ring]"), "only the list, as before:\n{on}");
    let order = positions(
        &on,
        &["# THE RING", "# To turn the ring off", "[[ring.slots]]"],
    );
    assert!(
        order.iter().all(Option::is_some) && order.is_sorted(),
        "{on}"
    );
}

#[test]
fn empty_slots_at_the_end_are_left_out() {
    let out = edited(&template(), |s| {
        s["ring"]["slots"][6] = json!({ "type": "empty" });
        s["ring"]["slots"][7] = json!({ "type": "empty" });
    });
    assert_eq!(out.lines().filter(|l| *l == "[[ring.slots]]").count(), 6);
}

#[test]
fn a_new_profile_is_added_and_removed() {
    let profile = json!({
        "name": "Browsers",
        "apps": ["browser.exe"],
        "gestures": { "left": "Ctrl+Tab", "right": "Ctrl+Shift+Tab" },
    });
    let out = edited(&template(), |s| s["profiles"] = json!([profile]));
    assert!(out.contains(
        "[[profiles]]\nname = \"Browsers\"\napps = [\"browser.exe\"]\n\
         gestures = { left = \"Ctrl+Tab\", right = \"Ctrl+Shift+Tab\" }\n"
    ));

    let removed = edited(&out, |s| {
        s.as_object_mut().expect("an object").remove("profiles");
    });
    assert!(!has_line(&removed, "[[profiles]]"));
    assert!(has_line(&removed, "# [[profiles]]"), "the examples stay");
}

#[test]
fn a_missing_section_is_created() {
    let out = edited("version = 1\n", |s| {
        s["behavior"]["hold_to_open_ms"] = json!(300)
    });
    assert_eq!(out, "version = 1\n\n[behavior]\nhold_to_open_ms = 300\n");
}

#[test]
fn a_list_of_apps_is_replaced() {
    let text = template();
    let out = edited(&text, |s| {
        s["apps"]["excluded"] = json!(["game.exe", "remote.exe"])
    });
    assert_eq!(
        changed_lines(&text, &out),
        [r#"excluded = ["game.exe", "remote.exe"]"#]
    );
}
