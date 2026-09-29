use std::path::Path;

use serde::Deserialize;
use ts_rs::TS;

use crate::repo::workdir;
use crate::{Error, GitCli, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StashOptions {
    pub message: Option<String>,
    pub include_untracked: bool,
    /// Leave staged changes in place as well as stashing them.
    pub keep_index: bool,
}

/// Stashes uncommitted changes. Returns false if there was nothing to stash.
pub fn stash_push(git: &GitCli, path: &Path, options: &StashOptions) -> Result<bool> {
    let workdir = workdir(path)?;
    let before = stash_top(git, &workdir)?;
    let mut args = vec!["stash", "push", "--quiet"];
    if options.include_untracked {
        args.push("--include-untracked");
    }
    if options.keep_index {
        args.push("--keep-index");
    }
    let message = options.message.as_deref().map(str::trim).unwrap_or("");
    if !message.is_empty() {
        args.extend(["--message", message]);
    }
    git.run(&workdir, args)?;
    Ok(stash_top(git, &workdir)? != before)
}

/// Applies stash `index` to the working tree, dropping it if `pop`.
/// `oid` guards against the stash list having changed in the meantime.
pub fn stash_apply(git: &GitCli, path: &Path, index: u32, oid: &str, pop: bool) -> Result<()> {
    let workdir = workdir(path)?;
    let name = checked_stash(git, &workdir, index, oid)?;
    let action = if pop { "pop" } else { "apply" };
    git.run(&workdir, ["stash", action, "--quiet", &name])?;
    Ok(())
}

pub fn stash_drop(git: &GitCli, path: &Path, index: u32, oid: &str) -> Result<()> {
    let workdir = workdir(path)?;
    let name = checked_stash(git, &workdir, index, oid)?;
    git.run(&workdir, ["stash", "drop", "--quiet", &name])?;
    Ok(())
}

fn stash_top(git: &GitCli, workdir: &Path) -> Result<Option<String>> {
    // Exits with 1 when there is no stash.
    let out = git.run_accepting(
        workdir,
        ["rev-parse", "-q", "--verify", "refs/stash"],
        &[0, 1],
    )?;
    Ok(Some(out.trim().to_owned()).filter(|s| !s.is_empty()))
}

/// `stash@{index}`, if it is still the stash the user picked.
fn checked_stash(git: &GitCli, workdir: &Path, index: u32, oid: &str) -> Result<String> {
    let name = format!("stash@{{{index}}}");
    let out = git.run_accepting(workdir, ["rev-parse", "-q", "--verify", &name], &[0, 1])?;
    if out.trim() != oid {
        return Err(Error::Invalid(
            "the stash list changed; refresh and try again".into(),
        ));
    }
    Ok(name)
}
