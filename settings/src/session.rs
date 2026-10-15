//! The settings being edited: what the file had when it was read, and saving
//! the interface's changes back to it.
//!
//! Saving is careful about three things:
//! - Someone may have changed the file since it was read (another editor, or
//!   the user by hand). Then saving stops and asks, unless told to go on.
//! - The changes must make a file that Thumbring reads without new warnings.
//!   The new text is checked with the engine's own reader before it is
//!   written.
//! - Only the values the interface changed are touched (see [`crate::edit`]).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value as Json, json};
use thumbring_core::config::{BlobStyle, Config, DEFAULT_FILE};
use toml_edit::DocumentMut;

use crate::{edit, file};

const BOM: &str = "\u{feff}";

/// What the interface gets: the settings, as the engine reads them, and the
/// problems the engine finds in the file.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    /// In the shape of the file, with every value filled in.
    pub settings: Json,
    /// Values the engine replaced by their defaults or ignored.
    pub warnings: Vec<String>,
    pub path: PathBuf,
    /// The file does not exist yet: saving creates it.
    pub new_file: bool,
    /// The default settings, in the same shape.
    pub defaults: Json,
    /// Allowed range of each number, by key path: `"behavior.hold_to_open_ms": [60, 1000]`.
    pub limits: Json,
    /// The blob shape each style starts from: `{ "viscous": { ... }, "elastic": { ... } }`.
    pub blob_presets: Json,
}

/// Why the file could not be opened.
#[derive(Debug, Clone, Serialize)]
pub struct OpenError {
    pub path: PathBuf,
    pub reason: String,
}

/// Why saving did not happen.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "details", rename_all = "snake_case")]
pub enum SaveError {
    /// The file changed since it was read.
    ChangedOutside,
    /// The changes would make values the engine does not accept.
    Invalid(Vec<String>),
    /// The file on the disk cannot be read as settings any more.
    Unreadable(String),
    /// Reading or writing failed.
    Io(String),
}

pub struct Session {
    path: PathBuf,
    /// The bytes read, to notice changes made outside. `None`: no file yet.
    bytes: Option<Vec<u8>>,
    text: Text,
    settings: Json,
    warnings: Vec<String>,
}

/// The file's text and how it was written, to write it back the same way.
#[derive(Debug, Clone)]
struct Text {
    body: String,
    bom: bool,
    crlf: bool,
}

impl Session {
    /// Reads the file. A file that does not exist yet starts from the
    /// commented defaults, as the engine writes them on its first run.
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let fail = |reason: String| OpenError {
            path: path.to_path_buf(),
            reason,
        };
        let bytes = file::read(path).map_err(|e| fail(e.to_string()))?;
        let text = match &bytes {
            Some(bytes) => decode(bytes).map_err(fail)?,
            // Windows line endings on Windows, as the engine writes it, so
            // any editor opens it well.
            None => Text {
                body: DEFAULT_FILE.replace("\r\n", "\n"),
                bom: false,
                crlf: cfg!(windows),
            },
        };
        let (settings, warnings) = read_settings(&text.body).map_err(fail)?;
        // The editor changes the file with toml_edit, which must read it too.
        text.body
            .parse::<DocumentMut>()
            .map_err(|e| fail(format!("the settings file is not valid TOML: {e}")))?;
        Ok(Self {
            path: path.to_path_buf(),
            bytes,
            text,
            settings,
            warnings,
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            settings: self.settings.clone(),
            warnings: self.warnings.clone(),
            path: self.path.clone(),
            new_file: self.bytes.is_none(),
            defaults: serde_json::to_value(Config::default()).unwrap_or_default(),
            limits: limits(),
            blob_presets: blob_presets(),
        }
    }

    /// Writes `settings` (the interface's copy) to the file. With
    /// `overwrite`, a file changed outside is changed anyway: the values
    /// the interface changed win, and the others keep what the file has now.
    pub fn save(&mut self, settings: &Json, overwrite: bool) -> Result<Snapshot, SaveError> {
        // A value set to null in the interface is a value left out.
        let settings = &edit::without_nulls(settings);
        let current = file::read(&self.path).map_err(|e| SaveError::Io(e.to_string()))?;
        let (base, base_settings, warnings_before) = if current == self.bytes {
            (
                self.text.clone(),
                self.settings.clone(),
                self.warnings.clone(),
            )
        } else if overwrite {
            let text = match &current {
                Some(bytes) => decode(bytes).map_err(SaveError::Unreadable)?,
                None => self.text.clone(),
            };
            let (settings, warnings) = read_settings(&text.body).map_err(SaveError::Unreadable)?;
            (text, settings, warnings)
        } else {
            return Err(SaveError::ChangedOutside);
        };

        let mut doc: DocumentMut = base.body.parse().map_err(|e| {
            SaveError::Unreadable(format!("the settings file is not valid TOML: {e}"))
        })?;
        edit::apply(&mut doc, &self.settings, settings)
            .map_err(|e| SaveError::Invalid(vec![e.to_string()]))?;
        let body = doc.to_string();

        // The engine's own reader has the last word.
        let (saved, warnings) = read_settings(&body).map_err(|e| SaveError::Invalid(vec![e]))?;
        let new_warnings: Vec<String> = warnings
            .iter()
            .filter(|w| !warnings_before.contains(w))
            .cloned()
            .collect();
        if !new_warnings.is_empty() {
            return Err(SaveError::Invalid(new_warnings));
        }
        if !edit::same(
            &saved,
            &edit::merged(&base_settings, &self.settings, settings),
        ) {
            return Err(SaveError::Invalid(vec![
                "the file would not say what the editor shows".into(),
            ]));
        }

        let text = Text { body, ..base };
        let bytes = encode(&text);
        // Writing the same bytes again would only make the engine reload.
        if current.as_ref() != Some(&bytes) {
            file::write_atomic(&self.path, &bytes).map_err(|e| SaveError::Io(e.to_string()))?;
        }
        self.bytes = Some(bytes);
        self.text = text;
        self.settings = saved;
        self.warnings = warnings;
        Ok(self.snapshot())
    }
}

/// The ranges the engine accepts, so the interface never offers others.
fn limits() -> Json {
    use thumbring_core::config::limits::*;
    let ms = [*ANIMATION_MS.start(), *ANIMATION_MS.end()];
    json!({
        "gestures.threshold_px": [*THRESHOLD_PX.start(), *THRESHOLD_PX.end()],
        "behavior.hold_to_open_ms": [*HOLD_TO_OPEN_MS.start(), *HOLD_TO_OPEN_MS.end()],
        "behavior.path_tolerance_ms": [*PATH_TOLERANCE_MS.start(), *PATH_TOLERANCE_MS.end()],
        "animation.speed": [*SPEED.start(), *SPEED.end()],
        "animation.ring_open_ms": ms,
        "animation.ring_close_ms": ms,
        "animation.blob_ms_per_item": ms,
        "animation.folder_items_ms": ms,
        "animation.folder_close_ms": ms,
        "animation.viscosity": [*VISCOSITY.start(), *VISCOSITY.end()],
        "animation.break_at": [*BREAK_AT.start(), *BREAK_AT.end()],
        "animation.wobble": [*WOBBLE.start(), *WOBBLE.end()],
    })
}

/// The blob shape of each style, as choosing a style sets it.
fn blob_presets() -> Json {
    let preset = |style: BlobStyle| {
        let p = style.preset();
        json!({
            "blob_ms_per_item": p.blob_ms_per_item,
            "viscosity": p.viscosity,
            "break_at": p.break_at,
            "wobble": p.wobble,
        })
    };
    json!({ "viscous": preset(BlobStyle::Viscous), "elastic": preset(BlobStyle::Elastic) })
}

/// The settings as the engine reads them, and its warnings.
fn read_settings(text: &str) -> Result<(Json, Vec<String>), String> {
    let loaded = Config::from_toml(text).map_err(|e| e.to_string())?;
    let warnings = loaded.warnings.iter().map(ToString::to_string).collect();
    let settings = serde_json::to_value(&loaded.config).map_err(|e| e.to_string())?;
    Ok((settings, warnings))
}

fn decode(bytes: &[u8]) -> Result<Text, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "the settings file is not UTF-8; save it as UTF-8".to_string())?;
    let (bom, text) = match text.strip_prefix(BOM) {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    Ok(Text {
        crlf: text.contains("\r\n"),
        body: text.replace("\r\n", "\n"),
        bom,
    })
}

/// The text with the line endings and the byte order mark it had.
fn encode(text: &Text) -> Vec<u8> {
    let mut out = String::with_capacity(text.body.len() + 64);
    if text.bom {
        out.push_str(BOM);
    }
    if text.crlf {
        out.push_str(&text.body.replace('\n', "\r\n"));
    } else {
        out.push_str(&text.body);
    }
    out.into_bytes()
}
