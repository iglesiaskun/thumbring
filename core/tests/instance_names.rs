use std::path::Path;

use thumbring_core::instance::{ENGINE, engine_for, file_key, settings_for};

#[test]
fn the_same_file_gives_the_same_key_whatever_the_case_and_slashes() {
    let key = file_key(Path::new(
        r"C:\Users\Me\AppData\Roaming\thumbring\config.toml",
    ));
    assert_eq!(
        key,
        file_key(Path::new(
            "c:/users/me/appdata/roaming/thumbring/config.toml"
        ))
    );
    assert_eq!(key.len(), 16);
}

#[test]
fn other_files_give_other_keys() {
    assert_ne!(
        file_key(Path::new(r"S:\test\config.toml")),
        file_key(Path::new(r"S:\looks\config.toml"))
    );
}

#[test]
fn the_key_never_changes_between_versions() {
    // The engine and the settings window may come from different versions.
    assert_eq!(file_key(Path::new("")), "cbf29ce484222325");
    assert_eq!(file_key(Path::new("a")), "af63dc4c8601ec8c");
}

#[test]
fn each_program_has_its_own_name_for_the_same_file() {
    let file = Path::new(r"S:\test\config.toml");
    assert!(engine_for(file).starts_with(ENGINE));
    assert_ne!(engine_for(file), settings_for(file));
    assert!(settings_for(file).ends_with(&file_key(file)));
}
