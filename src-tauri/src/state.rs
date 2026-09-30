use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use ronin_config::{ConfigStore, Portable};
use ronin_git::{GitCli, Graph, GraphFilter, Journal, UndoStyle};

use ronin_hosting::{Transport, UreqTransport};

use crate::askpass::{self, Askpass};
use crate::hosting::{DeviceFlows, Rejected};
use crate::profile::ProfileGit;
use crate::secrets::SecretStore;
use crate::sync::SyncService;
use crate::terminal::Terminals;
use crate::watcher::RepoWatcher;

pub type CmdResult<T> = Result<T, String>;

pub fn err(e: impl ToString) -> String {
    e.to_string()
}

pub struct AppState {
    /// Resolved once at startup so the UI can show a banner if git is missing.
    git: Result<GitCli, String>,
    /// How the active profile changes git's configuration.
    pub profile: Mutex<ProfileGit>,
    pub sync: SyncService,
    pub terminals: Terminals,
    /// `None` if the prompt server could not start; git then relies on
    /// credential helpers alone.
    pub askpass: Option<Arc<Askpass>>,
    pub config: Mutex<ConfigStore>,
    /// Account tokens.
    pub secrets: SecretStore,
    /// How hosting services are reached.
    pub transport: Arc<dyn Transport>,
    pub device_flows: DeviceFlows,
    pub rejected: Rejected,
    /// Held while an expiring token is renewed.
    pub renewing: Mutex<()>,
    /// Problems found while loading settings, shown once by the UI.
    pub config_warnings: Mutex<Vec<String>>,
    /// Open repositories, keyed by their root path as reported by `open_repo`.
    pub repos: Mutex<HashMap<String, Arc<RepoSession>>>,
}

pub struct RepoSession {
    pub path: PathBuf,
    /// Built on first use and dropped whenever the refs change.
    pub graph: Mutex<Option<Graph>>,
    /// Undo history. Also held for the whole of every action that moves
    /// refs, so those run one at a time.
    pub journal: Mutex<Journal>,
    /// `None` if the platform refused to watch; the UI then relies on manual refresh.
    pub _watcher: Option<RepoWatcher>,
}

impl AppState {
    pub fn new(
        config: ConfigStore,
        config_warnings: Vec<String>,
        askpass: Option<Arc<Askpass>>,
        profile: ProfileGit,
    ) -> Self {
        let git = GitCli::discover_with(crate::env::child_env().to_vec()).map(|mut git| {
            for (key, value) in askpass.iter().flat_map(|a| a.env()) {
                git = git.env(key, value);
            }
            git
        });
        Self {
            git: git.map_err(err),
            profile: Mutex::new(profile),
            sync: SyncService::default(),
            terminals: Terminals::default(),
            askpass,
            config: Mutex::new(config),
            secrets: SecretStore::default(),
            transport: Arc::new(UreqTransport::default()),
            device_flows: DeviceFlows::default(),
            rejected: Rejected::default(),
            renewing: Mutex::new(()),
            config_warnings: Mutex::new(config_warnings),
            repos: Mutex::new(HashMap::new()),
        }
    }

    /// Git as the active profile runs it.
    pub fn git(&self) -> CmdResult<GitCli> {
        let git = self.git.as_ref().map_err(Clone::clone)?;
        Ok(lock(&self.profile).apply(git.clone()))
    }

    /// Git for commands the user didn't start: credential prompts fail
    /// instead of popping up.
    pub fn background_git(&self) -> CmdResult<GitCli> {
        Ok(self.git()?.env(askpass::BACKGROUND_ENV, "1"))
    }

    /// Git with the user's own configuration, whatever the profile.
    pub fn own_git(&self) -> CmdResult<GitCli> {
        self.git.as_ref().cloned().map_err(Clone::clone)
    }

    pub fn config(&self) -> MutexGuard<'_, ConfigStore> {
        lock(&self.config)
    }

    /// Changes the portable settings and lets the sync commit them.
    pub fn update_portable(&self, f: impl FnOnce(&mut Portable)) -> CmdResult<()> {
        self.config().update_portable(f).map_err(err)?;
        self.sync.changed();
        Ok(())
    }

    /// Points git at the active profile's configuration. Returns a warning
    /// if the profile could not be applied.
    pub fn apply_profile(&self) -> Option<String> {
        let version = self.git.as_ref().ok()?.version();
        let (dir, profile) = {
            let config = self.config();
            let config_ref = config.get();
            let profile = config_ref
                .portable
                .profile(&config_ref.local.active_profile)
                .cloned();
            (config.dir().to_owned(), profile?)
        };
        lock(&self.profile).activate(&dir, &profile, version).err()
    }

    pub fn session(&self, repo: &str) -> CmdResult<Arc<RepoSession>> {
        lock(&self.repos)
            .get(repo)
            .cloned()
            .ok_or_else(|| format!("repository is not open: {repo}"))
    }

    pub fn graph_filter(&self, repo: &str) -> GraphFilter {
        let config = self.config();
        let state = config.get().local.repos.get(repo);
        GraphFilter {
            hidden: state.map(|s| s.hidden_refs.clone()).unwrap_or_default(),
            solo: state.map(|s| s.solo_refs.clone()).unwrap_or_default(),
        }
    }

    /// Drops the cached graph after an action that moved refs, so the next
    /// page request sees them without waiting for the watcher.
    pub fn refs_moved(&self, repo: &str) {
        if let Ok(session) = self.session(repo) {
            *lock(&session.graph) = None;
        }
    }

    /// Runs an action that may move refs, recording it for undo as `label`.
    pub fn journaled<T>(
        &self,
        repo: &str,
        label: impl Into<String>,
        style: UndoStyle,
        f: impl FnOnce(&GitCli, &Path) -> ronin_git::Result<T>,
    ) -> CmdResult<T> {
        let session = self.session(repo)?;
        let git = &self.git()?;
        let mut journal = lock(&session.journal);
        let before = ronin_git::snapshot(git, &session.path);
        let result = f(git, &session.path);
        self.refs_moved(repo);
        match (before, ronin_git::snapshot(git, &session.path)) {
            (Ok(before), Ok(after)) => {
                journal.record(git, &session.path, label, &before, &after, style)
            }
            // Can't tell what happened; don't offer to undo it.
            _ => journal.clear(),
        }
        result.map_err(err)
    }

    /// Runs `f` on the repository's graph, building it first if needed.
    pub fn with_graph<T>(
        &self,
        repo: &str,
        f: impl FnOnce(&mut Graph, &GitCli) -> ronin_git::Result<T>,
    ) -> CmdResult<T> {
        let session = self.session(repo)?;
        let git = &self.git()?;
        let mut graph = lock(&session.graph);
        if graph.is_none() {
            let filter = self.graph_filter(repo);
            *graph = Some(Graph::open(git, &session.path, &filter).map_err(err)?);
        }
        let graph = graph.as_mut().expect("graph was just built");
        f(graph, git).map_err(err)
    }
}

/// A panic while holding a lock must not take every later command down with it.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
