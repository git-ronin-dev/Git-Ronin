//! Reviewing pull requests locally: fetching their commits, listing what
//! they change, and remembering which issue a branch is for.

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::cli::operand;
use crate::detail::parse_diff_tree;
use crate::repo::workdir;
use crate::{Error, FileChange, GitCli, Progress, Result};

/// What a pull request changes: from where its branch left the target to
/// its head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RangeDiff {
    /// The merge base, to diff files against.
    pub base: String,
    pub head: String,
    pub files: Vec<FileChange>,
}

/// Whether commit `oid` is in the repository.
pub fn has_commit(git: &GitCli, path: &Path, oid: &str) -> Result<bool> {
    let oid = operand(oid)?;
    let finished = git.run_raw(
        &workdir(path)?,
        ["cat-file", "-e", &format!("{oid}^{{commit}}")],
    )?;
    Ok(finished.success())
}

/// Fetches `refspecs` from `source` (a remote name or URL) unless every
/// commit in `oids` is already here. Nothing is written but `FETCH_HEAD`.
pub fn fetch_commits(
    git: &GitCli,
    path: &Path,
    source: &str,
    refspecs: &[&str],
    oids: &[&str],
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let mut missing = false;
    for oid in oids {
        missing |= !has_commit(git, path, oid)?;
    }
    if !missing && !oids.is_empty() {
        return Ok(());
    }
    let mut args = vec!["fetch", "--progress", "--no-tags"];
    args.push(operand(source)?);
    for refspec in refspecs {
        // Only plain refs: no destination, no forced updates.
        if refspec.contains([':', '+', '*']) {
            return Err(Error::Invalid(format!("not a plain ref: {refspec}")));
        }
        args.push(operand(refspec)?);
    }
    git.run_streaming(&workdir(path)?, args, on_progress)?
        .check(&[0])?;
    for oid in oids {
        if !has_commit(git, path, oid)? {
            return Err(Error::Invalid(format!(
                "commit {} is not on the remote; fetch the repository and try again",
                &oid[..oid.len().min(10)]
            )));
        }
    }
    Ok(())
}

/// Files changed on `head` since it left `base` (as `git diff base...head`).
pub fn range_files(git: &GitCli, path: &Path, base: &str, head: &str) -> Result<RangeDiff> {
    let workdir = workdir(path)?;
    let (base, head) = (operand(base)?, operand(head)?);
    let merge_base = git
        .run(&workdir, ["merge-base", base, head])?
        .trim()
        .to_owned();
    let raw = git.run(
        &workdir,
        [
            "diff-tree",
            "-r",
            "-z",
            "-M",
            "--raw",
            "--numstat",
            "--no-commit-id",
            &merge_base,
            head,
        ],
    )?;
    let head = git
        .run(
            &workdir,
            ["rev-parse", "--verify", &format!("{head}^{{commit}}")],
        )?
        .trim()
        .to_owned();
    Ok(RangeDiff {
        base: merge_base,
        head,
        files: parse_diff_tree(&raw),
    })
}

fn issue_key(branch: &str) -> Result<String> {
    Ok(format!("branch.{}.roninIssue", operand(branch)?))
}

/// The issue branch `branch` was started for (`#12`, `PROJ-12`), if any.
pub fn branch_issue(git: &GitCli, path: &Path, branch: &str) -> Result<Option<String>> {
    let value = git.run_accepting(
        &workdir(path)?,
        ["config", "--local", "--get", &issue_key(branch)?],
        &[0, 1],
    )?;
    let value = value.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

/// Records (or with `None` forgets) the issue `branch` is for. git moves
/// it along when the branch is renamed and drops it when it's deleted.
pub fn set_branch_issue(
    git: &GitCli,
    path: &Path,
    branch: &str,
    issue: Option<&str>,
) -> Result<()> {
    let workdir = workdir(path)?;
    let key = issue_key(branch)?;
    match issue {
        Some(issue) => git.run(&workdir, ["config", "--local", &key, issue])?,
        None => git.run_accepting(&workdir, ["config", "--local", "--unset", &key], &[0, 5])?,
    };
    Ok(())
}
