use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::gix_err;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoInfo {
    /// Working tree root, or the git dir for bare repositories.
    pub path: String,
    pub name: String,
    pub is_bare: bool,
    pub head: HeadState,
    /// A merge, rebase, … that stopped part-way, e.g. on conflicts.
    pub operation: Option<Operation>,
    /// Where a stopped rebase is.
    pub rebase: Option<RebaseProgress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebaseProgress {
    /// The branch being rebased; `None` for a detached HEAD.
    pub branch: Option<String>,
    /// The step it stopped at (1-based), of `total`.
    pub step: u32,
    pub total: u32,
    /// The commit it stopped at, abbreviated as git recorded it.
    pub stopped_at: Option<String>,
    /// Stopped at an `edit` step so the commit can be amended.
    pub editing: bool,
}

/// A multi-step git operation waiting for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    /// `git am`, applying patches from a mailbox.
    ApplyMailbox,
    Bisect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum HeadState {
    /// On a branch; `unborn` when the branch has no commits yet.
    Branch {
        name: String,
        unborn: bool,
    },
    Detached {
        oid: String,
    },
}

/// Finds the repository containing `path`, which may be any subdirectory.
pub(crate) fn discover(path: &Path) -> Result<gix::Repository> {
    gix::discover(path).map_err(|_| Error::NotARepo(path.to_owned()))
}

/// The working tree root of the repository containing `path`.
pub(crate) fn workdir(path: &Path) -> Result<PathBuf> {
    let repo = discover(path)?;
    repo.workdir()
        .map(Path::to_owned)
        .ok_or_else(|| Error::Bare(path.to_owned()))
}

/// The operation in progress in `repo`, if any.
pub(crate) fn operation(repo: &gix::Repository) -> Option<Operation> {
    use gix::state::InProgress as P;
    Some(match repo.state()? {
        P::Merge => Operation::Merge,
        P::Rebase | P::RebaseInteractive | P::ApplyMailboxRebase => Operation::Rebase,
        P::CherryPick | P::CherryPickSequence => Operation::CherryPick,
        P::Revert | P::RevertSequence => Operation::Revert,
        P::ApplyMailbox => Operation::ApplyMailbox,
        P::Bisect => Operation::Bisect,
    })
}

/// The repository's git dir and common dir. They differ for linked worktrees,
/// whose HEAD lives in the former and shared refs in the latter.
pub fn git_dirs(path: &Path) -> Result<(PathBuf, PathBuf)> {
    let repo = discover(path)?;
    Ok((repo.git_dir().to_owned(), repo.common_dir().to_owned()))
}

/// Opens the repository containing `path` (which may be any subdirectory).
pub fn open_repo(path: &Path) -> Result<RepoInfo> {
    let repo = discover(path)?;
    let root = repo.workdir().unwrap_or_else(|| repo.git_dir()).to_owned();
    let head = match repo.head().map_err(gix_err)?.kind {
        gix::head::Kind::Symbolic(reference) => HeadState::Branch {
            name: reference.name.shorten().to_string(),
            unborn: false,
        },
        gix::head::Kind::Unborn(name) => HeadState::Branch {
            name: name.shorten().to_string(),
            unborn: true,
        },
        gix::head::Kind::Detached { target, .. } => HeadState::Detached {
            oid: target.to_string(),
        },
    };
    Ok(RepoInfo {
        name: root
            .file_name()
            .map_or_else(|| root.to_string_lossy(), |n| n.to_string_lossy())
            .into_owned(),
        path: root.to_string_lossy().into_owned(),
        is_bare: repo.is_bare(),
        operation: operation(&repo),
        rebase: rebase_progress(repo.git_dir()),
        head,
    })
}

/// Reads the state files of a rebase in progress, if any.
fn rebase_progress(git_dir: &Path) -> Option<RebaseProgress> {
    let (dir, step, total) = [
        ("rebase-merge", "msgnum", "end"),
        ("rebase-apply", "next", "last"),
    ]
    .into_iter()
    .find(|(dir, ..)| git_dir.join(dir).is_dir())?;
    let dir = git_dir.join(dir);
    let read = |name: &str| {
        std::fs::read_to_string(dir.join(name))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    let number = |name: &str| read(name).and_then(|n| n.parse().ok()).unwrap_or(0);
    Some(RebaseProgress {
        branch: read("head-name").and_then(|n| n.strip_prefix("refs/heads/").map(str::to_owned)),
        step: number(step),
        total: number(total),
        stopped_at: read("stopped-sha"),
        editing: dir.join("amend").is_file(),
    })
}
