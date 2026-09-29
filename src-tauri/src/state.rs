use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use ronin_config::ConfigStore;
use ronin_git::{GitCli, Graph, GraphFilter};

use crate::watcher::RepoWatcher;

pub type CmdResult<T> = Result<T, String>;

pub fn err(e: impl ToString) -> String {
    e.to_string()
}

pub struct AppState {
    /// Resolved once at startup so the UI can show a banner if git is missing.
    pub git: Result<GitCli, String>,
    pub config: Mutex<ConfigStore>,
    /// Problems found while loading settings, shown once by the UI.
    pub config_warnings: Mutex<Vec<String>>,
    /// Open repositories, keyed by their root path as reported by `open_repo`.
    pub repos: Mutex<HashMap<String, Arc<RepoSession>>>,
}

pub struct RepoSession {
    pub path: PathBuf,
    /// Built on first use and dropped whenever the refs change.
    pub graph: Mutex<Option<Graph>>,
    /// `None` if the platform refused to watch; the UI then relies on manual refresh.
    pub _watcher: Option<RepoWatcher>,
}

impl AppState {
    pub fn new(config: ConfigStore, config_warnings: Vec<String>) -> Self {
        Self {
            git: GitCli::discover().map_err(err),
            config: Mutex::new(config),
            config_warnings: Mutex::new(config_warnings),
            repos: Mutex::new(HashMap::new()),
        }
    }

    pub fn git(&self) -> CmdResult<&GitCli> {
        self.git.as_ref().map_err(Clone::clone)
    }

    pub fn config(&self) -> MutexGuard<'_, ConfigStore> {
        lock(&self.config)
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

    /// Runs `f` on the repository's graph, building it first if needed.
    pub fn with_graph<T>(
        &self,
        repo: &str,
        f: impl FnOnce(&mut Graph, &GitCli) -> ronin_git::Result<T>,
    ) -> CmdResult<T> {
        let session = self.session(repo)?;
        let git = self.git()?;
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
