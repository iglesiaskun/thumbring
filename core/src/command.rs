//! Commands: what a gesture or a ring action does.
//!
//! In the configuration file every command is a short piece of text:
//!
//! | Text | Command |
//! |---|---|
//! | `"Ctrl+Shift+T"`, `"Win+Tab"`, `"F5"` | keyboard shortcut |
//! | `"VolumeUp"`, `"PlayPause"` | media or volume key (a key without modifiers) |
//! | `"open:C:\folder"`, `"open:https://..."` | opens an app, file, folder or URL |
//! | `"screenshot"`, `"emoji"`, `"explorer"`, `"lock"` | built-in actions |
//! | `"native"` | what the button would do without the engine |
//! | `"none"` | nothing |
//!
//! Nothing here talks to Windows: the engine maps each [`Key`] to a key code
//! and runs the command.

use std::fmt;
use std::str::FromStr;

use serde::{Serialize, Serializer};

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Passes the button's own click through, as if the engine did not exist.
    Native,
    /// Does nothing. Useful to turn a gesture off in a profile.
    Nothing,
    Keys(Chord),
    Open(Open),
    Builtin(Builtin),
}

/// Opens an app, file, folder or URL.
///
/// The engine expands environment variables (`%USERPROFILE%`) before opening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    pub target: String,
    /// Command line arguments, only when the target is quoted:
    /// `open:"C:\Program Files\App\app.exe" --option`.
    pub args: Option<String>,
}

/// Built-in actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    /// Snipping screenshot (Win+Shift+S).
    Screenshot,
    /// Emoji panel (Win+.).
    Emoji,
    /// File Explorer (Win+E).
    Explorer,
    /// Locks the screen.
    Lock,
}

impl Builtin {
    const ALL: [Builtin; 4] = [
        Builtin::Screenshot,
        Builtin::Emoji,
        Builtin::Explorer,
        Builtin::Lock,
    ];

    /// Shortcut that does the same thing. `Lock` has none: Windows ignores a
    /// simulated Win+L, so the engine calls the lock function directly.
    pub fn chord(self) -> Option<Chord> {
        match self {
            Builtin::Screenshot => Some(Chord::new(
                Modifiers::WIN.with(Modifiers::SHIFT),
                Key::Char('S'),
            )),
            Builtin::Emoji => Some(Chord::new(Modifiers::WIN, Key::Char('.'))),
            Builtin::Explorer => Some(Chord::new(Modifiers::WIN, Key::Char('E'))),
            Builtin::Lock => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Builtin::Screenshot => "screenshot",
            Builtin::Emoji => "emoji",
            Builtin::Explorer => "explorer",
            Builtin::Lock => "lock",
        }
    }
}

/// Modifier keys of a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub win: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        win: false,
        alt: false,
        shift: false,
    };
    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };
    pub const WIN: Self = Self {
        win: true,
        ..Self::NONE
    };
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };

    /// Union of both sets: `Modifiers::CTRL.with(Modifiers::WIN)`.
    pub const fn with(self, other: Self) -> Self {
        Self {
            ctrl: self.ctrl || other.ctrl,
            win: self.win || other.win,
            alt: self.alt || other.alt,
            shift: self.shift || other.shift,
        }
    }

    /// The modifiers that are on, in the order they are written and in which
    /// the engine presses them: Ctrl, Win, Alt, Shift.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        [
            (self.ctrl, "Ctrl"),
            (self.win, "Win"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
    }
}

/// A shortcut: modifiers plus one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Chord {
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }
}

/// A key, without modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// ASCII letter (always uppercase), digit or another character such as
    /// `.` or `ç`. The engine finds the key through the active keyboard layout.
    Char(char),
    /// F1 to F24.
    F(u8),
    Left,
    Right,
    Up,
    Down,
    Tab,
    Enter,
    Esc,
    Space,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    PrintScreen,
    /// Context menu key.
    Apps,
    VolumeUp,
    VolumeDown,
    VolumeMute,
    PlayPause,
    NextTrack,
    PrevTrack,
    MediaStop,
    BrowserBack,
    BrowserForward,
    BrowserRefresh,
}

/// Accepted key names. The first name of each key is the one written when a
/// command becomes text; the others are aliases.
const NAMED_KEYS: &[(&str, Key)] = &[
    ("Left", Key::Left),
    ("Right", Key::Right),
    ("Up", Key::Up),
    ("Down", Key::Down),
    ("Tab", Key::Tab),
    ("Enter", Key::Enter),
    ("Return", Key::Enter),
    ("Esc", Key::Esc),
    ("Escape", Key::Esc),
    ("Space", Key::Space),
    ("Backspace", Key::Backspace),
    ("Delete", Key::Delete),
    ("Del", Key::Delete),
    ("Insert", Key::Insert),
    ("Ins", Key::Insert),
    ("Home", Key::Home),
    ("End", Key::End),
    ("PageUp", Key::PageUp),
    ("PgUp", Key::PageUp),
    ("PageDown", Key::PageDown),
    ("PgDn", Key::PageDown),
    ("PrintScreen", Key::PrintScreen),
    ("PrtSc", Key::PrintScreen),
    ("Apps", Key::Apps),
    ("Menu", Key::Apps),
    ("VolumeUp", Key::VolumeUp),
    ("VolumeDown", Key::VolumeDown),
    ("VolumeMute", Key::VolumeMute),
    ("Mute", Key::VolumeMute),
    ("PlayPause", Key::PlayPause),
    ("NextTrack", Key::NextTrack),
    ("PrevTrack", Key::PrevTrack),
    ("PreviousTrack", Key::PrevTrack),
    ("MediaStop", Key::MediaStop),
    ("BrowserBack", Key::BrowserBack),
    ("BrowserForward", Key::BrowserForward),
    ("BrowserRefresh", Key::BrowserRefresh),
    // `+` separates the parts of a shortcut, so that key needs a name.
    ("Plus", Key::Char('+')),
    ("Minus", Key::Char('-')),
];

const MODIFIERS: &[(&str, Modifiers)] = &[
    ("Ctrl", Modifiers::CTRL),
    ("Control", Modifiers::CTRL),
    ("Win", Modifiers::WIN),
    ("Windows", Modifiers::WIN),
    ("Alt", Modifiers::ALT),
    ("Shift", Modifiers::SHIFT),
];

/// Why a piece of text is not a valid command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    Empty,
    /// `open:` with nothing after it.
    EmptyTarget,
    /// `open:"C:\...` without the closing quote.
    UnclosedQuote,
    UnknownKey(String),
    /// A part before the last one that is not Ctrl, Win, Alt or Shift.
    NotAModifier(String),
    RepeatedModifier(String),
    /// Only modifiers (`Ctrl+Shift`) or a trailing `+`.
    MissingKey,
    /// Two `+` in a row or a leading `+`.
    EmptyPart,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::Empty => f.write_str("empty command"),
            CommandError::EmptyTarget => f.write_str("nothing to open after \"open:\""),
            CommandError::UnclosedQuote => {
                f.write_str("the quoted path is missing its closing quote")
            }
            CommandError::UnknownKey(k) => write!(f, "unknown key \"{k}\""),
            CommandError::NotAModifier(k) => write!(
                f,
                "\"{k}\" is not Ctrl, Win, Alt or Shift (the main key goes last)"
            ),
            CommandError::RepeatedModifier(k) => write!(f, "the modifier \"{k}\" appears twice"),
            CommandError::MissingKey => f.write_str("a key is missing after the modifiers"),
            CommandError::EmptyPart => f.write_str("the shortcut has an extra \"+\""),
        }
    }
}

impl std::error::Error for CommandError {}

impl FromStr for Command {
    type Err = CommandError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        if text.is_empty() {
            return Err(CommandError::Empty);
        }
        if text.eq_ignore_ascii_case("native") {
            return Ok(Command::Native);
        }
        if text.eq_ignore_ascii_case("none") {
            return Ok(Command::Nothing);
        }
        if let Some(builtin) = Builtin::ALL
            .into_iter()
            .find(|b| text.eq_ignore_ascii_case(b.name()))
        {
            return Ok(Command::Builtin(builtin));
        }
        // `get` returns `None` if the cut falls inside a character.
        if text
            .get(..5)
            .is_some_and(|p| p.eq_ignore_ascii_case("open:"))
        {
            return parse_open(&text[5..]).map(Command::Open);
        }
        parse_chord(text).map(Command::Keys)
    }
}

fn parse_open(rest: &str) -> Result<Open, CommandError> {
    let rest = rest.trim();
    if let Some(quoted) = rest.strip_prefix('"') {
        let end = quoted.find('"').ok_or(CommandError::UnclosedQuote)?;
        let target = quoted[..end].trim();
        let args = quoted[end + 1..].trim();
        if target.is_empty() {
            return Err(CommandError::EmptyTarget);
        }
        return Ok(Open {
            target: target.to_owned(),
            args: (!args.is_empty()).then(|| args.to_owned()),
        });
    }
    if rest.is_empty() {
        return Err(CommandError::EmptyTarget);
    }
    Ok(Open {
        target: rest.to_owned(),
        args: None,
    })
}

fn parse_chord(text: &str) -> Result<Chord, CommandError> {
    let parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let (last, first) = parts.split_last().expect("split yields at least one item");

    let mut modifiers = Modifiers::NONE;
    for part in first {
        if part.is_empty() {
            return Err(CommandError::EmptyPart);
        }
        let flag =
            find_modifier(part).ok_or_else(|| CommandError::NotAModifier(part.to_string()))?;
        if modifiers.with(flag) == modifiers {
            return Err(CommandError::RepeatedModifier(part.to_string()));
        }
        modifiers = modifiers.with(flag);
    }
    if last.is_empty() || find_modifier(last).is_some() {
        return Err(CommandError::MissingKey);
    }
    let key = parse_key(last).ok_or_else(|| CommandError::UnknownKey(last.to_string()))?;
    Ok(Chord { modifiers, key })
}

fn find_modifier(name: &str) -> Option<Modifiers> {
    MODIFIERS
        .iter()
        .find(|(n, _)| name.eq_ignore_ascii_case(n))
        .map(|&(_, m)| m)
}

fn parse_key(name: &str) -> Option<Key> {
    if let Some(&(_, key)) = NAMED_KEYS
        .iter()
        .find(|(n, _)| name.eq_ignore_ascii_case(n))
    {
        return Some(key);
    }
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Some(Key::Char(c.to_ascii_uppercase()));
    }
    // F1 to F24. A lone "F" was already handled above as a letter.
    let number = name.strip_prefix(['F', 'f'])?;
    match number.parse::<u8>() {
        Ok(n @ 1..=24) if !number.starts_with('0') => Some(Key::F(n)),
        _ => None,
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((name, _)) = NAMED_KEYS.iter().find(|(_, k)| k == self) {
            return f.write_str(name);
        }
        match self {
            Key::Char(c) => write!(f, "{c}"),
            Key::F(n) => write!(f, "F{n}"),
            // Every named key is in the table.
            _ => unreachable!("key missing from the name table: {self:?}"),
        }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for name in self.modifiers.names() {
            write!(f, "{name}+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// Writes the command in the same format the parser accepts, so parsing the
/// text again gives the same command.
impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Native => f.write_str("native"),
            Command::Nothing => f.write_str("none"),
            Command::Keys(chord) => write!(f, "{chord}"),
            Command::Open(Open { target, args: None }) => write!(f, "open:{target}"),
            Command::Open(Open {
                target,
                args: Some(args),
            }) => write!(f, "open:\"{target}\" {args}"),
            Command::Builtin(b) => f.write_str(b.name()),
        }
    }
}

impl Serialize for Command {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
