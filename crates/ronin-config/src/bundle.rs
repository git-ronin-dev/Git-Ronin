//! Export and import of the portable settings as a single `.ronin.toml`
//! file.

use serde::Deserialize;
use toml::Value;
use ts_rs::TS;

use crate::merge::{self, Change};
use crate::{Error, Portable, Result};

/// Marks a file as a Git Ronin settings bundle, with its format version.
const MARKER: &str = "gitRonin";
const VERSION: i64 = 1;

/// How an imported bundle is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ImportMode {
    /// Take every setting the bundle has; keep the ones it doesn't.
    Merge,
    /// The bundle becomes the settings; anything it lacks is a default.
    Replace,
}

pub fn export(portable: &Portable) -> Result<String> {
    let mut table = toml::Table::new();
    table.insert(MARKER.into(), Value::Integer(VERSION));
    let Value::Table(settings) = Value::try_from(portable)? else {
        unreachable!("settings serialize to a table");
    };
    table.extend(settings);
    Ok(format!(
        "# Git Ronin settings. Import them from Settings → Sync & backup.\n{}",
        toml::to_string_pretty(&table)?
    ))
}

/// Parses a bundle made by [`export`].
pub fn parse(text: &str) -> Result<Portable> {
    let mut table: toml::Table =
        toml::from_str(text).map_err(|e| Error::Invalid(format!("not a settings file: {e}")))?;
    match table.remove(MARKER) {
        Some(Value::Integer(VERSION)) => {}
        Some(Value::Integer(v)) if v > VERSION => {
            return Err(Error::Invalid(
                "these settings come from a newer version of Git Ronin".into(),
            ));
        }
        _ => return Err(Error::Invalid("not a Git Ronin settings file".into())),
    }
    let portable: Portable = Value::Table(table)
        .try_into()
        .map_err(|e: toml::de::Error| Error::Invalid(e.to_string()))?;
    Ok(portable.normalized())
}

/// What importing `incoming` would change, setting by setting.
pub fn preview(current: &Portable, incoming: &Portable, mode: ImportMode) -> Result<Vec<Change>> {
    let result = apply(current, incoming, mode)?;
    Ok(merge::diff(
        &Value::try_from(current)?,
        &Value::try_from(&result)?,
    ))
}

/// The settings after importing `incoming`.
pub fn apply(current: &Portable, incoming: &Portable, mode: ImportMode) -> Result<Portable> {
    match mode {
        ImportMode::Replace => Ok(incoming.clone()),
        ImportMode::Merge => {
            let merged = merge::overlay(&Value::try_from(current)?, &Value::try_from(incoming)?);
            let portable: Portable = merged
                .try_into()
                .map_err(|e: toml::de::Error| Error::Invalid(e.to_string()))?;
            Ok(portable.normalized())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Profile, Theme};

    fn sample() -> Portable {
        let mut p = Portable::default();
        p.ui.theme = Theme::Light;
        p.keybindings.insert("palette.open".into(), "Mod+K".into());
        p.profiles.push(Profile {
            id: "work".into(),
            name: "Work".into(),
            user_email: "me@work.example".into(),
            ..Profile::default()
        });
        p
    }

    #[test]
    fn round_trips() {
        let text = export(&sample()).unwrap();
        assert!(text.starts_with("# Git Ronin settings"));
        assert!(text.contains("gitRonin = 1"), "{text}");
        assert_eq!(parse(&text).unwrap(), sample());
    }

    #[test]
    fn rejects_other_files() {
        assert!(parse("[ui]\ntheme = \"dark\"\n").is_err());
        assert!(
            parse("gitRonin = 99\n")
                .unwrap_err()
                .to_string()
                .contains("newer")
        );
        assert!(parse("not toml").is_err());
    }

    #[test]
    fn merge_keeps_what_the_bundle_lacks_and_replace_does_not() {
        let mut current = Portable::default();
        current
            .keybindings
            .insert("repo.fetchAll".into(), "Mod+Shift+F".into());
        let incoming = parse(&export(&sample()).unwrap()).unwrap();

        let merged = apply(&current, &incoming, ImportMode::Merge).unwrap();
        assert_eq!(merged.ui.theme, Theme::Light);
        assert_eq!(merged.keybindings.len(), 2);
        assert_eq!(merged.profiles.len(), 2);

        let replaced = apply(&current, &incoming, ImportMode::Replace).unwrap();
        assert_eq!(replaced, incoming);

        let changes = preview(&current, &incoming, ImportMode::Merge).unwrap();
        let keys: Vec<&str> = changes.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            ["keybindings.\"palette.open\"", "profiles[work]", "ui.theme"]
        );
    }
}
