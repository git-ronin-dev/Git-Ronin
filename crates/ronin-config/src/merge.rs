//! Key-by-key comparison and merging of settings documents.
//!
//! Settings are compared as TOML trees. Tables merge key by key; arrays of
//! tables that all carry a unique string `id` (profiles) merge element by
//! element; anything else is a single value.

use serde::Serialize;
use toml::{Table, Value};
use ts_rs::TS;

/// One step into a settings document: a table key, or the element of an
/// array of tables with this `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Key(String),
    Id(String),
}

pub type KeyPath = Vec<Segment>;

/// A readable key such as `ui.theme` or `profiles[work].userEmail`.
pub fn display(path: &[Segment]) -> String {
    let mut out = String::new();
    for segment in path {
        match segment {
            Segment::Key(key) => {
                if !out.is_empty() {
                    out.push('.');
                }
                if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    out.push_str(key);
                } else {
                    out.push_str(&format!("\"{key}\""));
                }
            }
            Segment::Id(id) => out.push_str(&format!("[{id}]")),
        }
    }
    out
}

/// A value as it would appear in a TOML file.
pub fn show(value: &Value) -> String {
    value.to_string()
}

/// A setting that differs between two documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Change {
    pub key: String,
    /// `None`: not set on this side.
    pub current: Option<String>,
    pub incoming: Option<String>,
}

/// Every leaf setting that differs between `current` and `incoming`.
pub fn diff(current: &Value, incoming: &Value) -> Vec<Change> {
    let mut changes = Vec::new();
    diff_into(&mut Vec::new(), Some(current), Some(incoming), &mut changes);
    changes
}

fn diff_into(path: &mut KeyPath, a: Option<&Value>, b: Option<&Value>, out: &mut Vec<Change>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Some(Value::Table(a)), Some(Value::Table(b))) => {
            for key in union(a.keys(), b.keys()) {
                path.push(Segment::Key(key.clone()));
                diff_into(path, a.get(&key), b.get(&key), out);
                path.pop();
            }
        }
        (Some(Value::Array(a)), Some(Value::Array(b))) if by_id(a) && by_id(b) => {
            for id in union(ids(a), ids(b)) {
                path.push(Segment::Id(id.clone()));
                diff_into(path, element(a, &id), element(b, &id), out);
                path.pop();
            }
        }
        _ => out.push(Change {
            key: display(path),
            current: a.map(show),
            incoming: b.map(show),
        }),
    }
}

/// `current` with everything `incoming` sets laid over it; settings only
/// `current` has are kept.
pub fn overlay(current: &Value, incoming: &Value) -> Value {
    match (current, incoming) {
        (Value::Table(a), Value::Table(b)) => {
            let mut out = a.clone();
            for (key, value) in b {
                let merged = match a.get(key) {
                    Some(old) => overlay(old, value),
                    None => value.clone(),
                };
                out.insert(key.clone(), merged);
            }
            Value::Table(out)
        }
        (Value::Array(a), Value::Array(b)) if by_id(a) && by_id(b) => {
            let mut out: Vec<Value> = a
                .iter()
                .map(
                    |old| match element(b, id_of(old).map_or("", String::as_str)) {
                        Some(new) => overlay(old, new),
                        None => old.clone(),
                    },
                )
                .collect();
            out.extend(
                b.iter()
                    .filter(|new| element(a, id_of(new).map_or("", String::as_str)).is_none())
                    .cloned(),
            );
            Value::Array(out)
        }
        _ => incoming.clone(),
    }
}

/// A setting both sides changed differently since the common base.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub path: KeyPath,
    pub ours: Option<Value>,
    pub theirs: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    /// The merge, with our side of every conflict.
    pub value: Value,
    pub conflicts: Vec<Conflict>,
}

/// Three-way merge of settings: each side's changes since `base` are
/// kept; a setting both sides changed differently is a conflict.
pub fn merge3(base: Option<&Value>, ours: &Value, theirs: &Value) -> Merged {
    let mut conflicts = Vec::new();
    let value = merge_into(
        &mut Vec::new(),
        base,
        Some(ours),
        Some(theirs),
        &mut conflicts,
    )
    .unwrap_or_else(|| Value::Table(Table::new()));
    Merged { value, conflicts }
}

fn merge_into(
    path: &mut KeyPath,
    base: Option<&Value>,
    ours: Option<&Value>,
    theirs: Option<&Value>,
    conflicts: &mut Vec<Conflict>,
) -> Option<Value> {
    if ours == theirs || base == theirs {
        return ours.cloned();
    }
    if base == ours {
        return theirs.cloned();
    }
    match (ours, theirs) {
        (Some(Value::Table(o)), Some(Value::Table(t))) => {
            let empty = Table::new();
            let b = base.and_then(Value::as_table).unwrap_or(&empty);
            let mut out = Table::new();
            for key in union(o.keys(), t.keys()) {
                path.push(Segment::Key(key.clone()));
                if let Some(v) = merge_into(path, b.get(&key), o.get(&key), t.get(&key), conflicts)
                {
                    out.insert(key, v);
                }
                path.pop();
            }
            Some(Value::Table(out))
        }
        (Some(Value::Array(o)), Some(Value::Array(t)))
            if by_id(o)
                && by_id(t)
                && base.is_none_or(|b| b.as_array().is_some_and(|b| by_id(b))) =>
        {
            let empty = Vec::new();
            let b = base.and_then(Value::as_array).unwrap_or(&empty);
            let mut out = Vec::new();
            for id in union(ids(o), ids(t)) {
                path.push(Segment::Id(id.clone()));
                if let Some(v) = merge_into(
                    path,
                    element(b, &id),
                    element(o, &id),
                    element(t, &id),
                    conflicts,
                ) {
                    out.push(v);
                }
                path.pop();
            }
            Some(Value::Array(out))
        }
        _ => {
            conflicts.push(Conflict {
                path: path.clone(),
                ours: ours.cloned(),
                theirs: theirs.cloned(),
            });
            ours.cloned()
        }
    }
}

/// Sets (or with `None` removes) the value at `path`, creating tables on
/// the way.
pub fn set(root: &mut Value, path: &[Segment], new: Option<Value>) {
    let Some((last, parents)) = path.split_last() else {
        if let Some(new) = new {
            *root = new;
        }
        return;
    };
    let mut node = root;
    for segment in parents {
        node = match segment {
            Segment::Key(key) => {
                let Some(table) = node.as_table_mut() else {
                    return;
                };
                table
                    .entry(key.clone())
                    .or_insert_with(|| Value::Table(Table::new()))
            }
            Segment::Id(id) => {
                let Some(found) = node
                    .as_array_mut()
                    .and_then(|a| a.iter_mut().find(|v| id_of(v) == Some(id)))
                else {
                    return;
                };
                found
            }
        };
    }
    match last {
        Segment::Key(key) => {
            if let Some(table) = node.as_table_mut() {
                match new {
                    Some(v) => {
                        table.insert(key.clone(), v);
                    }
                    None => {
                        table.remove(key);
                    }
                }
            }
        }
        Segment::Id(id) => {
            if let Some(array) = node.as_array_mut() {
                let at = array.iter().position(|v| id_of(v) == Some(id));
                match (at, new) {
                    (Some(i), Some(v)) => array[i] = v,
                    (Some(i), None) => {
                        array.remove(i);
                    }
                    (None, Some(v)) => array.push(v),
                    (None, None) => {}
                }
            }
        }
    }
}

/// Keys of `a` in order, then those only `b` has.
fn union<'a>(
    a: impl Iterator<Item = &'a String>,
    b: impl Iterator<Item = &'a String>,
) -> Vec<String> {
    let mut out: Vec<String> = a.cloned().collect();
    for key in b {
        if !out.contains(key) {
            out.push(key.clone());
        }
    }
    out
}

fn id_of(value: &Value) -> Option<&String> {
    match value.as_table()?.get("id")? {
        Value::String(id) => Some(id),
        _ => None,
    }
}

/// An array of tables keyed by unique `id`s (an empty array qualifies).
fn by_id(values: &[Value]) -> bool {
    let mut seen = std::collections::HashSet::new();
    values
        .iter()
        .all(|v| id_of(v).is_some_and(|id| seen.insert(id)))
}

fn ids(values: &[Value]) -> impl Iterator<Item = &String> {
    values.iter().filter_map(id_of)
}

fn element<'a>(values: &'a [Value], id: &str) -> Option<&'a Value> {
    values.iter().find(|v| id_of(v).is_some_and(|x| x == id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Value {
        Value::Table(toml::from_str(text).unwrap())
    }

    #[test]
    fn diff_lists_leaf_changes_with_readable_keys() {
        let a = v(r#"
            ui = { theme = "dark", showAvatars = true }
            keybindings = { "repo.undo" = "Mod+Z" }
            profiles = [{ id = "default", userEmail = "a@x" }]
        "#);
        let b = v(r#"
            ui = { theme = "light", showAvatars = true }
            keybindings = {}
            profiles = [{ id = "default", userEmail = "b@x" }, { id = "work", name = "Work" }]
        "#);
        let keys: Vec<String> = diff(&a, &b).into_iter().map(|c| c.key).collect();
        assert_eq!(
            keys,
            [
                "keybindings.\"repo.undo\"",
                "profiles[default].userEmail",
                "profiles[work]",
                "ui.theme",
            ]
        );
        let theme = &diff(&a, &b)[3];
        assert_eq!(theme.current.as_deref(), Some("\"dark\""));
        assert_eq!(theme.incoming.as_deref(), Some("\"light\""));
    }

    #[test]
    fn overlay_keeps_current_only_settings_and_merges_profiles_by_id() {
        let a = v(r#"
            ui = { theme = "dark", diffView = "split" }
            profiles = [{ id = "default", name = "Me", userEmail = "a@x" }, { id = "home" }]
        "#);
        let b = v(r#"
            ui = { theme = "light" }
            profiles = [{ id = "default", userEmail = "b@x" }, { id = "work" }]
        "#);
        let out = overlay(&a, &b);
        assert_eq!(
            out,
            v(r#"
            ui = { theme = "light", diffView = "split" }
            profiles = [
                { id = "default", name = "Me", userEmail = "b@x" },
                { id = "home" },
                { id = "work" },
            ]
        "#)
        );
    }

    #[test]
    fn three_way_merge_takes_each_sides_changes() {
        let base = v(r#"ui = { theme = "dark", showAvatars = true }
            git = { autoFetchMinutes = 10 }"#);
        let ours = v(r#"ui = { theme = "light", showAvatars = true }
            git = { autoFetchMinutes = 10 }"#);
        let theirs = v(r#"ui = { theme = "dark", showAvatars = false }
            git = { autoFetchMinutes = 10 }
            keybindings = { "palette.open" = "Mod+K" }"#);
        let merged = merge3(Some(&base), &ours, &theirs);
        assert!(merged.conflicts.is_empty());
        assert_eq!(
            merged.value,
            v(r#"ui = { theme = "light", showAvatars = false }
            git = { autoFetchMinutes = 10 }
            keybindings = { "palette.open" = "Mod+K" }"#)
        );
    }

    #[test]
    fn deleted_on_one_side_and_untouched_on_the_other_stays_deleted() {
        let base = v(r#"profiles = [{ id = "a" }, { id = "b" }]"#);
        let ours = v(r#"profiles = [{ id = "a" }]"#);
        let theirs = v(r#"profiles = [{ id = "a", name = "A" }, { id = "b" }]"#);
        let merged = merge3(Some(&base), &ours, &theirs);
        assert!(merged.conflicts.is_empty());
        assert_eq!(merged.value, v(r#"profiles = [{ id = "a", name = "A" }]"#));
    }

    #[test]
    fn conflicting_changes_are_reported_and_resolved_with_set() {
        let base = v(r#"ui = { theme = "dark" }
            profiles = [{ id = "w", userEmail = "old@x" }]"#);
        let ours = v(r#"ui = { theme = "light" }
            profiles = [{ id = "w", userEmail = "mine@x" }]"#);
        let theirs = v(r#"ui = { theme = "system" }
            profiles = []"#);
        let mut merged = merge3(Some(&base), &ours, &theirs);
        let keys: Vec<String> = merged.conflicts.iter().map(|c| display(&c.path)).collect();
        assert_eq!(keys, ["profiles[w]", "ui.theme"]);
        // Tentatively ours.
        assert_eq!(merged.value["ui"]["theme"].as_str(), Some("light"));

        for c in merged.conflicts.clone() {
            set(&mut merged.value, &c.path, c.theirs);
        }
        assert_eq!(
            merged.value,
            v(r#"ui = { theme = "system" }
            profiles = []"#)
        );
    }

    #[test]
    fn unrelated_documents_merge_without_a_base() {
        let ours = v(r#"ui = { theme = "light" }"#);
        let theirs = v(r#"ui = { theme = "light", showAvatars = false }
            git = { autoFetchMinutes = 0 }"#);
        let merged = merge3(None, &ours, &theirs);
        assert!(merged.conflicts.is_empty());
        assert_eq!(merged.value, theirs);

        let other = v(r#"ui = { theme = "dark" }"#);
        assert_eq!(merge3(None, &ours, &other).conflicts.len(), 1);
    }

    #[test]
    fn set_creates_missing_tables_and_adds_or_removes_elements() {
        let mut doc = v(r#"profiles = [{ id = "a", name = "A" }]"#);
        set(
            &mut doc,
            &[Segment::Key("ui".into()), Segment::Key("theme".into())],
            Some(Value::String("dark".into())),
        );
        set(
            &mut doc,
            &[
                Segment::Key("profiles".into()),
                Segment::Id("a".into()),
                Segment::Key("name".into()),
            ],
            None,
        );
        set(
            &mut doc,
            &[Segment::Key("profiles".into()), Segment::Id("b".into())],
            Some(v(r#"id = "b""#)),
        );
        assert_eq!(
            doc,
            v(r#"profiles = [{ id = "a" }, { id = "b" }]
            ui = { theme = "dark" }"#)
        );
    }
}
