//! Linked worktrees: more working trees sharing one repository.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::cli::operand;
use crate::repo::discover;
use crate::{Error, GitCli, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Worktree {
    pub path: String,
    /// `None` before the first commit.
    pub head: Option<String>,
    /// The checked-out branch (short name); `None` when detached.
    pub branch: Option<String>,
    /// The repository's main working tree (or the bare repository itself).
    pub is_main: bool,
    /// The working tree this repository was opened from.
    pub is_current: bool,
    pub bare: bool,
    /// Why it is locked, if it is (possibly an empty reason).
    pub locked: Option<String>,
    /// Its directory is gone; `prune` would forget it.
    pub prunable: bool,
}

/// Lists the repository's working trees, the main one first.
pub fn list_worktrees(git: &GitCli, path: &Path) -> Result<Vec<Worktree>> {
    let repo = discover(path)?;
    let here = repo.workdir().unwrap_or(repo.git_dir());
    let here = canonical(here);
    let out = git.run(here.as_path(), ["worktree", "list", "--porcelain"])?;
    let mut worktrees = parse_porcelain(&out);
    for wt in &mut worktrees {
        wt.is_current = canonical(Path::new(&wt.path)) == here;
    }
    Ok(worktrees)
}

/// Creates a working tree at `dest` with `branch` checked out. With
/// `create`, `branch` is a new branch starting at `start` (HEAD when
/// `None`); otherwise an existing branch not checked out elsewhere.
pub fn add_worktree(
    git: &GitCli,
    path: &Path,
    dest: &Path,
    branch: &str,
    create: bool,
    start: Option<&str>,
) -> Result<()> {
    let repo = discover(path)?;
    let cwd = repo.workdir().unwrap_or(repo.git_dir()).to_owned();
    let branch = operand(branch)?;
    if dest.exists() && std::fs::read_dir(dest)?.next().is_some() {
        return Err(Error::Invalid(format!("{} is not empty", dest.display())));
    }
    let dest = dest.as_os_str().to_owned();
    let mut args: Vec<std::ffi::OsString> = vec!["worktree".into(), "add".into(), "--quiet".into()];
    if create {
        let finished = git.run_raw(&cwd, ["check-ref-format", "--branch", branch])?;
        if !finished.success() {
            return Err(Error::Invalid(format!(
                "“{branch}” is not a valid branch name"
            )));
        }
        args.extend(["-b".into(), branch.into(), "--".into(), dest]);
        args.extend(start.map(|s| operand(s).map(Into::into)).transpose()?);
    } else {
        args.extend(["--".into(), dest, branch.into()]);
    }
    git.run(&cwd, &args)?;
    Ok(())
}

/// Deletes working tree `wt_path`. Without `force`, git refuses when it has
/// uncommitted changes or untracked files.
pub fn remove_worktree(git: &GitCli, path: &Path, wt_path: &str, force: bool) -> Result<()> {
    let repo = discover(path)?;
    let cwd = repo.workdir().unwrap_or(repo.git_dir()).to_owned();
    if canonical(Path::new(wt_path)) == canonical(&cwd) {
        return Err(Error::Invalid(
            "can't remove the working tree it was opened from".into(),
        ));
    }
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.extend(["--", wt_path]);
    git.run(&cwd, args)?;
    Ok(())
}

/// Forgets working trees whose directories were deleted.
pub fn prune_worktrees(git: &GitCli, path: &Path) -> Result<()> {
    let repo = discover(path)?;
    git.run(
        repo.workdir().unwrap_or(repo.git_dir()),
        ["worktree", "prune"],
    )?;
    Ok(())
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}

/// Parses `git worktree list --porcelain`: blank-line separated records of
/// `worktree <path>`, `HEAD <oid>`, `branch <ref>`, `detached`, `bare`,
/// `locked [<reason>]` and `prunable [<reason>]`.
fn parse_porcelain(output: &str) -> Vec<Worktree> {
    let mut worktrees: Vec<Worktree> = Vec::new();
    for line in output.lines() {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key == "worktree" {
            worktrees.push(Worktree {
                path: value.to_owned(),
                head: None,
                branch: None,
                is_main: worktrees.is_empty(),
                is_current: false,
                bare: false,
                locked: None,
                prunable: false,
            });
            continue;
        }
        let Some(wt) = worktrees.last_mut() else {
            continue;
        };
        match key {
            "HEAD" => wt.head = Some(value.to_owned()).filter(|h| h.bytes().any(|b| b != b'0')),
            "branch" => {
                wt.branch = Some(
                    value
                        .strip_prefix("refs/heads/")
                        .unwrap_or(value)
                        .to_owned(),
                )
            }
            "bare" => wt.bare = true,
            "locked" => wt.locked = Some(value.to_owned()),
            "prunable" => wt.prunable = true,
            _ => {}
        }
    }
    worktrees
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_worktree_list() {
        let out = "worktree /r\nHEAD 1111\nbranch refs/heads/main\n\n\
                   worktree /r-feature\nHEAD 2222\nbranch refs/heads/feature/x\n\n\
                   worktree /mnt/usb/r\nHEAD 3333\ndetached\nlocked on usb\n\n\
                   worktree /gone\nHEAD 0000\ndetached\nprunable gitdir file points to non-existent location\n";
        let wts = parse_porcelain(out);
        assert_eq!(wts.len(), 4);
        assert!(wts[0].is_main && !wts[1].is_main);
        assert_eq!(wts[1].branch.as_deref(), Some("feature/x"));
        assert_eq!(wts[2].branch, None);
        assert_eq!(wts[2].locked.as_deref(), Some("on usb"));
        assert!(wts[3].prunable);
        assert_eq!(wts[3].head, None);
    }
}
