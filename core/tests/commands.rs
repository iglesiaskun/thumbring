//! Commands as text: shortcuts, media keys, open, built-in actions, errors
//! with a clear message, and the round trip between text and command.

use thumbring_core::command::{Builtin, Chord, Command, CommandError, Key, Modifiers, Open};

fn parse(text: &str) -> Command {
    text.parse()
        .unwrap_or_else(|e| panic!("\"{text}\" should be valid: {e}"))
}

fn error(text: &str) -> CommandError {
    match text.parse::<Command>() {
        Ok(c) => panic!("\"{text}\" should fail, got {c:?}"),
        Err(e) => e,
    }
}

fn keys(modifiers: Modifiers, key: Key) -> Command {
    Command::Keys(Chord::new(modifiers, key))
}

const CTRL_WIN: Modifiers = Modifiers::CTRL.with(Modifiers::WIN);
const CTRL_SHIFT: Modifiers = Modifiers::CTRL.with(Modifiers::SHIFT);

// ---------------------------------------------------------------------------
// Shortcuts
// ---------------------------------------------------------------------------

#[test]
fn the_default_gestures_parse() {
    assert_eq!(parse("Win+Tab"), keys(Modifiers::WIN, Key::Tab));
    assert_eq!(parse("Win+D"), keys(Modifiers::WIN, Key::Char('D')));
    assert_eq!(parse("Ctrl+Win+Left"), keys(CTRL_WIN, Key::Left));
    assert_eq!(parse("Ctrl+Win+Right"), keys(CTRL_WIN, Key::Right));
}

#[test]
fn case_spaces_and_modifier_order_do_not_matter() {
    let expected = keys(CTRL_SHIFT, Key::Char('T'));
    assert_eq!(parse("ctrl+shift+t"), expected);
    assert_eq!(parse("  Ctrl + Shift + T  "), expected);
    assert_eq!(parse("Shift+Ctrl+T"), expected);
    assert_eq!(parse("Control+SHIFT+t"), expected);
    assert_eq!(parse("windows+tab"), keys(Modifiers::WIN, Key::Tab));
}

#[test]
fn a_key_alone_is_a_shortcut_without_modifiers() {
    assert_eq!(parse("F5"), keys(Modifiers::NONE, Key::F(5)));
    assert_eq!(parse("f24"), keys(Modifiers::NONE, Key::F(24)));
    assert_eq!(parse("Esc"), keys(Modifiers::NONE, Key::Esc));
    assert_eq!(parse("escape"), keys(Modifiers::NONE, Key::Esc));
    assert_eq!(parse("PgDn"), keys(Modifiers::NONE, Key::PageDown));
    assert_eq!(parse("F"), keys(Modifiers::NONE, Key::Char('F')));
}

#[test]
fn media_and_volume_keys_are_keys() {
    assert_eq!(parse("VolumeUp"), keys(Modifiers::NONE, Key::VolumeUp));
    assert_eq!(parse("volumedown"), keys(Modifiers::NONE, Key::VolumeDown));
    assert_eq!(parse("Mute"), keys(Modifiers::NONE, Key::VolumeMute));
    assert_eq!(parse("PlayPause"), keys(Modifiers::NONE, Key::PlayPause));
    assert_eq!(parse("NextTrack"), keys(Modifiers::NONE, Key::NextTrack));
    assert_eq!(
        parse("PreviousTrack"),
        keys(Modifiers::NONE, Key::PrevTrack)
    );
    assert_eq!(parse("MediaStop"), keys(Modifiers::NONE, Key::MediaStop));
}

#[test]
fn punctuation_and_other_characters_are_keys() {
    assert_eq!(parse("Win+."), keys(Modifiers::WIN, Key::Char('.')));
    assert_eq!(parse("Ctrl+,"), keys(Modifiers::CTRL, Key::Char(',')));
    assert_eq!(parse("Ctrl+-"), keys(Modifiers::CTRL, Key::Char('-')));
    // `+` separates the parts, so the + key is called Plus.
    assert_eq!(parse("Ctrl+Plus"), keys(Modifiers::CTRL, Key::Char('+')));
    assert_eq!(parse("Ctrl+Minus"), keys(Modifiers::CTRL, Key::Char('-')));
    // Letters outside ASCII stay as written.
    assert_eq!(parse("Ctrl+ç"), keys(Modifiers::CTRL, Key::Char('ç')));
}

// ---------------------------------------------------------------------------
// Built-in actions, native and none
// ---------------------------------------------------------------------------

#[test]
fn builtins_native_and_none() {
    assert_eq!(parse("screenshot"), Command::Builtin(Builtin::Screenshot));
    assert_eq!(parse("Emoji"), Command::Builtin(Builtin::Emoji));
    assert_eq!(parse("EXPLORER"), Command::Builtin(Builtin::Explorer));
    assert_eq!(parse("lock"), Command::Builtin(Builtin::Lock));
    assert_eq!(parse("native"), Command::Native);
    assert_eq!(parse("None"), Command::Nothing);
}

#[test]
fn builtins_map_to_their_shortcuts_except_lock() {
    assert_eq!(
        Builtin::Screenshot.chord(),
        Some(Chord::new(
            Modifiers::WIN.with(Modifiers::SHIFT),
            Key::Char('S')
        ))
    );
    assert_eq!(
        Builtin::Emoji.chord(),
        Some(Chord::new(Modifiers::WIN, Key::Char('.')))
    );
    assert_eq!(
        Builtin::Explorer.chord(),
        Some(Chord::new(Modifiers::WIN, Key::Char('E')))
    );
    // Windows ignores a simulated Win+L: the engine calls LockWorkStation.
    assert_eq!(Builtin::Lock.chord(), None);
}

// ---------------------------------------------------------------------------
// Open
// ---------------------------------------------------------------------------

fn open(target: &str, args: Option<&str>) -> Command {
    Command::Open(Open {
        target: target.into(),
        args: args.map(Into::into),
    })
}

#[test]
fn open_takes_paths_urls_and_variables() {
    assert_eq!(
        parse(r"open:C:\Users\you\Downloads"),
        open(r"C:\Users\you\Downloads", None)
    );
    assert_eq!(
        parse("open:https://example.com/a?b=1"),
        open("https://example.com/a?b=1", None)
    );
    assert_eq!(parse("Open: %USERPROFILE% "), open("%USERPROFILE%", None));
    assert_eq!(parse("open:notepad.exe"), open("notepad.exe", None));
}

#[test]
fn spaces_in_an_unquoted_path_are_part_of_the_path() {
    assert_eq!(
        parse(r"open:C:\Program Files\App\app.exe"),
        open(r"C:\Program Files\App\app.exe", None)
    );
}

#[test]
fn a_quoted_path_can_have_arguments() {
    assert_eq!(
        parse(r#"open:"C:\Program Files\App\app.exe" --profile "Work" -n"#),
        open(
            r"C:\Program Files\App\app.exe",
            Some(r#"--profile "Work" -n"#)
        )
    );
    assert_eq!(
        parse(r#"open:"C:\Program Files\App\app.exe""#),
        open(r"C:\Program Files\App\app.exe", None)
    );
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[test]
fn invalid_commands_say_what_is_wrong() {
    assert_eq!(error(""), CommandError::Empty);
    assert_eq!(error("   "), CommandError::Empty);
    assert_eq!(error("open:"), CommandError::EmptyTarget);
    assert_eq!(error(r#"open:"""#), CommandError::EmptyTarget);
    assert_eq!(error(r#"open:"C:\no end"#), CommandError::UnclosedQuote);
    assert_eq!(error("Ctrl+Tabb"), CommandError::UnknownKey("Tabb".into()));
    assert_eq!(error("F25"), CommandError::UnknownKey("F25".into()));
    assert_eq!(error("F0"), CommandError::UnknownKey("F0".into()));
    assert_eq!(error("F05"), CommandError::UnknownKey("F05".into()));
    assert_eq!(error("A+Ctrl"), CommandError::NotAModifier("A".into()));
    assert_eq!(error("Hyper+A"), CommandError::NotAModifier("Hyper".into()));
    assert_eq!(
        error("Ctrl+ctrl+A"),
        CommandError::RepeatedModifier("ctrl".into())
    );
    assert_eq!(error("Ctrl+"), CommandError::MissingKey);
    assert_eq!(error("Ctrl+Shift"), CommandError::MissingKey);
    assert_eq!(error("Win"), CommandError::MissingKey);
    assert_eq!(error("Ctrl++A"), CommandError::EmptyPart);
    assert_eq!(error("+A"), CommandError::EmptyPart);
}

#[test]
fn error_messages_are_readable() {
    assert_eq!(error("Ctrl+Tabb").to_string(), "unknown key \"Tabb\"");
    assert_eq!(
        error("Ctrl+Shift").to_string(),
        "a key is missing after the modifiers"
    );
    assert!(error("A+Ctrl").to_string().contains("goes last"));
}

// ---------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------

#[test]
fn display_is_canonical() {
    let cases = [
        ("shift+ctrl+t", "Ctrl+Shift+T"),
        ("win + tab", "Win+Tab"),
        ("Shift+Win+s", "Win+Shift+S"),
        ("alt+ctrl+del", "Ctrl+Alt+Delete"),
        ("Return", "Enter"),
        ("ctrl+-", "Ctrl+Minus"),
        ("f11", "F11"),
        ("EMOJI", "emoji"),
        ("NATIVE", "native"),
        (r"OPEN:C:\x", r"open:C:\x"),
    ];
    for (input, canonical) in cases {
        assert_eq!(parse(input).to_string(), canonical, "input: {input}");
    }
}

#[test]
fn every_command_reads_back_identical() {
    let texts = [
        "Win+Tab",
        "Ctrl+Win+Left",
        "Ctrl+Alt+Shift+Win+F12",
        "Win+.",
        "Ctrl+Plus",
        "Ctrl+ç",
        "VolumeMute",
        "PrintScreen",
        "Apps",
        "BrowserBack",
        "screenshot",
        "lock",
        "native",
        "none",
        r"open:C:\Program Files\App\app.exe",
        r#"open:"C:\Program Files\App\app.exe" --a "b c""#,
        "open:https://example.com",
    ];
    for text in texts {
        let command = parse(text);
        let written = command.to_string();
        assert_eq!(parse(&written), command, "{text} -> {written}");
    }
}
