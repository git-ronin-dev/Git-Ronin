//! Operations that rewrite the current branch: merge, rebase, cherry-pick,
//! revert and reset, plus continuing or aborting one that stopped.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cli::{Finished, operand};
use crate::error::gix_err;
use crate::repo::{discover, operation, workdir};
use crate::{Error, GitCli, Operation, Result};

/// How an operation that may stop on conflicts ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Outcome {
    Done,
    /// Stopped on conflicts; the repository reports the operation in progress.
    Conflicts,
    /// Stopped without conflicts, e.g. at a commit an interactive rebase
    /// marked for editing, or because a command in it failed.
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ResetMode {
    /// Move the branch only; changes stay staged.
    Soft,
    /// Move the branch and reset the index; changes stay in the working tree.
    Mixed,
    /// Move the branch and discard every uncommitted change.
    Hard,
}

/// Merges `rev` into the current branch, fast-forwarding when possible
/// unless `no_ff`.
pub fn merge(git: &GitCli, path: &Path, rev: &str, no_ff: bool) -> Result<Outcome> {
    let rev = operand(rev)?;
    let mut args = vec!["merge", "--no-edit"];
    if no_ff {
        args.push("--no-ff");
    }
    args.push(rev);
    finish(path, git.run_raw(&workdir(path)?, args)?)
}

/// Rebases the current branch onto `onto`. Uncommitted changes are stashed
/// first and restored afterwards.
pub fn rebase(git: &GitCli, path: &Path, onto: &str) -> Result<Outcome> {
    let onto = operand(onto)?;
    let args = ["rebase", "--autostash", onto];
    finish(path, git.run_raw(&workdir(path)?, args)?)
}

/// Applies the changes of commit `oid` on top of the current branch.
pub fn cherry_pick(git: &GitCli, path: &Path, oid: &str) -> Result<Outcome> {
    pick(git, path, "cherry-pick", oid)
}

/// Adds a commit undoing commit `oid`.
pub fn revert(git: &GitCli, path: &Path, oid: &str) -> Result<Outcome> {
    pick(git, path, "revert", oid)
}

/// Moves the current branch (or detached HEAD) to `rev`.
pub fn reset(git: &GitCli, path: &Path, rev: &str, mode: ResetMode) -> Result<()> {
    let flag = match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
    };
    git.run(&workdir(path)?, ["reset", "--quiet", flag, operand(rev)?])?;
    Ok(())
}

/// What to do with an operation that stopped part-way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OperationAction {
    /// Carry on after the conflicts are resolved and staged.
    Continue,
    /// Leave out the commit that stopped it (not for merges).
    Skip,
    /// Go back to where it started.
    Abort,
}

/// Continues, skips or aborts the operation in progress.
pub fn resolve_operation(git: &GitCli, path: &Path, action: OperationAction) -> Result<Outcome> {
    let repo = discover(path)?;
    let Some(op) = operation(&repo) else {
        return Err(Error::Invalid("no operation is in progress".into()));
    };
    let command = match op {
        Operation::Merge => "merge",
        Operation::Rebase => "rebase",
        Operation::CherryPick => "cherry-pick",
        Operation::Revert => "revert",
        Operation::ApplyMailbox => "am",
        Operation::Bisect if action == OperationAction::Abort => {
            git.run(&workdir(path)?, ["bisect", "reset"])?;
            return Ok(Outcome::Done);
        }
        Operation::Bisect => return Err(Error::Invalid("bisect can only be ended".into())),
    };
    let flag = match action {
        OperationAction::Continue => "--continue",
        OperationAction::Abort => "--abort",
        OperationAction::Skip if op == Operation::Merge => {
            return Err(Error::Invalid("a merge cannot be skipped".into()));
        }
        OperationAction::Skip => "--skip",
    };
    finish(path, git.run_raw(&workdir(path)?, [command, flag])?)
}

/// The message git prepared for the commit that concludes a stopped merge,
/// cherry-pick or revert, with its comment lines removed.
pub fn pending_message(path: &Path) -> Result<Option<String>> {
    let repo = discover(path)?;
    let file = repo.git_dir().join("MERGE_MSG");
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let message: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
    let message = message.join("\n").trim().to_owned();
    Ok(Some(message).filter(|m| !m.is_empty()))
}

fn pick(git: &GitCli, path: &Path, command: &str, oid: &str) -> Result<Outcome> {
    let oid = operand(oid)?;
    let repo = discover(path)?;
    let id = repo
        .rev_parse_single(oid)
        .map_err(gix_err)?
        .object()
        .map_err(gix_err)?
        .peel_to_commit()
        .map_err(gix_err)?;
    let mut args = vec![command];
    if command == "revert" {
        args.push("--no-edit");
    }
    // A merge commit is taken relative to its first parent.
    if id.parent_ids().count() > 1 {
        args.extend(["--mainline", "1"]);
    }
    args.push(oid);
    finish(path, git.run_raw(&workdir(path)?, args)?)
}

/// Success, a stop (on conflicts or otherwise), or an error when git failed
/// without leaving an operation in progress.
pub(crate) fn finish(path: &Path, finished: Finished) -> Result<Outcome> {
    let repo = discover(path)?;
    if operation(&repo).is_none() {
        return if finished.success() {
            Ok(Outcome::Done)
        } else {
            Err(finished.into_error())
        };
    }
    let index = repo.index_or_empty().map_err(gix_err)?;
    let conflicts = index.entries().iter().any(|e| e.stage_raw() != 0);
    Ok(if conflicts {
        Outcome::Conflicts
    } else {
        Outcome::Stopped
    })
}
