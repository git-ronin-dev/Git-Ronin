//! Remotes: their configuration and everything that talks to them.
//!
//! Network commands stream git's progress to a callback. Credentials come
//! from git's own helpers, or from `GIT_ASKPASS` if the caller set one on
//! the [`GitCli`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cli::operand;
use crate::history::finish;
use crate::repo::workdir;
use crate::{Error, GitCli, Outcome, Progress, Result};

pub fn add_remote(git: &GitCli, path: &Path, name: &str, url: &str) -> Result<()> {
    let (name, url) = (operand(name)?, operand(url.trim())?);
    git.run(&workdir(path)?, ["remote", "add", name, url])?;
    Ok(())
}

/// Renames remote `name` to `new_name` and sets its URLs. A `push_url` of
/// `None` makes it push to `url`.
pub fn edit_remote(
    git: &GitCli,
    path: &Path,
    name: &str,
    new_name: &str,
    url: &str,
    push_url: Option<&str>,
) -> Result<()> {
    let workdir = workdir(path)?;
    let (name, new_name) = (operand(name)?, operand(new_name)?);
    let url = operand(url.trim())?;
    if name != new_name {
        git.run(&workdir, ["remote", "rename", name, new_name])?;
    }
    git.run(&workdir, ["remote", "set-url", new_name, url])?;
    // Exits with 5 when there was no push URL to remove.
    let key = format!("remote.{new_name}.pushurl");
    git.run_accepting(&workdir, ["config", "--unset-all", &key], &[0, 5])?;
    if let Some(push_url) = push_url.map(str::trim).filter(|u| !u.is_empty()) {
        git.run(
            &workdir,
            ["remote", "set-url", "--push", new_name, operand(push_url)?],
        )?;
    }
    Ok(())
}

/// Removes a remote along with its remote-tracking branches.
pub fn remove_remote(git: &GitCli, path: &Path, name: &str) -> Result<()> {
    git.run(&workdir(path)?, ["remote", "remove", operand(name)?])?;
    Ok(())
}

/// Fetches `remote`, or every remote when `None`, pruning remote-tracking
/// branches that were deleted on the remote.
pub fn fetch(
    git: &GitCli,
    path: &Path,
    remote: Option<&str>,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let mut args = vec!["fetch", "--progress", "--prune"];
    match remote {
        Some(remote) => args.push(operand(remote)?),
        None => args.push("--all"),
    }
    git.run_streaming(&workdir(path)?, args, on_progress)?
        .check(&[0])?;
    Ok(())
}

/// How `pull` combines the upstream branch with the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PullMode {
    /// Whatever the user's `pull.rebase` / `pull.ff` config says; a merge
    /// if neither is set.
    Default,
    /// Only if the current branch has no commits of its own.
    FastForwardOnly,
    Merge,
    Rebase,
}

/// Pulls the upstream of the current branch. Uncommitted changes are
/// stashed first and restored afterwards.
pub fn pull(
    git: &GitCli,
    path: &Path,
    mode: PullMode,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<Outcome> {
    let workdir = workdir(path)?;
    let mut args = vec!["pull", "--progress", "--autostash"];
    match mode {
        PullMode::Default => {
            // Since git 2.33, a pull of diverged branches fails unless one
            // of these is set.
            let configured = ["pull.rebase", "pull.ff"].iter().any(|key| {
                git.run_raw(&workdir, ["config", "--get", key])
                    .is_ok_and(|f| f.success())
            });
            if !configured {
                args.push("--no-rebase");
            }
        }
        PullMode::FastForwardOnly => args.push("--ff-only"),
        PullMode::Merge => args.push("--no-rebase"),
        PullMode::Rebase => args.push("--rebase"),
    }
    finish(path, git.run_streaming(&workdir, args, on_progress)?)
}

/// What happened to a push.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PushOutcome {
    Pushed,
    /// The remote has commits the local branch lacks; pull first, or force.
    Rejected,
}

/// Where to push a branch that has no upstream yet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushTarget {
    pub remote: String,
    /// Branch name on the remote.
    pub branch: String,
}

/// Pushes local branch `name` to its upstream, or to `target` (which then
/// becomes its upstream).
///
/// `force_over` forces the push, but only while the remote branch is still
/// at that commit (`--force-with-lease=<branch>:<commit>`): pass the commit
/// the user saw and agreed to replace, so nothing they haven't seen is lost.
/// (`--force-if-includes` would express the same, but git can't check it
/// until a fetch has written a reflog for the remote-tracking branch, which
/// `git clone` doesn't.)
pub fn push_branch(
    git: &GitCli,
    path: &Path,
    name: &str,
    target: Option<&PushTarget>,
    force_over: Option<&str>,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<PushOutcome> {
    let workdir = workdir(path)?;
    let name = operand(name)?;
    let (remote, merge, set_upstream) = match target {
        Some(t) => (
            operand(&t.remote)?.to_owned(),
            format!("refs/heads/{}", operand(&t.branch)?),
            true,
        ),
        None => {
            let get = |key: String| -> Result<Option<String>> {
                let out = git.run_accepting(&workdir, ["config", "--get", &key], &[0, 1])?;
                Ok(Some(out.trim().to_owned()).filter(|s| !s.is_empty()))
            };
            let remote = get(format!("branch.{name}.remote"))?;
            let merge = get(format!("branch.{name}.merge"))?;
            match (remote, merge) {
                (Some(remote), Some(merge)) if remote != "." => (remote, merge, false),
                _ => return Err(Error::NoUpstream(name.to_owned())),
            }
        }
    };
    let lease = match force_over {
        Some(oid) if oid.len() >= 40 && oid.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Some(format!("--force-with-lease={merge}:{oid}"))
        }
        Some(oid) => return Err(Error::Invalid(format!("not a commit id: {oid}"))),
        None => None,
    };
    let mut args = vec!["push", "--progress", "--porcelain"];
    args.extend(lease.as_deref());
    if set_upstream {
        args.push("--set-upstream");
    }
    let refspec = format!("refs/heads/{name}:{merge}");
    args.extend([remote.as_str(), refspec.as_str()]);
    push(git, &workdir, &args, on_progress)
}

/// Pushes tag `name` to `remote`.
pub fn push_tag(
    git: &GitCli,
    path: &Path,
    remote: &str,
    name: &str,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<PushOutcome> {
    let refspec = format!("refs/tags/{0}:refs/tags/{0}", operand(name)?);
    let args = [
        "push",
        "--progress",
        "--porcelain",
        operand(remote)?,
        &refspec,
    ];
    push(git, &workdir(path)?, &args, on_progress)
}

/// Deletes `full_ref` (`refs/heads/…` or `refs/tags/…`) on `remote`.
pub fn delete_remote_ref(
    git: &GitCli,
    path: &Path,
    remote: &str,
    full_ref: &str,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    if !(full_ref.starts_with("refs/heads/") || full_ref.starts_with("refs/tags/")) {
        return Err(Error::Invalid(format!(
            "cannot delete {full_ref} on a remote"
        )));
    }
    let args = [
        "push",
        "--progress",
        "--porcelain",
        operand(remote)?,
        "--delete",
        full_ref,
    ];
    match push(git, &workdir(path)?, &args, on_progress)? {
        PushOutcome::Pushed => Ok(()),
        PushOutcome::Rejected => Err(Error::Invalid(format!(
            "the remote refused to delete {full_ref}"
        ))),
    }
}

fn push(
    git: &GitCli,
    workdir: &Path,
    args: &[&str],
    on_progress: &mut dyn FnMut(Progress),
) -> Result<PushOutcome> {
    let finished = git.run_streaming(workdir, args, on_progress)?;
    if finished.success() {
        return Ok(PushOutcome::Pushed);
    }
    // Porcelain lines are `<flag>\t<from>:<to>\t<summary>`; `!` is a failure.
    let failures: Vec<&str> = finished
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("!\t"))
        .filter_map(|l| l.split('\t').nth(1))
        .collect();
    if failures.iter().any(|f| f.contains("stale info")) {
        return Err(Error::Invalid(
            "the remote branch changed since it was last fetched; fetch, review the \
             new commits, then push again"
                .into(),
        ));
    }
    if !failures.is_empty()
        && failures
            .iter()
            .all(|f| f.contains("non-fast-forward") || f.contains("fetch first"))
    {
        return Ok(PushOutcome::Rejected);
    }
    Err(finished.into_error())
}

/// Clones `url` into a new directory `name` under `parent` and returns the
/// new working tree.
pub fn clone(
    git: &GitCli,
    url: &str,
    parent: &Path,
    name: &str,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf> {
    let url = operand(url.trim())?;
    let name = operand(name.trim())?;
    if name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err(Error::Invalid(format!("“{name}” is not a folder name")));
    }
    let dest = parent.join(name);
    if dest.exists() {
        return Err(Error::Invalid(format!("{} already exists", dest.display())));
    }
    git.run_streaming(
        parent,
        ["clone", "--progress", "--", url, name],
        on_progress,
    )?
    .check(&[0])?;
    Ok(dest)
}

/// Creates an empty repository at `path`, creating the directory if needed.
pub fn init(git: &GitCli, path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    git.run(path, ["init", "--quiet"])?;
    Ok(())
}

/// Suggests a folder name for a clone of `url`: its last path segment
/// without `.git`.
pub fn clone_name(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(['/', '\\']);
    let last = trimmed.rsplit(['/', '\\', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_clone_names() {
        let n = |u| clone_name(u);
        assert_eq!(
            n("https://github.com/owner/repo.git").as_deref(),
            Some("repo")
        );
        assert_eq!(n("git@github.com:owner/repo.git").as_deref(), Some("repo"));
        assert_eq!(n("git@host:repo").as_deref(), Some("repo"));
        assert_eq!(n("/srv/git/project/").as_deref(), Some("project"));
        assert_eq!(n(""), None);
    }
}
