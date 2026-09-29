use std::path::Path;

use serde::Deserialize;
use ts_rs::TS;

use crate::error::gix_err;
use crate::repo::discover;
use crate::{Error, GitCli, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitOptions {
    /// Replace the HEAD commit instead of adding one.
    pub amend: bool,
    /// Add a `Signed-off-by` trailer.
    pub signoff: bool,
}

/// Commits the index with `message` and returns the new HEAD. Runs through
/// the git CLI, so hooks and GPG/SSH signing apply as configured.
pub fn commit(git: &GitCli, path: &Path, message: &str, options: CommitOptions) -> Result<String> {
    if message.trim().is_empty() {
        return Err(Error::Invalid("the commit message is empty".into()));
    }
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    let mut args = vec!["commit", "--quiet", "--file=-"];
    if options.amend {
        args.push("--amend");
    }
    if options.signoff {
        args.push("--signoff");
    }
    git.run_with_input(workdir, args, message.as_bytes())?;
    Ok(git.run(workdir, ["rev-parse", "HEAD"])?.trim().to_owned())
}

/// The message of the HEAD commit, or `None` before the first commit.
pub fn head_message(path: &Path) -> Result<Option<String>> {
    let repo = discover(path)?;
    let Ok(commit) = repo.head_commit() else {
        return Ok(None);
    };
    let message = commit.message_raw().map_err(gix_err)?;
    Ok(Some(message.to_string()))
}
