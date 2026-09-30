//! Settings storage for Git Ronin.
//!
//! Two TOML files live in the config directory:
//! - `portable.toml`: preferences that make sense on any machine. This is the
//!   part that export/import ([`bundle`]) and config sync ([`sync`]) carry.
//! - `local.toml`: machine-specific state such as repository paths.
//!
//! Secrets never go in either file; they belong in the OS keyring.
//! [`secrets::find_secret`] guards what leaves the machine.

pub mod accounts;
pub mod bundle;
pub mod merge;
pub mod secrets;
pub mod sync;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use accounts::{Account, ProviderKind};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Portable {
    pub ui: UiPrefs,
    pub git: GitPrefs,
    /// Command id → shortcuts such as `Mod+Shift+P` (`Mod` is Ctrl, or Cmd
    /// on macOS), separated by spaces, replacing the command's defaults.
    /// An empty string unbinds it.
    pub keybindings: BTreeMap<String, String>,
    /// Identities to work as; never empty (the first is the default).
    pub profiles: Vec<Profile>,
}

impl Default for Portable {
    fn default() -> Self {
        Self {
            ui: UiPrefs::default(),
            git: GitPrefs::default(),
            keybindings: BTreeMap::new(),
            profiles: vec![Profile::default()],
        }
    }
}

impl Portable {
    /// The profile with `id`, else the first one.
    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.id == id)
            .or(self.profiles.first())
    }
}

/// An identity: who commits are by, how they are signed, and which ssh key
/// is used. Empty fields leave git's own configuration in charge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Profile {
    /// Stable identifier; also keys the profile's tabs in `local.toml`.
    pub id: String,
    pub name: String,
    pub user_name: String,
    pub user_email: String,
    /// `user.signingKey`: a key id, or for ssh signing a public key path.
    pub signing_key: String,
    pub signing_format: SigningFormat,
    /// Sign every commit and annotated tag; off leaves git's config in charge.
    pub sign_commits: bool,
    /// Private key file for ssh remotes (`core.sshCommand`).
    pub ssh_key: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: DEFAULT_PROFILE.into(),
            name: "Default".into(),
            user_name: String::new(),
            user_email: String::new(),
            signing_key: String::new(),
            signing_format: SigningFormat::Openpgp,
            sign_commits: false,
            ssh_key: String::new(),
        }
    }
}

impl Profile {
    /// Whether the profile changes anything about how git runs.
    pub fn overrides_git(&self) -> bool {
        !(self.user_name.is_empty()
            && self.user_email.is_empty()
            && self.signing_key.is_empty()
            && !self.sign_commits
            && self.ssh_key.is_empty())
    }
}

pub const DEFAULT_PROFILE: &str = "default";

/// `gpg.format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SigningFormat {
    Openpgp,
    Ssh,
    X509,
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
    /// Changed files as a folder tree rather than a flat list.
    pub file_tree: bool,
    /// Width of the graph column in pixels; 0 sizes it to the lanes.
    pub graph_width: u32,
    pub terminal_font_size: u32,
    /// Short sounds when an action succeeds or fails. Off by default.
    pub sounds: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            show_avatars: true,
            diff_view: DiffView::Unified,
            ignore_whitespace: false,
            file_tree: false,
            graph_width: 0,
            terminal_font_size: 13,
            sounds: false,
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
    /// Tabs of the active profile.
    pub open_tabs: Vec<String>,
    pub active_tab: Option<String>,
    /// Per-repository state, keyed by repository path.
    pub repos: BTreeMap<String, RepoState>,
    /// Id of the profile in use; an unknown id means the first profile.
    pub active_profile: String,
    /// Tabs of the other profiles, keyed by profile id.
    pub profile_tabs: BTreeMap<String, Tabs>,
    /// Named groups of repositories.
    pub workspaces: Vec<Workspace>,
    /// Where the portable settings are synced to, if anywhere.
    pub sync: Option<SyncSettings>,
    /// Shell for the terminal panel; empty means the platform default.
    pub terminal_shell: String,
    /// Accounts on hosting services, each belonging to a profile. Tokens
    /// are in the OS keyring, so accounts stay on this machine.
    pub accounts: Vec<Account>,
    /// Look for a new version at startup and twice a day.
    pub check_updates: CheckUpdates,
}

/// Whether to look for new versions by itself. Local, since how the app is
/// installed (and so updated) differs between machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, type = "boolean")]
pub struct CheckUpdates(pub bool);

impl Default for CheckUpdates {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Tabs {
    pub open_tabs: Vec<String>,
    pub active_tab: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Workspace {
    pub name: String,
    /// Repository paths, in the order shown.
    pub repos: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SyncSettings {
    /// A private repository the user owns.
    pub remote: String,
    pub branch: String,
    /// Push committed changes this often (they are also pushed on exit).
    pub push_minutes: u32,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            remote: String::new(),
            branch: "main".into(),
            push_minutes: 5,
        }
    }
}

impl Local {
    /// Switches to profile `id`, keeping the current tabs for when the
    /// previous profile comes back.
    pub fn switch_profile(&mut self, id: &str) {
        if self.active_profile == id {
            return;
        }
        let previous = Tabs {
            open_tabs: std::mem::take(&mut self.open_tabs),
            active_tab: self.active_tab.take(),
        };
        let old = std::mem::replace(&mut self.active_profile, id.to_owned());
        self.profile_tabs.insert(old, previous);
        let next = self.profile_tabs.remove(id).unwrap_or_default();
        self.open_tabs = next.open_tabs;
        self.active_tab = next.active_tab;
    }

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
    #[error("could not read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Git(#[from] ronin_git::Error),
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

    /// Switches to profile `id`, keeping the current profile's tabs for
    /// when it comes back.
    pub fn switch_profile(&mut self, id: &str) -> Result<()> {
        // The saved id may be unset or stale; the tabs belong to the
        // profile actually in use.
        let current = self
            .config
            .portable
            .profile(&self.config.local.active_profile)
            .map(|p| p.id.clone());
        self.update_local(|l| {
            if let Some(current) = current {
                l.active_profile = current;
            }
            l.switch_profile(id);
        })
    }

    /// Replaces the portable settings wholesale (an import).
    pub fn set_portable(&mut self, portable: Portable) -> Result<()> {
        self.update_portable(|p| *p = portable)
    }

    /// Rereads `portable.toml` after something else (a sync) wrote it.
    pub fn reload_portable(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();
        self.config.portable = read_or_default(&self.dir.join(PORTABLE_FILE), &mut warnings);
        warnings
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Parses portable settings, filling what's missing with defaults.
pub fn parse_portable(text: &str) -> Result<Portable> {
    let portable: Portable = toml::from_str(text).map_err(|e| Error::Invalid(e.to_string()))?;
    Ok(portable.normalized())
}

impl Portable {
    /// Keeps the invariants other code relies on: at least one profile,
    /// unique non-empty ids.
    pub fn normalized(mut self) -> Self {
        let mut seen = std::collections::HashSet::new();
        for (i, profile) in self.profiles.iter_mut().enumerate() {
            if profile.id.is_empty() || !seen.insert(profile.id.clone()) {
                profile.id = format!("profile-{i}");
                seen.insert(profile.id.clone());
            }
        }
        if self.profiles.is_empty() {
            self.profiles.push(Profile::default());
        }
        self
    }
}

/// Settings files fix up what a hand edit may have broken after parsing.
trait Normalize {
    fn normalize(self) -> Self;
}

impl Normalize for Portable {
    fn normalize(self) -> Self {
        self.normalized()
    }
}

impl Normalize for Local {
    fn normalize(self) -> Self {
        self
    }
}

fn read_or_default<T: DeserializeOwned + Default + Normalize>(
    path: &Path,
    warnings: &mut Vec<String>,
) -> T {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return T::default(),
        Err(e) => {
            warnings.push(format!("could not read {}: {e}", path.display()));
            return T::default();
        }
    };
    match toml::from_str::<T>(&text) {
        Ok(value) => value.normalize(),
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
    write_text_atomic(path, &toml::to_string_pretty(value)?)
}

pub(crate) fn write_text_atomic(path: &Path, text: &str) -> Result<()> {
    let write_err = |source| Error::Write {
        path: path.to_owned(),
        source,
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(write_err)?;
    }
    let tmp = path.with_extension("tmp");
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
    fn update_checks_default_to_on_and_are_a_plain_flag() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = ConfigStore::load(dir.path());
        assert_eq!(store.get().local.check_updates, CheckUpdates(true));
        store
            .update_local(|l| l.check_updates = CheckUpdates(false))
            .unwrap();
        let text = fs::read_to_string(dir.path().join("local.toml")).unwrap();
        assert!(text.contains("checkUpdates = false"), "{text}");
        let (reloaded, _) = ConfigStore::load(dir.path());
        assert_eq!(reloaded.get().local.check_updates, CheckUpdates(false));
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
    fn profiles_keep_their_own_tabs() {
        let mut local = Local {
            open_tabs: vec!["/a".into(), "/b".into()],
            active_tab: Some("/b".into()),
            ..Local::default()
        };
        local.switch_profile("work");
        assert!(local.open_tabs.is_empty());
        assert_eq!(local.active_tab, None);
        local.open_tabs.push("/w".into());

        local.switch_profile("");
        assert_eq!(local.open_tabs, ["/a", "/b"]);
        assert_eq!(local.active_tab.as_deref(), Some("/b"));
        local.switch_profile("work");
        assert_eq!(local.open_tabs, ["/w"]);
    }

    #[test]
    fn switching_from_an_unset_profile_keeps_the_default_profiles_tabs() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = ConfigStore::load(dir.path());
        store
            .update_portable(|p| {
                p.profiles.push(Profile {
                    id: "work".into(),
                    ..Profile::default()
                })
            })
            .unwrap();
        store
            .update_local(|l| l.open_tabs = vec!["/mine".into()])
            .unwrap();
        store.switch_profile("work").unwrap();
        assert!(store.get().local.open_tabs.is_empty());
        store.switch_profile(DEFAULT_PROFILE).unwrap();
        assert_eq!(store.get().local.open_tabs, ["/mine"]);
    }

    #[test]
    fn settings_always_have_a_profile_with_a_unique_id() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("portable.toml"), "profiles = []\n").unwrap();
        let (store, _) = ConfigStore::load(dir.path());
        assert_eq!(store.get().portable.profiles, [Profile::default()]);

        let p =
            parse_portable("[[profiles]]\nid = \"a\"\n[[profiles]]\nid = \"a\"\n[[profiles]]\n")
                .unwrap();
        let ids: Vec<&str> = p.profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["a", "profile-1", "default"]);
        assert_eq!(p.profile("nope").unwrap().id, "a");
        assert!(!p.profiles[0].overrides_git());
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
