//! Applies the interface's changes to the settings file, keeping everything
//! else as the user wrote it: comments, blank lines, order and formatting.
//!
//! The interface works on the settings as JSON, in the same shape as the
//! TOML file. [`apply`] compares the JSON the interface received with the
//! JSON it sends back and changes only the values that differ. A value that
//! changes keeps the comment after it; a value that was not in the file is
//! added to its section.
//!
//! The lists written as `[[...]]` tables (the ring's slots and the per-app
//! profiles) are written again as a whole when anything in them changes.
//! The comments above the first table of the list stay; comments between
//! its tables do not.

use std::fmt;

use serde_json::{Map, Value as Json};
use toml_edit::{
    Array, ArrayOfTables, Decor, DocumentMut, InlineTable, Item, Table, TableLike, Value,
};

/// Settings written as a list of `[[...]]` tables.
const TABLE_LISTS: [&str; 2] = ["profiles", "ring.slots"];

/// Order of the keys when a table or an inline table is written from
/// scratch, so new entries read like the template. Other keys follow.
const KEY_ORDER: [&str; 10] = [
    "type", "name", "icon", "command", "items", "apps", "up", "down", "left", "right",
];

/// Why the changes could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError {
    /// Key path, such as `behavior.hold_to_open_ms`.
    pub key: String,
    pub message: String,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

impl std::error::Error for EditError {}

/// Changes in `doc` every value that differs between `old` and `new`, two
/// JSON objects in the shape of the settings file.
pub fn apply(doc: &mut DocumentMut, old: &Json, new: &Json) -> Result<(), EditError> {
    let (Some(old), Some(new)) = (old.as_object(), new.as_object()) else {
        return Err(error("", "the settings must be a JSON object"));
    };
    apply_section(Section::Table(doc.as_table_mut()), old, new, "")
}

/// A section of the file: a `[table]`, or an inline `{ table }`, whose own
/// sections must be inline too.
enum Section<'a> {
    Table(&'a mut Table),
    Inline(&'a mut InlineTable),
}

impl Section<'_> {
    fn entries(&mut self) -> &mut dyn TableLike {
        match self {
            Section::Table(table) => *table,
            Section::Inline(table) => *table,
        }
    }
}

fn apply_section(
    mut section: Section<'_>,
    old: &Map<String, Json>,
    new: &Map<String, Json>,
    path: &str,
) -> Result<(), EditError> {
    for (key, value) in new {
        let before = old.get(key);
        if before.is_some_and(|b| same(b, value)) {
            continue;
        }
        let key_path = join(path, key);
        if TABLE_LISTS.contains(&key_path.as_str()) {
            set_table_list(&mut section, key, value, &key_path)?;
        } else if let Json::Object(fields) = value {
            let empty = Map::new();
            let before = before.and_then(Json::as_object).unwrap_or(&empty);
            let child = subsection(&mut section, key, &key_path)?;
            apply_section(child, before, fields, &key_path)?;
        } else {
            // A whole number stays whole where the file has one: 60, not 60.0.
            let whole = section
                .entries()
                .get(key)
                .and_then(Item::as_value)
                .is_some_and(Value::is_integer);
            let like = if whole { None } else { before };
            set_value(section.entries(), key, to_toml(value, like, &key_path)?);
        }
    }
    for key in old.keys().filter(|k| !new.contains_key(*k)) {
        section.entries().remove(key);
    }
    Ok(())
}

/// The section at `key`, created if the file does not have it yet.
fn subsection<'a>(
    section: &'a mut Section<'_>,
    key: &str,
    path: &str,
) -> Result<Section<'a>, EditError> {
    let inline = matches!(section, Section::Inline(_));
    let entries = section.entries();
    if entries.get(key).is_none() {
        let item = if inline {
            Item::Value(Value::InlineTable(InlineTable::new()))
        } else {
            let mut new = Table::new();
            // `ring` holds only the `[[ring.slots]]` list: no `[ring]` header.
            new.set_implicit(path == "ring");
            Item::Table(new)
        };
        entries.insert(key, item);
    }
    match entries.get_mut(key) {
        Some(Item::Table(child)) => Ok(Section::Table(child)),
        Some(Item::Value(Value::InlineTable(child))) => Ok(Section::Inline(child)),
        _ => Err(error(path, "the file has a value here, not a section")),
    }
}

/// Replaces a value, keeping the comment after it in the same column.
fn set_value(table: &mut dyn TableLike, key: &str, mut value: Value) {
    match table.get_mut(key) {
        Some(Item::Value(old)) => {
            keep_decor(old, &mut value);
            *old = value;
        }
        Some(item) => *item = Item::Value(value),
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}

/// Copies the spaces and the comment around `old` to `new`. When the value
/// gets longer or shorter, the spaces before a trailing comment shrink or
/// grow to keep the comment where it was (at least one space stays).
fn keep_decor(old: &Value, new: &mut Value) {
    let decor = old.decor().clone();
    let suffix = decor.suffix().and_then(|s| s.as_str()).map(str::to_owned);
    *new.decor_mut() = decor;
    let Some(suffix) = suffix else { return };
    let spaces = suffix.len() - suffix.trim_start_matches(' ').len();
    if spaces == 0 || !suffix[spaces..].starts_with('#') {
        return;
    }
    let grow = bare_width(new) as isize - bare_width(old) as isize;
    let spaces = (spaces as isize - grow).max(1) as usize;
    let comment = suffix.trim_start_matches(' ');
    new.decor_mut()
        .set_suffix(format!("{}{comment}", " ".repeat(spaces)));
}

fn bare_width(value: &Value) -> usize {
    let mut bare = value.clone();
    bare.decor_mut().clear();
    bare.to_string().chars().count()
}

/// Writes a `[[...]]` list again.
///
/// Empty slots at the end of the ring are left out: the missing ones stay
/// empty. A ring with nothing in it is written `slots = []`, as the template
/// suggests, and an empty list of profiles is removed.
fn set_table_list(
    section: &mut Section<'_>,
    key: &str,
    value: &Json,
    path: &str,
) -> Result<(), EditError> {
    let Json::Array(entries) = value else {
        return Err(error(path, "expected a list"));
    };
    let mut entries = entries.as_slice();
    if path == "ring.slots" {
        while let [rest @ .., last] = entries
            && last.get("type").and_then(Json::as_str) == Some("empty")
        {
            entries = rest;
        }
    }
    // The comments above the list belong to its first table. Written
    // `key = []`, they go above that line, or above the section's header if
    // the section had no header of its own (`[ring]` holding only the list).
    // Only the prefix moves: the comment lines and blank lines before it.
    let mut comments = match section.entries().get(key) {
        Some(Item::ArrayOfTables(list)) => list.get(0).and_then(|t| prefix(t.decor())),
        Some(Item::Value(_)) => section
            .entries()
            .key(key)
            .and_then(|k| prefix(k.leaf_decor()))
            .filter(|p| !p.trim().is_empty()),
        _ => None,
    };
    if entries.is_empty() {
        if path != "ring.slots" {
            section.entries().remove(key);
            return Ok(());
        }
        section
            .entries()
            .insert(key, Item::Value(Value::Array(Array::new())));
        match (section, comments) {
            (Section::Table(table), Some(comments)) if table.is_implicit() => {
                table.set_implicit(false);
                table.decor_mut().set_prefix(comments);
            }
            (section, Some(comments)) => {
                if let Some(mut k) = section.entries().key_mut(key) {
                    k.leaf_decor_mut().set_prefix(comments);
                }
            }
            (Section::Table(table), None) => table.set_implicit(false),
            _ => {}
        }
        return Ok(());
    }
    if comments.is_none()
        && let Section::Table(table) = section
        && table.iter().all(|(k, _)| k == key)
        && !table.is_implicit()
    {
        // The header was there only for `key = []`: its comments go back to
        // the list, which needs no header.
        comments = prefix(table.decor());
        table.decor_mut().clear();
        table.set_implicit(true);
    }
    let mut list = ArrayOfTables::new();
    for (i, entry) in entries.iter().enumerate() {
        let entry_path = format!("{path}[{i}]");
        let Json::Object(fields) = entry else {
            return Err(error(&entry_path, "expected a table"));
        };
        let mut new = Table::new();
        for (field, value) in ordered(fields) {
            let field_path = join(&entry_path, field);
            let value = if field == "items" {
                folder_items(value, &field_path)?
            } else {
                to_toml(value, None, &field_path)?
            };
            new.insert(field, Item::Value(value));
        }
        if i == 0
            && let Some(comments) = &comments
        {
            new.decor_mut().set_prefix(comments.as_str());
        }
        list.push(new);
    }
    section.entries().insert(key, Item::ArrayOfTables(list));
    Ok(())
}

fn prefix(decor: &Decor) -> Option<String> {
    decor.prefix().and_then(|p| p.as_str()).map(str::to_owned)
}

/// A folder's actions, one inline table per line, as in the template.
fn folder_items(value: &Json, path: &str) -> Result<Value, EditError> {
    let Json::Array(items) = value else {
        return Err(error(path, "expected a list of actions"));
    };
    let mut array = Array::new();
    for (i, item) in items.iter().enumerate() {
        let mut item = to_toml(item, None, &format!("{path}[{i}]"))?;
        item.decor_mut().set_prefix("\n  ");
        array.push_formatted(item);
    }
    if !items.is_empty() {
        array.set_trailing_comma(true);
        array.set_trailing("\n");
    }
    Ok(Value::Array(array))
}

/// Converts a JSON value. `like` is the value it replaces, if any: a number
/// that was a decimal stays a decimal (JavaScript writes 1.0 as 1).
fn to_toml(value: &Json, like: Option<&Json>, path: &str) -> Result<Value, EditError> {
    Ok(match value {
        Json::Bool(b) => Value::from(*b),
        Json::String(s) => Value::from(s.as_str()),
        Json::Number(n) => {
            let decimal =
                n.is_f64() || like.is_some_and(|l| matches!(l, Json::Number(l) if l.is_f64()));
            match n.as_i64() {
                Some(i) if !decimal => Value::from(i),
                _ => Value::from(
                    n.as_f64()
                        .ok_or_else(|| error(path, "number out of range"))?,
                ),
            }
        }
        Json::Array(items) => {
            let like = like.and_then(Json::as_array).and_then(|a| a.first());
            let mut array = Array::new();
            for (i, item) in items.iter().enumerate() {
                array.push(to_toml(item, like, &format!("{path}[{i}]"))?);
            }
            Value::Array(array)
        }
        Json::Object(fields) => {
            let mut table = InlineTable::new();
            for (key, field) in ordered(fields) {
                let before = like.and_then(|l| l.get(key));
                table.insert(key, to_toml(field, before, &join(path, key))?);
            }
            table.fmt();
            Value::InlineTable(table)
        }
        Json::Null => return Err(error(path, "a value is missing")),
    })
}

/// The keys of a JSON object in [`KEY_ORDER`], then the others as they came.
fn ordered(fields: &Map<String, Json>) -> Vec<(&str, &Json)> {
    let rank = |k: &str| {
        KEY_ORDER
            .iter()
            .position(|o| *o == k)
            .unwrap_or(KEY_ORDER.len())
    };
    let mut list: Vec<(&str, &Json)> = fields.iter().map(|(k, v)| (k.as_str(), v)).collect();
    list.sort_by_key(|(k, _)| rank(k));
    list
}

/// The same settings without the keys whose value is null, at any depth.
pub fn without_nulls(value: &Json) -> Json {
    match value {
        Json::Object(fields) => Json::Object(
            fields
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), without_nulls(v)))
                .collect(),
        ),
        Json::Array(items) => Json::Array(items.iter().map(without_nulls).collect()),
        other => other.clone(),
    }
}

/// What `base` becomes with the changes from `old` to `new`: what [`apply`]
/// makes of a file whose settings are `base`. Lists are replaced whole.
pub fn merged(base: &Json, old: &Json, new: &Json) -> Json {
    if same(old, new) {
        return base.clone();
    }
    let (Json::Object(base), Json::Object(old), Json::Object(new)) = (base, old, new) else {
        return new.clone();
    };
    let mut out = base.clone();
    for (key, value) in new {
        let before = old.get(key).unwrap_or(&Json::Null);
        let current = base.get(key).unwrap_or(&Json::Null);
        out.insert(key.clone(), merged(current, before, value));
    }
    for key in old.keys().filter(|k| !new.contains_key(*k)) {
        out.remove(key);
    }
    Json::Object(out)
}

/// Equal settings values: numbers compare by value, so 1 and 1.0 are the
/// same, and object keys may come in any order.
pub fn same(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(a), Json::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0),
            _ => a == b,
        },
        (Json::Array(a), Json::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Json::Object(a), Json::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

fn error(key: &str, message: &str) -> EditError {
    EditError {
        key: key.to_owned(),
        message: message.to_owned(),
    }
}
