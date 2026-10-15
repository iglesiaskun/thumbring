//! Lenient TOML loading.
//!
//! The text first becomes a generic tree (`toml::Table`). Then each field is
//! converted on its own with serde: that way a bad value spoils only itself,
//! not the whole file.

use std::fmt;
use std::ops::RangeInclusive;

use serde::de::DeserializeOwned;
use toml::{Table, Value};

use super::{
    Animation, Apps, Behavior, BlobStyle, Config, FORMAT_VERSION, GestureOverrides, Gestures,
    OpenWith, Profile, limits,
};
use crate::apps::AppName;
use crate::command::Command;
use crate::ring::{Action, FOLDER_MAX_ITEMS, ICONS, Ring, SLOT_COUNT, Slot};

/// Result of loading: an always usable configuration and the warnings about
/// what was corrected or ignored.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: Config,
    pub warnings: Vec<Warning>,
}

/// Something in the file that was replaced by its default or ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Key path, such as `animation.speed` or `ring.slots[2].items[0]`.
    pub key: String,
    pub message: String,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

/// Error that prevents loading. The caller keeps the previous configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// The text is not valid TOML.
    Syntax(toml::de::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Syntax(e) => {
                write!(f, "the settings file is not valid TOML: {e}")
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Syntax(e) => Some(e),
        }
    }
}

impl fmt::Display for BlobStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BlobStyle::Viscous => "viscous",
            BlobStyle::Elastic => "elastic",
        })
    }
}

impl fmt::Display for OpenWith {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            OpenWith::Hold => "hold",
            OpenWith::Tap => "tap",
        })
    }
}

pub(super) fn from_toml(text: &str) -> Result<Loaded, ConfigError> {
    let root: Table = toml::from_str(text).map_err(ConfigError::Syntax)?;
    let mut warnings = Vec::new();

    let mut root = Section::new("", root, &mut warnings);
    let version = root.take("version");
    let animation = root.take("animation");
    let behavior = root.take("behavior");
    let ring = root.take("ring");
    let gestures = root.take("gestures");
    let apps = root.take("apps");
    let profiles = root.take("profiles");
    root.finish();

    read_version(version, &mut warnings);
    let animation = read_animation(animation, &mut warnings);
    let behavior = read_behavior(behavior, &mut warnings);
    let ring = read_ring(ring, &mut warnings);
    let gestures = read_gestures(gestures, &mut warnings);
    let apps = read_apps(apps, &mut warnings);
    // Profiles come after the exclusion list to warn about apps in both.
    let profiles = read_profiles(profiles, &apps.excluded, &mut warnings);
    let config = Config {
        gestures,
        apps,
        profiles,
        animation,
        behavior,
        ring,
    };
    Ok(Loaded { config, warnings })
}

/// Checks the format version. Version 1 is the only format so far, so there
/// is nothing to convert yet; when a version 2 comes, the conversion of older
/// files goes here, before the sections are read.
fn read_version(value: Option<Value>, warnings: &mut Vec<Warning>) {
    let current = i64::from(FORMAT_VERSION);
    match value {
        // Written before the field existed: the same as version 1.
        None => {}
        Some(Value::Integer(v)) if v == current => {}
        Some(Value::Integer(v)) if v > current => warnings.push(Warning::new(
            "version",
            format!(
                "the file is for a newer Thumbring (format {v}, this one reads up to \
                 {current}); what it does not understand falls back to the defaults"
            ),
        )),
        Some(other) => warnings.push(Warning::new(
            "version",
            format!("invalid value ({other}); read as version {current}"),
        )),
    }
}

fn read_gestures(value: Option<Value>, warnings: &mut Vec<Warning>) -> Gestures {
    let mut s = Section::open("gestures", value, warnings);
    let d = Gestures::default();
    let gestures = Gestures {
        up: s.command_or("up", d.up),
        down: s.command_or("down", d.down),
        left: s.command_or("left", d.left),
        right: s.command_or("right", d.right),
        click: s.command_or("click", d.click),
        threshold_px: s.integer("threshold_px", d.threshold_px, limits::THRESHOLD_PX),
    };
    s.finish();
    gestures
}

fn read_apps(value: Option<Value>, warnings: &mut Vec<Warning>) -> Apps {
    let mut s = Section::open("apps", value, warnings);
    let d = Apps::default();
    let apps = Apps {
        disable_in_fullscreen: s.or_default("disable_in_fullscreen", d.disable_in_fullscreen),
        excluded: s.app_list("excluded"),
    };
    s.finish();
    apps
}

fn read_profiles(
    value: Option<Value>,
    excluded: &[AppName],
    warnings: &mut Vec<Warning>,
) -> Vec<Profile> {
    let items = match value {
        None => return Vec::new(),
        Some(Value::Array(items)) => items,
        Some(other) => {
            warnings.push(Warning::new(
                "profiles",
                format!("expected a list of profiles ([[profiles]]), got {other}; ignored"),
            ));
            return Vec::new();
        }
    };

    let mut profiles: Vec<Profile> = Vec::new();
    for (i, value) in items.into_iter().enumerate() {
        let path = format!("profiles[{i}]");
        let Value::Table(table) = value else {
            warnings.push(Warning::new(
                path,
                format!("expected a table, got {value}; the profile was ignored"),
            ));
            continue;
        };
        let mut s = Section::new(path, table, warnings);
        let name = s.text("name");
        let had_apps = s.table.contains_key("apps");
        let mut apps = Vec::new();
        for app in s.app_list("apps") {
            // An app in two profiles: the first one wins, as in a list of rules.
            let earlier = profiles.iter().position(|p| p.apps.contains(&app));
            if let Some(j) = earlier {
                s.warn(
                    "apps",
                    format!("{app} is already in profiles[{j}]; the first one wins"),
                );
                continue;
            }
            if excluded.contains(&app) {
                s.warn(
                    "apps",
                    format!("{app} is in apps.excluded: gestures are off there and the profile has no effect"),
                );
            }
            apps.push(app);
        }
        let gestures_path = s.key("gestures");
        let gestures = read_overrides(gestures_path, s.take("gestures"), &mut *s.warnings);
        // Common mistake: writing the gestures straight into the profile.
        for key in ["up", "down", "left", "right", "click"] {
            if s.take(key).is_some() {
                s.warn(
                    key,
                    format!("profile gestures go inside gestures, as in gestures = {{ {key} = \"...\" }}; ignored"),
                );
            }
        }
        if apps.is_empty() {
            let message = if had_apps {
                "the profile has no apps left; ignored"
            } else {
                "the apps list is missing, as in apps = [\"browser.exe\"]; the profile was ignored"
            };
            s.warn("apps", message.into());
            s.finish();
            continue;
        }
        s.finish();
        profiles.push(Profile {
            name,
            apps,
            gestures,
        });
    }
    profiles
}

fn read_overrides(
    path: String,
    value: Option<Value>,
    warnings: &mut Vec<Warning>,
) -> GestureOverrides {
    let mut s = Section::open(&path, value, warnings);
    let if_invalid = "the gesture from [gestures] applies";
    let overrides = GestureOverrides {
        up: s.command("up", if_invalid),
        down: s.command("down", if_invalid),
        left: s.command("left", if_invalid),
        right: s.command("right", if_invalid),
        click: s.command("click", if_invalid),
    };
    s.finish();
    overrides
}

fn read_animation(value: Option<Value>, warnings: &mut Vec<Warning>) -> Animation {
    let mut s = Section::open("animation", value, warnings);
    // The style comes first because it defines the defaults of the blob shape.
    let style = s.or_default("style", BlobStyle::default());
    let d = Animation::for_style(style);
    let animation = Animation {
        style,
        speed: s.in_range("speed", d.speed, limits::SPEED),
        ring_open_ms: s.millis("ring_open_ms", d.ring_open_ms, limits::ANIMATION_MS),
        ring_close_ms: s.millis("ring_close_ms", d.ring_close_ms, limits::ANIMATION_MS),
        blob_ms_per_item: s.millis("blob_ms_per_item", d.blob_ms_per_item, limits::ANIMATION_MS),
        viscosity: s.in_range("viscosity", d.viscosity, limits::VISCOSITY),
        break_at: s.in_range("break_at", d.break_at, limits::BREAK_AT),
        wobble: s.in_range("wobble", d.wobble, limits::WOBBLE),
        folder_items_ms: s.millis("folder_items_ms", d.folder_items_ms, limits::ANIMATION_MS),
        folder_close_ms: s.millis("folder_close_ms", d.folder_close_ms, limits::ANIMATION_MS),
        ignore_system_reduced_motion: s.or_default(
            "ignore_system_reduced_motion",
            d.ignore_system_reduced_motion,
        ),
    };
    s.finish();
    animation
}

fn read_behavior(value: Option<Value>, warnings: &mut Vec<Warning>) -> Behavior {
    let mut s = Section::open("behavior", value, warnings);
    let d = Behavior::default();
    let behavior = Behavior {
        open_with: s.or_default("open_with", d.open_with),
        execute_on_release: s.or_default("execute_on_release", d.execute_on_release),
        hold_to_open_ms: s.millis(
            "hold_to_open_ms",
            d.hold_to_open_ms,
            limits::HOLD_TO_OPEN_MS,
        ),
        path_tolerance_ms: s.millis(
            "path_tolerance_ms",
            d.path_tolerance_ms,
            limits::PATH_TOLERANCE_MS,
        ),
    };
    s.finish();
    behavior
}

fn read_ring(value: Option<Value>, warnings: &mut Vec<Warning>) -> Ring {
    let mut s = Section::open("ring", value, warnings);
    // Without a list of slots (or with something else in its place) the ring
    // is the default one. An empty list turns the ring off.
    let listed = matches!(s.table.get("slots"), Some(Value::Array(_)));
    let slots = s.array("slots");
    s.finish();
    if !listed {
        return Ring::default();
    }

    if slots.len() > SLOT_COUNT {
        warnings.push(Warning::new(
            "ring.slots",
            format!(
                "the ring has {SLOT_COUNT} slots; the {} extra ones were ignored",
                slots.len() - SLOT_COUNT
            ),
        ));
    }
    // Fewer than 8 slots is normal: the missing ones stay empty.
    let mut ring = Ring::empty();
    for (i, value) in slots.into_iter().take(SLOT_COUNT).enumerate() {
        ring.slots[i] = read_slot(format!("ring.slots[{i}]"), value, warnings);
    }
    ring
}

fn read_slot(path: String, value: Value, warnings: &mut Vec<Warning>) -> Slot {
    let Value::Table(table) = value else {
        warnings.push(Warning::new(
            path,
            format!("expected a table, got {value}; the slot stays empty"),
        ));
        return Slot::Empty;
    };
    let mut s = Section::new(path, table, warnings);
    let slot = match s.text("type").as_deref() {
        Some("empty") => Some(Slot::Empty),
        Some("action") => read_action_fields(&mut s, "the slot stays empty").map(Slot::Action),
        Some("folder") => read_folder(&mut s),
        Some(other) => {
            s.warn(
                "type",
                format!("unknown type \"{other}\" (use \"action\", \"folder\" or \"empty\"); the slot stays empty"),
            );
            None
        }
        None => {
            s.warn(
                "type",
                "the type is missing (\"action\", \"folder\" or \"empty\"); the slot stays empty"
                    .into(),
            );
            None
        }
    };
    match slot {
        Some(slot) => {
            s.finish();
            slot
        }
        None => {
            s.discard();
            Slot::Empty
        }
    }
}

fn read_folder(s: &mut Section) -> Option<Slot> {
    let Some(name) = s.text("name") else {
        s.warn("name", "the name is missing; the slot stays empty".into());
        return None;
    };
    let icon = s.icon("icon");

    let mut items = Vec::new();
    for (j, value) in s.array("items").into_iter().enumerate() {
        let path = format!("{}.items[{j}]", s.path);
        if let Some(action) = read_folder_item(path, value, &mut *s.warnings) {
            items.push(action);
        }
    }
    if items.len() > FOLDER_MAX_ITEMS {
        s.warn(
            "items",
            format!(
                "a folder holds at most {FOLDER_MAX_ITEMS} actions; the {} extra ones were ignored",
                items.len() - FOLDER_MAX_ITEMS
            ),
        );
        items.truncate(FOLDER_MAX_ITEMS);
    }
    Some(Slot::Folder { name, icon, items })
}

fn read_folder_item(path: String, value: Value, warnings: &mut Vec<Warning>) -> Option<Action> {
    let Value::Table(table) = value else {
        warnings.push(Warning::new(
            path,
            format!("expected a table, got {value}; the item was ignored"),
        ));
        return None;
    };
    let mut s = Section::new(path, table, warnings);
    // There is no folder inside a folder: the `Slot::Folder` type only holds `Action`.
    let has_items = s.table.contains_key("items");
    let kind = s.text("type");
    let problem = match kind.as_deref() {
        Some("folder") => Some("a folder cannot hold a folder; the item was ignored".to_owned()),
        _ if has_items => Some("a folder cannot hold a folder; the item was ignored".to_owned()),
        None | Some("action") => None,
        Some(other) => Some(format!(
            "a folder only holds actions, got \"{other}\"; the item was ignored"
        )),
    };
    if let Some(message) = problem {
        s.warn_here(message);
        s.discard();
        return None;
    }
    let action = read_action_fields(&mut s, "the item was ignored");
    if action.is_some() {
        s.finish();
    } else {
        s.discard();
    }
    action
}

fn read_action_fields(s: &mut Section, if_invalid: &str) -> Option<Action> {
    let Some(name) = s.text("name") else {
        s.warn("name", format!("the name is missing; {if_invalid}"));
        return None;
    };
    Some(Action {
        name,
        icon: s.icon("icon"),
        command: s.command("command", "the action has no command"),
    })
}

impl Warning {
    fn new(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
        }
    }
}

/// How a field showed up in the file.
enum Field<T> {
    Missing,
    /// Could not be converted; keeps the value as it was written.
    Invalid(String),
    Valid(T),
}

/// A TOML table being read field by field. Every read takes the key out of
/// the table; whatever is left at the end is an unknown key.
struct Section<'w> {
    path: String,
    table: Table,
    warnings: &'w mut Vec<Warning>,
}

impl<'w> Section<'w> {
    fn new(path: impl Into<String>, table: Table, warnings: &'w mut Vec<Warning>) -> Self {
        Self {
            path: path.into(),
            table,
            warnings,
        }
    }

    /// Opens an optional section. Missing becomes an empty table (all defaults).
    fn open(path: &str, value: Option<Value>, warnings: &'w mut Vec<Warning>) -> Self {
        let table = match value {
            None => Table::new(),
            Some(Value::Table(table)) => table,
            Some(other) => {
                warnings.push(Warning::new(
                    path,
                    format!("expected a table, got {other}; using the defaults"),
                ));
                Table::new()
            }
        };
        Self::new(path, table, warnings)
    }

    fn key(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_owned()
        } else {
            format!("{}.{key}", self.path)
        }
    }

    fn warn(&mut self, key: &str, message: String) {
        let key = self.key(key);
        self.warnings.push(Warning { key, message });
    }

    fn warn_here(&mut self, message: String) {
        self.warnings.push(Warning {
            key: self.path.clone(),
            message,
        });
    }

    fn take(&mut self, key: &str) -> Option<Value> {
        self.table.remove(key)
    }

    fn field<T: DeserializeOwned>(&mut self, key: &str) -> Field<T> {
        let Some(value) = self.table.remove(key) else {
            return Field::Missing;
        };
        match T::deserialize(value.clone()) {
            Ok(v) => Field::Valid(v),
            Err(_) => Field::Invalid(value.to_string()),
        }
    }

    fn or_default<T: DeserializeOwned + fmt::Display>(&mut self, key: &str, default: T) -> T {
        match self.field(key) {
            Field::Missing => default,
            Field::Valid(v) => v,
            Field::Invalid(raw) => {
                self.warn(
                    key,
                    format!("invalid value ({raw}); using the default {default}"),
                );
                default
            }
        }
    }

    fn in_range<T>(&mut self, key: &str, default: T, range: RangeInclusive<T>) -> T
    where
        T: DeserializeOwned + PartialOrd + fmt::Display,
    {
        match self.field(key) {
            Field::Missing => default,
            // NaN is never inside a range, so it falls back to the default too.
            Field::Valid(v) if range.contains(&v) => v,
            Field::Valid(v) => {
                let (min, max) = range.into_inner();
                self.warn(
                    key,
                    format!(
                        "{v} is outside the range from {min} to {max}; using the default {default}"
                    ),
                );
                default
            }
            Field::Invalid(raw) => {
                self.warn(
                    key,
                    format!("invalid value ({raw}); using the default {default}"),
                );
                default
            }
        }
    }

    /// Milliseconds.
    fn millis(&mut self, key: &str, default: u32, range: RangeInclusive<u32>) -> u32 {
        self.integer(key, default, range)
    }

    /// Unsigned whole number. Read as `i64` so a negative number is "out of
    /// range" rather than "invalid type".
    fn integer(&mut self, key: &str, default: u32, range: RangeInclusive<u32>) -> u32 {
        let wide = i64::from(*range.start())..=i64::from(*range.end());
        let v = self.in_range(key, i64::from(default), wide);
        u32::try_from(v).unwrap_or(default)
    }

    /// Required command: missing or invalid becomes the default.
    fn command_or(&mut self, key: &str, default: Command) -> Command {
        match self.field::<String>(key) {
            Field::Missing => default,
            Field::Valid(text) => match text.parse() {
                Ok(command) => command,
                Err(e) => {
                    self.warn(key, format!("{e}; using the default \"{default}\""));
                    default
                }
            },
            Field::Invalid(raw) => {
                self.warn(
                    key,
                    format!(
                        "expected a command as text, got {raw}; using the default \"{default}\""
                    ),
                );
                default
            }
        }
    }

    /// Optional command: missing becomes `None`; invalid too, with a warning.
    fn command(&mut self, key: &str, if_invalid: &str) -> Option<Command> {
        match self.field::<String>(key) {
            Field::Missing => None,
            Field::Valid(text) => match text.parse() {
                Ok(command) => Some(command),
                Err(e) => {
                    self.warn(key, format!("{e}; {if_invalid}"));
                    None
                }
            },
            Field::Invalid(raw) => {
                self.warn(
                    key,
                    format!("expected a command as text, got {raw}; {if_invalid}"),
                );
                None
            }
        }
    }

    /// List of executable names. A single name works in place of the list.
    /// Repeated names are kept once.
    fn app_list(&mut self, key: &str) -> Vec<AppName> {
        let values = match self.table.remove(key) {
            None => return Vec::new(),
            Some(Value::Array(values)) => values,
            Some(Value::String(name)) => vec![Value::String(name)],
            Some(other) => {
                self.warn(
                    key,
                    format!("expected a list of .exe names, got {other}; ignored"),
                );
                return Vec::new();
            }
        };
        let mut apps = Vec::new();
        for (i, value) in values.into_iter().enumerate() {
            let item = format!("{key}[{i}]");
            match value {
                Value::String(name) => match AppName::new(&name) {
                    Some(app) if !apps.contains(&app) => apps.push(app),
                    Some(_) => {}
                    None => self.warn(&item, "empty name; ignored".into()),
                },
                other => self.warn(
                    &item,
                    format!("expected an .exe name, got {other}; ignored"),
                ),
            }
        }
        apps
    }

    fn text(&mut self, key: &str) -> Option<String> {
        match self.field::<String>(key) {
            Field::Missing => None,
            Field::Valid(v) => Some(v),
            Field::Invalid(raw) => {
                self.warn(key, format!("expected text, got {raw}; ignored"));
                None
            }
        }
    }

    /// Optional icon name. An unknown one is dropped with a warning: the item
    /// then shows the first letter of its name.
    fn icon(&mut self, key: &str) -> Option<String> {
        let name = self.text(key)?;
        if ICONS.contains(&name.as_str()) {
            return Some(name);
        }
        self.warn(
            key,
            format!("unknown icon \"{name}\"; the item shows the first letter of its name"),
        );
        None
    }

    fn array(&mut self, key: &str) -> Vec<Value> {
        match self.table.remove(key) {
            None => Vec::new(),
            Some(Value::Array(items)) => items,
            Some(other) => {
                self.warn(key, format!("expected a list, got {other}; ignored"));
                Vec::new()
            }
        }
    }

    /// Ends the reading and warns about every key nobody read.
    fn finish(mut self) {
        let unknown: Vec<String> = self.table.keys().cloned().collect();
        for key in unknown {
            self.warn(&key, "unknown key; ignored".into());
        }
    }

    /// Ends without warning about the rest: the table was already dropped with a warning.
    fn discard(self) {}
}
