//! IPC commands exposed to the UI. Errors cross the boundary as plain messages.
//! Anything touching the disk runs on a blocking thread: Tauri runs synchronous
//! commands on the main thread, which would freeze the window.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ronin_config::{Config, UiPrefs};
use ronin_git::{
    BlobSource, CommitDetail, CommitOptions, DiffOptions, FileDiff, GitVersion, GraphPage, Hunk,
    IgnoreScope, JournalState, LineSelection, OperationAction, Outcome, PatchTarget, Progress,
    PullMode, PushOutcome, PushTarget, Refs, RepoInfo, ResetMode, StashOptions, StatusEntry,
    UndoStyle, WorkingStatus,
};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, CmdResult, RepoSession, err, lock};
use crate::watcher;

/// Event emitted with the repository path when its refs change on disk.
pub const REPO_CHANGED: &str = "repo-changed";
/// Event emitted with the repository path when its index or working tree
/// change on disk.
pub const WORKTREE_CHANGED: &str = "worktree-changed";
/// Event with a progress update from a long-running command.
pub const PROGRESS: &str = "progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    /// The repository path, or the destination of a clone.
    key: String,
    message: String,
    percent: Option<u8>,
}

/// Forwards progress to the UI as `PROGRESS` events, skipping repeats.
fn reporter(app: &AppHandle, key: &str) -> impl FnMut(Progress) + use<> {
    let app = app.clone();
    let key = key.to_owned();
    let mut last: Option<Progress> = None;
    move |progress: Progress| {
        if last.as_ref() == Some(&progress) {
            return;
        }
        let _ = app.emit(
            PROGRESS,
            ProgressEvent {
                key: key.clone(),
                message: progress.message.clone(),
                percent: progress.percent,
            },
        );
        last = Some(progress);
    }
}

/// A revision for an undo label: full object ids are abbreviated.
fn short(rev: &str) -> &str {
    if rev.len() >= 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
        &rev[..7]
    } else {
        rev
    }
}

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
    blocking(&app, move |app, s| open_tab(app, s, &path)).await
}

/// Clones `url` into `parent/name` and opens it in a new tab.
#[tauri::command]
pub async fn clone_repo(
    app: AppHandle,
    url: String,
    parent: PathBuf,
    name: String,
) -> CmdResult<RepoInfo> {
    blocking(&app, move |app, s| {
        let key = parent.join(&name).to_string_lossy().into_owned();
        let dest = ronin_git::clone(s.git()?, &url, &parent, &name, &mut reporter(app, &key))
            .map_err(err)?;
        open_tab(app, s, &dest)
    })
    .await
}

/// Suggested folder name for a clone of `url`.
#[tauri::command]
pub fn clone_name(url: String) -> Option<String> {
    ronin_git::clone_name(&url)
}

/// Creates a repository at `path` and opens it in a new tab.
#[tauri::command]
pub async fn init_repo(app: AppHandle, path: PathBuf) -> CmdResult<RepoInfo> {
    blocking(&app, move |app, s| {
        ronin_git::init(s.git()?, &path).map_err(err)?;
        open_tab(app, s, &path)
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

/// Current state of an open repository (HEAD may have moved since it was opened).
#[tauri::command]
pub async fn repo_info(app: AppHandle, repo: String) -> CmdResult<RepoInfo> {
    blocking(&app, move |_, s| {
        ronin_git::open_repo(&s.session(&repo)?.path).map_err(err)
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

#[tauri::command]
pub async fn working_status(app: AppHandle, repo: String) -> CmdResult<WorkingStatus> {
    blocking(&app, move |_, s| {
        ronin_git::status(s.git()?, &s.session(&repo)?.path).map_err(err)
    })
    .await
}

/// Diff of an uncommitted change: HEAD to index when `staged`, else index
/// to working tree.
#[tauri::command]
pub async fn working_diff(
    app: AppHandle,
    repo: String,
    entry: StatusEntry,
    staged: bool,
    options: DiffOptions,
) -> CmdResult<FileDiff> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::working_diff(s.git()?, &session.path, &entry, staged, options).map_err(err)
    })
    .await
}

/// Raw contents of a file in HEAD, the index or the working tree.
#[tauri::command]
pub async fn working_blob(
    app: AppHandle,
    repo: String,
    path: String,
    source: BlobSource,
) -> CmdResult<Response> {
    blocking(&app, move |_, s| {
        ronin_git::working_blob(&s.session(&repo)?.path, &path, source)
            .map_err(err)?
            .map(Response::new)
            .ok_or_else(|| format!("{path} does not exist there"))
    })
    .await
}

#[tauri::command]
pub async fn stage_files(app: AppHandle, repo: String, paths: Vec<String>) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        ronin_git::stage_files(s.git()?, &s.session(&repo)?.path, &paths).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn unstage_files(app: AppHandle, repo: String, paths: Vec<String>) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        ronin_git::unstage_files(s.git()?, &s.session(&repo)?.path, &paths).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn discard_files(
    app: AppHandle,
    repo: String,
    entries: Vec<StatusEntry>,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        ronin_git::discard_files(s.git()?, &s.session(&repo)?.path, &entries).map_err(err)
    })
    .await
}

/// Stages, unstages or discards chosen lines of `hunks`, the diff the UI
/// is showing for `path`.
#[tauri::command]
pub async fn apply_lines(
    app: AppHandle,
    repo: String,
    path: String,
    hunks: Vec<Hunk>,
    selection: Vec<LineSelection>,
    target: PatchTarget,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::apply_lines(s.git()?, &session.path, &path, &hunks, &selection, target)
            .map_err(err)
    })
    .await
}

/// Commits the index and returns the new HEAD.
#[tauri::command]
pub async fn commit(
    app: AppHandle,
    repo: String,
    message: String,
    options: CommitOptions,
) -> CmdResult<String> {
    blocking(&app, move |_, s| {
        let label = if options.amend {
            "Amend commit"
        } else {
            "Commit"
        };
        s.journaled(&repo, label, UndoStyle::Soft, |git, path| {
            ronin_git::commit(git, path, &message, options)
        })
    })
    .await
}

#[tauri::command]
pub async fn head_message(app: AppHandle, repo: String) -> CmdResult<Option<String>> {
    blocking(&app, move |_, s| {
        ronin_git::head_message(&s.session(&repo)?.path).map_err(err)
    })
    .await
}

/// Returns false if there was nothing to stash.
#[tauri::command]
pub async fn stash_push(app: AppHandle, repo: String, options: StashOptions) -> CmdResult<bool> {
    blocking(&app, move |_, s| {
        let created = ronin_git::stash_push(s.git()?, &s.session(&repo)?.path, &options);
        s.refs_moved(&repo);
        created.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn stash_apply(
    app: AppHandle,
    repo: String,
    index: u32,
    oid: String,
    pop: bool,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let result = ronin_git::stash_apply(s.git()?, &s.session(&repo)?.path, index, &oid, pop);
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn stash_drop(app: AppHandle, repo: String, index: u32, oid: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let result = ronin_git::stash_drop(s.git()?, &s.session(&repo)?.path, index, &oid);
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

/// Adds a pattern covering `path` to the root `.gitignore`; returns it.
#[tauri::command]
pub async fn add_to_gitignore(
    app: AppHandle,
    repo: String,
    path: String,
    scope: IgnoreScope,
) -> CmdResult<String> {
    blocking(&app, move |_, s| {
        ronin_git::add_to_gitignore(&s.session(&repo)?.path, &path, scope).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn journal_state(app: AppHandle, repo: String) -> CmdResult<JournalState> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        let mut journal = lock(&session.journal);
        journal.state(s.git()?, &session.path).map_err(err)
    })
    .await
}

/// Undoes (or with `redo`, redoes) the last action; returns its label.
#[tauri::command]
pub async fn undo(app: AppHandle, repo: String, redo: bool) -> CmdResult<String> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        let git = s.git()?;
        let mut journal = lock(&session.journal);
        let result = if redo {
            journal.redo(git, &session.path)
        } else {
            journal.undo(git, &session.path)
        };
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn create_branch(
    app: AppHandle,
    repo: String,
    name: String,
    start: String,
    checkout: bool,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let style = if checkout {
            UndoStyle::Checkout
        } else {
            UndoStyle::Keep
        };
        s.journaled(
            &repo,
            format!("Create branch {name}"),
            style,
            |git, path| ronin_git::create_branch(git, path, &name, &start, checkout),
        )
    })
    .await
}

#[tauri::command]
pub async fn checkout_branch(app: AppHandle, repo: String, name: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.journaled(
            &repo,
            format!("Check out {name}"),
            UndoStyle::Checkout,
            |git, path| ronin_git::checkout_branch(git, path, &name),
        )
    })
    .await
}

/// Creates local branch `name` tracking `remote_ref` and checks it out.
#[tauri::command]
pub async fn checkout_remote_branch(
    app: AppHandle,
    repo: String,
    remote_ref: String,
    name: String,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.journaled(
            &repo,
            format!("Check out {name}"),
            UndoStyle::Checkout,
            |git, path| ronin_git::checkout_remote_branch(git, path, &remote_ref, &name),
        )
    })
    .await
}

#[tauri::command]
pub async fn checkout_detached(app: AppHandle, repo: String, rev: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let label = format!("Check out {}", short(&rev));
        s.journaled(&repo, label, UndoStyle::Checkout, |git, path| {
            ronin_git::checkout_detached(git, path, &rev)
        })
    })
    .await
}

#[tauri::command]
pub async fn rename_branch(
    app: AppHandle,
    repo: String,
    old: String,
    new: String,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        let mut journal = lock(&session.journal);
        let result = ronin_git::rename_branch(s.git()?, &session.path, &old, &new);
        s.refs_moved(&repo);
        result.map_err(err)?;
        journal.record_rename(format!("Rename {old} to {new}"), &old, &new);
        Ok(())
    })
    .await
}

/// Deletes a local branch. Without `force`, an unmerged branch fails with a
/// message containing "not fully merged".
#[tauri::command]
pub async fn delete_branch(
    app: AppHandle,
    repo: String,
    name: String,
    force: bool,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.journaled(
            &repo,
            format!("Delete branch {name}"),
            UndoStyle::Keep,
            |git, path| ronin_git::delete_branch(git, path, &name, force),
        )
    })
    .await
}

/// Points branch `name` (not checked out) at `rev`.
#[tauri::command]
pub async fn move_branch(app: AppHandle, repo: String, name: String, rev: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let label = format!("Move {name} to {}", short(&rev));
        s.journaled(&repo, label, UndoStyle::Keep, |git, path| {
            ronin_git::move_branch(git, path, &name, &rev)
        })
    })
    .await
}

#[tauri::command]
pub async fn set_upstream(
    app: AppHandle,
    repo: String,
    name: String,
    upstream: Option<String>,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let result = ronin_git::set_upstream(
            s.git()?,
            &s.session(&repo)?.path,
            &name,
            upstream.as_deref(),
        );
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn create_tag(
    app: AppHandle,
    repo: String,
    name: String,
    target: String,
    message: Option<String>,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.journaled(
            &repo,
            format!("Create tag {name}"),
            UndoStyle::Keep,
            |git, path| ronin_git::create_tag(git, path, &name, &target, message.as_deref()),
        )
    })
    .await
}

#[tauri::command]
pub async fn delete_tag(app: AppHandle, repo: String, name: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.journaled(
            &repo,
            format!("Delete tag {name}"),
            UndoStyle::Keep,
            |git, path| ronin_git::delete_tag(git, path, &name),
        )
    })
    .await
}

#[tauri::command]
pub async fn add_remote(app: AppHandle, repo: String, name: String, url: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let result = ronin_git::add_remote(s.git()?, &s.session(&repo)?.path, &name, &url);
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn edit_remote(
    app: AppHandle,
    repo: String,
    name: String,
    new_name: String,
    url: String,
    push_url: Option<String>,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let result = ronin_git::edit_remote(
            s.git()?,
            &s.session(&repo)?.path,
            &name,
            &new_name,
            &url,
            push_url.as_deref(),
        );
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn remove_remote(app: AppHandle, repo: String, name: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let result = ronin_git::remove_remote(s.git()?, &s.session(&repo)?.path, &name);
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

/// Fetches `remote`, or all remotes. `background` fetches (auto-fetch)
/// never prompt for credentials.
#[tauri::command]
pub async fn fetch(
    app: AppHandle,
    repo: String,
    remote: Option<String>,
    background: bool,
) -> CmdResult<()> {
    blocking(&app, move |app, s| {
        let session = s.session(&repo)?;
        let _serialised = lock(&session.journal);
        let git = if background {
            s.background_git()?
        } else {
            s.git()?.clone()
        };
        let mut report = reporter(app, &repo);
        let mut quiet = |_| {};
        let progress: &mut dyn FnMut(Progress) = if background { &mut quiet } else { &mut report };
        let result = ronin_git::fetch(&git, &session.path, remote.as_deref(), progress);
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn pull(app: AppHandle, repo: String, mode: PullMode) -> CmdResult<Outcome> {
    blocking(&app, move |app, s| {
        let mut report = reporter(app, &repo);
        s.journaled(&repo, "Pull", UndoStyle::Keep, |git, path| {
            ronin_git::pull(git, path, mode, &mut report)
        })
    })
    .await
}

/// Pushes branch `name` to its upstream, or to `target` (which becomes its
/// upstream). Fails with "has no upstream branch" when it has none and no
/// target is given. `force_over` forces the push while the remote branch
/// is still at that commit.
#[tauri::command]
pub async fn push_branch(
    app: AppHandle,
    repo: String,
    name: String,
    target: Option<PushTarget>,
    force_over: Option<String>,
) -> CmdResult<PushOutcome> {
    blocking(&app, move |app, s| {
        let session = s.session(&repo)?;
        let _serialised = lock(&session.journal);
        let result = ronin_git::push_branch(
            s.git()?,
            &session.path,
            &name,
            target.as_ref(),
            force_over.as_deref(),
            &mut reporter(app, &repo),
        );
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn push_tag(
    app: AppHandle,
    repo: String,
    remote: String,
    name: String,
) -> CmdResult<PushOutcome> {
    blocking(&app, move |app, s| {
        let session = s.session(&repo)?;
        let _serialised = lock(&session.journal);
        let result = ronin_git::push_tag(
            s.git()?,
            &session.path,
            &remote,
            &name,
            &mut reporter(app, &repo),
        );
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

/// Deletes a branch or tag (`full_ref`) on `remote`.
#[tauri::command]
pub async fn delete_remote_ref(
    app: AppHandle,
    repo: String,
    remote: String,
    full_ref: String,
) -> CmdResult<()> {
    blocking(&app, move |app, s| {
        let session = s.session(&repo)?;
        let _serialised = lock(&session.journal);
        let result = ronin_git::delete_remote_ref(
            s.git()?,
            &session.path,
            &remote,
            &full_ref,
            &mut reporter(app, &repo),
        );
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn merge(app: AppHandle, repo: String, rev: String, no_ff: bool) -> CmdResult<Outcome> {
    blocking(&app, move |_, s| {
        let label = format!("Merge {}", short(&rev));
        s.journaled(&repo, label, UndoStyle::Keep, |git, path| {
            ronin_git::merge(git, path, &rev, no_ff)
        })
    })
    .await
}

#[tauri::command]
pub async fn rebase(app: AppHandle, repo: String, onto: String) -> CmdResult<Outcome> {
    blocking(&app, move |_, s| {
        let label = format!("Rebase onto {}", short(&onto));
        s.journaled(&repo, label, UndoStyle::Keep, |git, path| {
            ronin_git::rebase(git, path, &onto)
        })
    })
    .await
}

#[tauri::command]
pub async fn cherry_pick(app: AppHandle, repo: String, oid: String) -> CmdResult<Outcome> {
    blocking(&app, move |_, s| {
        let label = format!("Cherry-pick {}", short(&oid));
        s.journaled(&repo, label, UndoStyle::Keep, |git, path| {
            ronin_git::cherry_pick(git, path, &oid)
        })
    })
    .await
}

#[tauri::command]
pub async fn revert(app: AppHandle, repo: String, oid: String) -> CmdResult<Outcome> {
    blocking(&app, move |_, s| {
        let label = format!("Revert {}", short(&oid));
        s.journaled(&repo, label, UndoStyle::Keep, |git, path| {
            ronin_git::revert(git, path, &oid)
        })
    })
    .await
}

/// Moves the current branch to `rev`.
#[tauri::command]
pub async fn reset(app: AppHandle, repo: String, rev: String, mode: ResetMode) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let (style, kind) = match mode {
            ResetMode::Soft => (UndoStyle::Soft, "Soft"),
            ResetMode::Mixed => (UndoStyle::Mixed, "Mixed"),
            // Undo restores the commits; the discarded changes are gone.
            ResetMode::Hard => (UndoStyle::Keep, "Hard"),
        };
        let label = format!("{kind} reset to {}", short(&rev));
        s.journaled(&repo, label, style, |git, path| {
            ronin_git::reset(git, path, &rev, mode)
        })
    })
    .await
}

/// Continues, skips or aborts the merge, rebase, … in progress.
#[tauri::command]
pub async fn resolve_operation(
    app: AppHandle,
    repo: String,
    action: OperationAction,
) -> CmdResult<Outcome> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        let _serialised = lock(&session.journal);
        let result = ronin_git::resolve_operation(s.git()?, &session.path, action);
        s.refs_moved(&repo);
        result.map_err(err)
    })
    .await
}

/// The message prepared for the commit that concludes a stopped merge,
/// cherry-pick or revert.
#[tauri::command]
pub async fn pending_message(app: AppHandle, repo: String) -> CmdResult<Option<String>> {
    blocking(&app, move |_, s| {
        ronin_git::pending_message(&s.session(&repo)?.path).map_err(err)
    })
    .await
}

/// Answers a `credential-request`; `None` cancels it.
#[tauri::command]
pub fn credential_respond(state: tauri::State<'_, AppState>, id: u64, answer: Option<String>) {
    if let Some(askpass) = &state.askpass {
        askpass.respond(id, answer);
    }
}

/// Opens (or reuses) the session for `path` in a tab and records it as recent.
fn open_tab(app: &AppHandle, s: &AppState, path: &Path) -> CmdResult<RepoInfo> {
    let info = start_session(app, s, path)?;
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
}

/// Opens (or reuses) the session for the repository containing `path`.
fn start_session(app: &AppHandle, state: &AppState, path: &Path) -> CmdResult<RepoInfo> {
    let info = ronin_git::open_repo(path).map_err(err)?;
    let mut repos = lock(&state.repos);
    if !repos.contains_key(&info.path) {
        let (git_dir, common_dir) = ronin_git::git_dirs(path).map_err(err)?;
        let workdir = (!info.is_bare).then(|| PathBuf::from(&info.path));
        let session = Arc::new_cyclic(|weak: &std::sync::Weak<RepoSession>| {
            let weak = weak.clone();
            let app = app.clone();
            let repo = info.path.clone();
            let on_change = move |changes: watcher::Changes| {
                if changes.refs {
                    if let Some(session) = weak.upgrade() {
                        *lock(&session.graph) = None;
                    }
                    let _ = app.emit(REPO_CHANGED, &repo);
                }
                if changes.worktree {
                    let _ = app.emit(WORKTREE_CHANGED, &repo);
                }
            };
            let watcher = watcher::watch(&git_dir, &common_dir, workdir.as_deref(), on_change).ok();
            RepoSession {
                path: PathBuf::from(&info.path),
                graph: Mutex::new(None),
                journal: Mutex::new(Default::default()),
                _watcher: watcher,
            }
        });
        repos.insert(info.path.clone(), session);
    }
    Ok(info)
}
