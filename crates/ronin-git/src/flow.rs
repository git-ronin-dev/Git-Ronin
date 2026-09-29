//! Git Flow, done natively with the git-flow (AVH) configuration keys, so
//! repositories set up with the `git flow` tool work as they are.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::repo::workdir;
use crate::{
    Error, GitCli, Outcome, Result, checkout_branch, create_branch, create_tag, delete_branch,
    merge,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FlowConfig {
    /// The production branch.
    pub master: String,
    /// The integration branch.
    pub develop: String,
    pub feature_prefix: String,
    pub release_prefix: String,
    pub hotfix_prefix: String,
    /// Put in front of release and hotfix names to make their tags.
    pub version_tag_prefix: String,
}

impl FlowConfig {
    pub fn prefix(&self, kind: FlowKind) -> &str {
        match kind {
            FlowKind::Feature => &self.feature_prefix,
            FlowKind::Release => &self.release_prefix,
            FlowKind::Hotfix => &self.hotfix_prefix,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FlowKind {
    Feature,
    Release,
    Hotfix,
}

const KEYS: [&str; 6] = [
    "gitflow.branch.master",
    "gitflow.branch.develop",
    "gitflow.prefix.feature",
    "gitflow.prefix.release",
    "gitflow.prefix.hotfix",
    "gitflow.prefix.versiontag",
];

/// The repository's Git Flow setup, or `None` if it has none.
pub fn flow_config(git: &GitCli, path: &Path) -> Result<Option<FlowConfig>> {
    // Exits with 1 when nothing matches.
    let out = git.run_accepting(
        &workdir(path)?,
        ["config", "--get-regexp", r"^gitflow\."],
        &[0, 1],
    )?;
    let get = |key: &str| {
        out.lines().find_map(|line| {
            let (k, v) = line.split_once(' ').unwrap_or((line, ""));
            (k == key).then(|| v.to_owned())
        })
    };
    let (Some(master), Some(develop)) = (get(KEYS[0]), get(KEYS[1])) else {
        return Ok(None);
    };
    Ok(Some(FlowConfig {
        master,
        develop,
        feature_prefix: get(KEYS[2]).unwrap_or_else(|| "feature/".into()),
        release_prefix: get(KEYS[3]).unwrap_or_else(|| "release/".into()),
        hotfix_prefix: get(KEYS[4]).unwrap_or_else(|| "hotfix/".into()),
        version_tag_prefix: get(KEYS[5]).unwrap_or_default(),
    }))
}

/// Saves the Git Flow setup, creating the develop branch from the master
/// branch if it doesn't exist yet.
pub fn flow_init(git: &GitCli, path: &Path, config: &FlowConfig) -> Result<()> {
    let workdir = workdir(path)?;
    let exists = |branch: &str| -> Result<bool> {
        let name = format!("refs/heads/{branch}");
        Ok(git
            .run_raw(&workdir, ["rev-parse", "--verify", "--quiet", &name])?
            .success())
    };
    if config.master.trim().is_empty() || config.develop.trim().is_empty() {
        return Err(Error::Invalid("name both branches".into()));
    }
    if config.master == config.develop {
        return Err(Error::Invalid(
            "the production and development branches must differ".into(),
        ));
    }
    if !exists(&config.master)? {
        return Err(Error::Invalid(format!(
            "there is no branch {}",
            config.master
        )));
    }
    if !exists(&config.develop)? {
        create_branch(git, path, &config.develop, &config.master, false)?;
    }
    let values = [
        &config.master,
        &config.develop,
        &config.feature_prefix,
        &config.release_prefix,
        &config.hotfix_prefix,
        &config.version_tag_prefix,
    ];
    for (key, value) in KEYS.iter().zip(values) {
        git.run(&workdir, ["config", "--local", key, value.trim()])?;
    }
    Ok(())
}

/// Starts a feature or release (from develop) or a hotfix (from master)
/// named `name`, and checks it out.
pub fn flow_start(git: &GitCli, path: &Path, kind: FlowKind, name: &str) -> Result<()> {
    let config = require(git, path)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("enter a name".into()));
    }
    let base = match kind {
        FlowKind::Hotfix => &config.master,
        FlowKind::Feature | FlowKind::Release => &config.develop,
    };
    let branch = format!("{}{name}", config.prefix(kind));
    create_branch(git, path, &branch, base, true)
}

/// Finishes `branch` (a full branch name with its prefix): a feature is
/// merged into develop; a release or hotfix into master, tagged, and then
/// into develop. The branch is deleted afterwards unless `keep`.
///
/// Stops (with [`Outcome::Conflicts`]) if a merge does; once that merge is
/// concluded, finishing again carries on where it stopped.
pub fn flow_finish(
    git: &GitCli,
    path: &Path,
    kind: FlowKind,
    branch: &str,
    tag_message: Option<&str>,
    keep: bool,
) -> Result<Outcome> {
    let config = require(git, path)?;
    let prefix = config.prefix(kind);
    let Some(name) = branch.strip_prefix(prefix).filter(|n| !n.is_empty()) else {
        return Err(Error::Invalid(format!(
            "{branch} doesn't start with {prefix}"
        )));
    };
    let merge_into = |target: &str| -> Result<Outcome> {
        checkout_branch(git, path, target)?;
        merge(git, path, branch, true)
    };
    if kind != FlowKind::Feature {
        let outcome = merge_into(&config.master)?;
        if outcome != Outcome::Done {
            return Ok(outcome);
        }
        let tag = format!("{}{name}", config.version_tag_prefix);
        let workdir = workdir(path)?;
        let tag_ref = format!("refs/tags/{tag}");
        let tagged = git
            .run_raw(&workdir, ["rev-parse", "--verify", "--quiet", &tag_ref])?
            .success();
        if !tagged {
            let default = match kind {
                FlowKind::Hotfix => format!("Hotfix {name}"),
                _ => format!("Release {name}"),
            };
            let message = tag_message.map(str::trim).filter(|m| !m.is_empty());
            create_tag(
                git,
                path,
                &tag,
                &config.master,
                Some(message.unwrap_or(&default)),
            )?;
        }
    }
    let outcome = merge_into(&config.develop)?;
    if outcome != Outcome::Done {
        return Ok(outcome);
    }
    if !keep {
        delete_branch(git, path, branch, false)?;
    }
    Ok(Outcome::Done)
}

fn require(git: &GitCli, path: &Path) -> Result<FlowConfig> {
    flow_config(git, path)?.ok_or_else(|| Error::Invalid("Git Flow is not set up here".into()))
}
