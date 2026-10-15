//! Ring structure: 8 fixed slots, starting at the top, clockwise (45° apart).
//! Each slot is empty, an action or a folder.
//!
//! These types only hold valid data. Reading the file (in [`crate::config`])
//! is what applies the rules and drops whatever does not fit.

use serde::Serialize;

use crate::command::Command;

/// Fixed number of slots in the ring.
pub const SLOT_COUNT: usize = 8;

/// Maximum number of actions inside a folder.
pub const FOLDER_MAX_ITEMS: usize = 9;

/// Icon names an item can use: the Lucide icons the engine embeds. A test in
/// the engine makes sure this list and the embedded files are the same.
pub const ICONS: [&str; 29] = [
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

/// An action the ring can run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    /// Label shown next to the item.
    pub name: String,
    /// Icon name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// What the action does. Without a command, the item shows up but does
    /// nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
}

/// The content of a ring slot.
///
/// In TOML it becomes a table with `type = "empty" | "action" | "folder"`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Slot {
    #[default]
    Empty,
    Action(Action),
    /// A folder holds `Action`, never `Slot`: the type itself rules out a
    /// folder inside a folder. The [`FOLDER_MAX_ITEMS`] limit is applied on load.
    Folder {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        items: Vec<Action>,
    },
}

/// The whole ring. The fixed-size array guarantees, at compile time, that
/// there are always exactly [`SLOT_COUNT`] slots.
///
/// [`Ring::default`] is the ring the engine starts with; [`Ring::empty`] has
/// nothing in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ring {
    pub slots: [Slot; SLOT_COUNT],
}

impl Ring {
    /// A ring with every slot empty.
    pub fn empty() -> Self {
        Self {
            slots: Default::default(),
        }
    }

    /// No slot has anything: there is nothing to show, so the trigger keeps
    /// its own function.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| *slot == Slot::Empty)
    }
}

/// The default ring, with commands that work on any Windows 11.
impl Default for Ring {
    fn default() -> Self {
        Self {
            slots: [
                action("Play or pause", "play", "PlayPause"),
                action("Screenshot", "scan", "screenshot"),
                folder(
                    "Media",
                    "music",
                    &[
                        ("Previous track", "skip-back", "PrevTrack"),
                        ("Next track", "skip-forward", "NextTrack"),
                        ("Volume down", "volume-1", "VolumeDown"),
                        ("Volume up", "volume-2", "VolumeUp"),
                        ("Mute", "volume-x", "VolumeMute"),
                    ],
                ),
                action("Lock screen", "lock", "lock"),
                folder(
                    "Editing",
                    "clipboard",
                    &[
                        ("Copy", "copy", "Ctrl+C"),
                        ("Paste", "clipboard-paste", "Ctrl+V"),
                        ("Cut", "scissors", "Ctrl+X"),
                        ("Undo", "undo-2", "Ctrl+Z"),
                        ("Redo", "redo-2", "Ctrl+Y"),
                        ("Clipboard history", "clipboard-list", "Win+V"),
                    ],
                ),
                action("Emoji", "smile", "emoji"),
                action("File Explorer", "folder-open", "explorer"),
                folder(
                    "Apps",
                    "layout-grid",
                    &[
                        ("Calculator", "calculator", "open:calc.exe"),
                        ("Notepad", "sticky-note", "open:notepad.exe"),
                        ("Terminal", "terminal", "open:wt.exe"),
                        ("Settings", "settings", "open:ms-settings:"),
                    ],
                ),
            ],
        }
    }
}

fn item(name: &str, icon: &str, command: &str) -> Action {
    Action {
        name: name.into(),
        icon: Some(icon.into()),
        command: Some(command.parse().expect("the default commands are valid")),
    }
}

fn action(name: &str, icon: &str, command: &str) -> Slot {
    Slot::Action(item(name, icon, command))
}

fn folder(name: &str, icon: &str, items: &[(&str, &str, &str)]) -> Slot {
    Slot::Folder {
        name: name.into(),
        icon: Some(icon.into()),
        items: items
            .iter()
            .map(|&(name, icon, command)| item(name, icon, command))
            .collect(),
    }
}
