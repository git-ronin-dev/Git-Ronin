//! Local branches: create, check out, rename, delete, move and track.

use std::path::Path;

use crate::cli::operand;
use crate::repo::workdir;
use crate::{Error, GitCli, Result};

/// Creates branch `name` at `start` (a commit, branch or tag), checking it
/// out if `checkout`. Starting from a remote branch sets it as upstream, as
/// `git branch` does by default.
pub fn create_branch(
    git: &GitCli,
    path: &Path,
    name: &str,
    start: &str,
    checkout: bool,
) -> Result<()> {
    let workdir = workdir(path)?;
    check_branch_name(git, &workdir, name)?;
    let start = operand(start)?;
    if checkout {
        git.run(&workdir, ["switch", "--quiet", "--create", name, start])?;
    } else {
        git.run(&workdir, ["branch", "--quiet", name, start])?;
    }
    Ok(())
}

/// Checks out local branch `name`. Uncommitted changes are carried over
/// unless they conflict, in which case git refuses and nothing changes.
pub fn checkout_branch(git: &GitCli, path: &Path, name: &str) -> Result<()> {
    let name = operand(name)?;
    git.run(&workdir(path)?, ["switch", "--quiet", "--no-guess", name])?;
    Ok(())
}

/// Creates local branch `name` tracking `remote_ref` (a full
/// `refs/remotes/…` name) and checks it out.
pub fn checkout_remote_branch(
    git: &GitCli,
    path: &Path,
    remote_ref: &str,
    name: &str,
) -> Result<()> {
    let workdir = workdir(path)?;
    check_branch_name(git, &workdir, name)?;
    if !remote_ref.starts_with("refs/remotes/") {
        return Err(Error::Invalid(format!("not a remote branch: {remote_ref}")));
    }
    git.run(
        &workdir,
        ["switch", "--quiet", "--create", name, "--track", remote_ref],
    )?;
    Ok(())
}

/// Checks out `rev` without a branch (detached HEAD).
pub fn checkout_detached(git: &GitCli, path: &Path, rev: &str) -> Result<()> {
    let rev = operand(rev)?;
    git.run(&workdir(path)?, ["switch", "--quiet", "--detach", rev])?;
    Ok(())
}

pub fn rename_branch(git: &GitCli, path: &Path, old: &str, new: &str) -> Result<()> {
    let workdir = workdir(path)?;
    let old = operand(old)?;
    check_branch_name(git, &workdir, new)?;
    git.run(&workdir, ["branch", "--move", old, new])?;
    Ok(())
}

/// Deletes local branch `name`. Without `force`, git refuses to delete a
/// branch whose commits are not merged anywhere; that is reported as
/// [`Error::NotMerged`].
pub fn delete_branch(git: &GitCli, path: &Path, name: &str, force: bool) -> Result<()> {
    let name = operand(name)?;
    let flag = if force { "-D" } else { "-d" };
    let finished = git.run_raw(&workdir(path)?, ["branch", flag, name])?;
    if !finished.success() && finished.stderr.contains("is not fully merged") {
        return Err(Error::NotMerged(name.to_owned()));
    }
    finished.check(&[0])?;
    Ok(())
}

/// Points branch `name`, which must not be checked out, at `rev`.
pub fn move_branch(git: &GitCli, path: &Path, name: &str, rev: &str) -> Result<()> {
    let (name, rev) = (operand(name)?, operand(rev)?);
    git.run(&workdir(path)?, ["branch", "--force", name, rev])?;
    Ok(())
}

/// Sets (or with `None`, removes) the upstream of branch `name`.
/// `upstream` is a branch such as `origin/main`.
pub fn set_upstream(git: &GitCli, path: &Path, name: &str, upstream: Option<&str>) -> Result<()> {
    let workdir = workdir(path)?;
    let name = operand(name)?;
    match upstream {
        Some(upstream) => {
            let flag = format!("--set-upstream-to={}", operand(upstream)?);
            git.run(&workdir, ["branch", "--quiet", &flag, name])?
        }
        None => git.run(&workdir, ["branch", "--unset-upstream", name])?,
    };
    Ok(())
}

fn check_branch_name(git: &GitCli, workdir: &Path, name: &str) -> Result<()> {
    let invalid = || Error::Invalid(format!("“{name}” is not a valid branch name"));
    let name = operand(name).map_err(|_| invalid())?;
    let finished = git.run_raw(workdir, ["check-ref-format", "--branch", name])?;
    if finished.success() {
        Ok(())
    } else {
        Err(invalid())
    }
}
