//! IPC commands exposed to the UI. Errors cross the boundary as plain messages.
//! Anything touching the disk runs on a blocking thread: Tauri runs synchronous
//! commands on the main thread, which would freeze the window.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ronin_config::{Config, UiPrefs};
use ronin_git::{CommitDetail, DiffOptions, FileDiff, GitVersion, GraphPage, Refs, RepoInfo};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, CmdResult, RepoSession, err, lock};
use crate::watcher;

/// Event emitted with the repository path when its refs change on disk.
pub const REPO_CHANGED: &str = "repo-changed";

async fn blocking<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce(&AppHandle, &AppState) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || f(&app, &app.state::<AppState>()))
        .await
        .map_err(err)?
}

#[tauri::command]
pub fn git_version(state: tauri::State<'_, AppState>) -> CmdResult<GitVersion> {
    state.git().map(|g| g.version())
}

#[tauri::command]
pub fn config_get(state: tauri::State<'_, AppState>) -> Config {
    state.config().get().clone()
}

/// Returns settings problems found at startup, once.
#[tauri::command]
pub fn config_take_warnings(state: tauri::State<'_, AppState>) -> Vec<String> {
    std::mem::take(&mut lock(&state.config_warnings))
}

#[tauri::command]
pub async fn config_set_ui(app: AppHandle, ui: UiPrefs) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.config().update_portable(|p| p.ui = ui).map_err(err)
    })
    .await
}

/// Opens a repository in a new tab and records it as recent.
#[tauri::command]
pub async fn open_repo(app: AppHandle, path: PathBuf) -> CmdResult<RepoInfo> {
    blocking(&app, move |app, s| {
        let info = start_session(app, s, &path)?;
        s.config()
            .update_local(|l| {
                l.touch_recent(&info.path);
                if !l.open_tabs.contains(&info.path) {
                    l.open_tabs.push(info.path.clone());
                }
                l.active_tab = Some(info.path.clone());
            })
            .map_err(err)?;
        Ok(info)
    })
    .await
}

/// Reopens the tabs from the last session. Repositories that no longer
/// exist are dropped from the tab list.
#[tauri::command]
pub async fn restore_tabs(app: AppHandle) -> CmdResult<Vec<RepoInfo>> {
    blocking(&app, |app, s| {
        let tabs = s.config().get().local.open_tabs.clone();
        let opened: Vec<RepoInfo> = tabs
            .iter()
            .filter_map(|path| start_session(app, s, Path::new(path)).ok())
            .collect();
        if opened.len() != tabs.len() {
            s.config()
                .update_local(|l| l.open_tabs = opened.iter().map(|r| r.path.clone()).collect())
                .map_err(err)?;
        }
        Ok(opened)
    })
    .await
}

#[tauri::command]
pub async fn close_repo(app: AppHandle, repo: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        lock(&s.repos).remove(&repo);
        s.config()
            .update_local(|l| {
                l.open_tabs.retain(|t| t != &repo);
                if l.active_tab.as_ref() == Some(&repo) {
                    l.active_tab = l.open_tabs.first().cloned();
                }
            })
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn set_active_tab(app: AppHandle, repo: Option<String>) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.config()
            .update_local(|l| l.active_tab = repo)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn list_refs(app: AppHandle, repo: String) -> CmdResult<Refs> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::list_refs(s.git()?, &session.path).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn graph_page(
    app: AppHandle,
    repo: String,
    start: usize,
    end: usize,
) -> CmdResult<GraphPage> {
    blocking(&app, move |_, s| {
        s.with_graph(&repo, |graph, _| graph.page(start, end))
    })
    .await
}

/// Row indices matching `query`; `by_path` treats it as a pathspec instead.
#[tauri::command]
pub async fn graph_search(
    app: AppHandle,
    repo: String,
    query: String,
    by_path: bool,
) -> CmdResult<Vec<u32>> {
    blocking(&app, move |_, s| {
        s.with_graph(&repo, |graph, git| {
            if by_path {
                graph.search_path(git, &query)
            } else {
                graph.search(&query)
            }
        })
    })
    .await
}

#[tauri::command]
pub async fn graph_locate(app: AppHandle, repo: String, oid: String) -> CmdResult<Option<u32>> {
    blocking(&app, move |_, s| {
        s.with_graph(&repo, |graph, _| graph.locate(&oid))
    })
    .await
}

/// Hides refs from the graph, or shows only `solo` when it is non-empty.
#[tauri::command]
pub async fn set_graph_filter(
    app: AppHandle,
    repo: String,
    hidden: Vec<String>,
    solo: Vec<String>,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.config()
            .update_local(|l| {
                let state = l.repos.entry(repo.clone()).or_default();
                state.hidden_refs = hidden;
                state.solo_refs = solo;
            })
            .map_err(err)?;
        *lock(&s.session(&repo)?.graph) = None;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn commit_detail(app: AppHandle, repo: String, oid: String) -> CmdResult<CommitDetail> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::commit_detail(s.git()?, &session.path, &oid).map_err(err)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn file_diff(
    app: AppHandle,
    repo: String,
    base: Option<String>,
    target: String,
    path: String,
    old_path: Option<String>,
    options: DiffOptions,
) -> CmdResult<FileDiff> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::file_diff(
            s.git()?,
            &session.path,
            base.as_deref(),
            &target,
            &path,
            old_path.as_deref(),
            options,
        )
        .map_err(err)
    })
    .await
}

/// Raw file contents at a commit, e.g. for image previews.
#[tauri::command]
pub async fn blob(app: AppHandle, repo: String, oid: String, path: String) -> CmdResult<Response> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::blob_at(&session.path, &oid, &path)
            .map_err(err)?
            .map(Response::new)
            .ok_or_else(|| format!("{path} does not exist at {oid}"))
    })
    .await
}

/// Opens (or reuses) the session for the repository containing `path`.
fn start_session(app: &AppHandle, state: &AppState, path: &Path) -> CmdResult<RepoInfo> {
    let info = ronin_git::open_repo(path).map_err(err)?;
    let mut repos = lock(&state.repos);
    if !repos.contains_key(&info.path) {
        let (git_dir, common_dir) = ronin_git::git_dirs(path).map_err(err)?;
        let session = Arc::new_cyclic(|weak: &std::sync::Weak<RepoSession>| {
            let weak = weak.clone();
            let app = app.clone();
            let repo = info.path.clone();
            let watcher = watcher::watch(&git_dir, &common_dir, move || {
                if let Some(session) = weak.upgrade() {
                    *lock(&session.graph) = None;
                }
                let _ = app.emit(REPO_CHANGED, &repo);
            })
            .ok();
            RepoSession {
                path: PathBuf::from(&info.path),
                graph: Mutex::new(None),
                _watcher: watcher,
            }
        });
        repos.insert(info.path.clone(), session);
    }
    Ok(info)
}
