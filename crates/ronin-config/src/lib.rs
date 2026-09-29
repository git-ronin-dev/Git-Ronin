//! Settings storage for Git Ronin.
//!
//! Two TOML files live in the config directory:
//! - `portable.toml`: preferences that make sense on any machine. This is the
//!   part that export/import and config sync (Phase 5) will carry.
//! - `local.toml`: machine-specific state such as repository paths.
//!
//! Secrets never go in either file; they belong in the OS keyring.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const PORTABLE_FILE: &str = "portable.toml";
const LOCAL_FILE: &str = "local.toml";
const MAX_RECENT: usize = 15;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Config {
    pub portable: Portable,
    pub local: Local,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Portable {
    pub ui: UiPrefs,
    pub git: GitPrefs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct GitPrefs {
    /// Fetch every remote of each open repository this often; 0 turns it off.
    pub auto_fetch_minutes: u32,
}

impl Default for GitPrefs {
    fn default() -> Self {
        Self {
            auto_fetch_minutes: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct UiPrefs {
    pub theme: Theme,
    /// Fetch author avatars from Gravatar (sends a hash of the email address).
    pub show_avatars: bool,
    pub diff_view: DiffView,
    pub ignore_whitespace: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            show_avatars: true,
            diff_view: DiffView::Unified,
            ignore_whitespace: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DiffView {
    Unified,
    Split,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Local {
    /// Most recently opened first.
    pub recent_repos: Vec<String>,
    pub open_tabs: Vec<String>,
    pub active_tab: Option<String>,
    /// Per-repository state, keyed by repository path.
    pub repos: BTreeMap<String, RepoState>,
}

impl Local {
    /// Moves `path` to the front of the recent list.
    pub fn touch_recent(&mut self, path: &str) {
        self.recent_repos.retain(|p| p != path);
        self.recent_repos.insert(0, path.to_owned());
        self.recent_repos.truncate(MAX_RECENT);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct RepoState {
    /// Full ref names hidden from the graph.
    pub hidden_refs: Vec<String>,
    /// When non-empty, only these refs are shown.
    pub solo_refs: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Serialize(#[from] toml::ser::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Owns the config directory. Every update is written to disk immediately.
pub struct ConfigStore {
    dir: PathBuf,
    config: Config,
}

impl ConfigStore {
    /// Loads settings from `dir`. Missing files mean defaults. A file that
    /// can't be parsed is moved aside (so the user's edits aren't lost) and
    /// replaced by defaults; the returned warnings say what happened.
    pub fn load(dir: impl Into<PathBuf>) -> (Self, Vec<String>) {
        let dir = dir.into();
        let mut warnings = Vec::new();
        let config = Config {
            portable: read_or_default(&dir.join(PORTABLE_FILE), &mut warnings),
            local: read_or_default(&dir.join(LOCAL_FILE), &mut warnings),
        };
        (Self { dir, config }, warnings)
    }

    pub fn get(&self) -> &Config {
        &self.config
    }

    pub fn update_portable(&mut self, f: impl FnOnce(&mut Portable)) -> Result<()> {
        f(&mut self.config.portable);
        write_atomic(&self.dir.join(PORTABLE_FILE), &self.config.portable)
    }

    pub fn update_local(&mut self, f: impl FnOnce(&mut Local)) -> Result<()> {
        f(&mut self.config.local);
        write_atomic(&self.dir.join(LOCAL_FILE), &self.config.local)
    }
}

fn read_or_default<T: DeserializeOwned + Default>(path: &Path, warnings: &mut Vec<String>) -> T {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return T::default(),
        Err(e) => {
            warnings.push(format!("could not read {}: {e}", path.display()));
            return T::default();
        }
    };
    match toml::from_str(&text) {
        Ok(value) => value,
        Err(e) => {
            let aside = path.with_extension("toml.invalid");
            let moved = fs::rename(path, &aside).is_ok();
            warnings.push(format!(
                "{} is invalid and was reset to defaults{}: {e}",
                path.display(),
                if moved {
                    format!(" (original kept as {})", aside.display())
                } else {
                    String::new()
                }
            ));
            T::default()
        }
    }
}

/// Writes via a temp file and rename, so a crash never leaves a half-written file.
fn write_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let text = toml::to_string_pretty(value)?;
    let write_err = |source| Error::Write {
        path: path.to_owned(),
        source,
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(write_err)?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).map_err(write_err)?;
    fs::rename(&tmp, path).map_err(write_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_mean_defaults_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let (store, warnings) = ConfigStore::load(dir.path().join("cfg"));
        assert!(warnings.is_empty());
        assert_eq!(store.get(), &Config::default());
        assert!(store.get().portable.ui.show_avatars);
        assert!(!dir.path().join("cfg").exists());
    }

    #[test]
    fn updates_persist_across_loads() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = ConfigStore::load(dir.path());
        store
            .update_portable(|p| {
                p.ui.diff_view = DiffView::Split;
                p.ui.theme = Theme::Light;
            })
            .unwrap();
        store
            .update_local(|l| {
                l.touch_recent("/work/a");
                l.repos.entry("/work/a".into()).or_default().hidden_refs =
                    vec!["refs/heads/x".into()];
            })
            .unwrap();

        let (reloaded, warnings) = ConfigStore::load(dir.path());
        assert!(warnings.is_empty());
        assert_eq!(reloaded.get(), store.get());
        let text = fs::read_to_string(dir.path().join("portable.toml")).unwrap();
        assert!(text.contains("diffView = \"split\""), "{text}");
    }

    #[test]
    fn unknown_and_missing_keys_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("portable.toml"),
            "[ui]\ntheme = \"dark\"\nfromTheFuture = 1\n",
        )
        .unwrap();
        let (store, warnings) = ConfigStore::load(dir.path());
        assert!(warnings.is_empty());
        assert_eq!(store.get().portable.ui.theme, Theme::Dark);
        assert!(store.get().portable.ui.show_avatars);
    }

    #[test]
    fn invalid_files_are_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("local.toml"), "recentRepos = [unclosed").unwrap();
        let (store, warnings) = ConfigStore::load(dir.path());
        assert_eq!(store.get().local, Local::default());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("reset to defaults"));
        assert!(dir.path().join("local.toml.invalid").exists());
    }

    #[test]
    fn recent_repos_are_most_recent_first_and_capped() {
        let mut local = Local::default();
        for i in 0..20 {
            local.touch_recent(&format!("/r/{i}"));
        }
        local.touch_recent("/r/10");
        assert_eq!(local.recent_repos.len(), MAX_RECENT);
        assert_eq!(local.recent_repos[0], "/r/10");
        assert_eq!(local.recent_repos[1], "/r/19");
        assert_eq!(
            local.recent_repos.iter().filter(|p| *p == "/r/10").count(),
            1
        );
    }
}
