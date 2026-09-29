//! Bisect: finding the commit that introduced a problem by testing
//! commits halfway between a known good and a known bad one.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cli::operand;
use crate::repo::discover;
use crate::{Error, GitCli, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BisectMark {
    /// The problem is present (or whatever the "new" term is).
    Bad,
    Good,
    /// Can't be tested; try a commit nearby.
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BisectState {
    /// What git calls bad and good commits: "bad"/"good" unless the
    /// bisect was started with other terms.
    pub bad_term: String,
    pub good_term: String,
    pub bad: Option<String>,
    pub good: Vec<String>,
    pub skipped: Vec<String>,
    /// Commits that may still be the first bad one, the bad one included.
    /// Zero until both a bad and a good commit are known.
    pub remaining: u32,
    /// Found: the first bad commit.
    pub first_bad: Option<String>,
    /// Only skipped commits are left to test, so it can't narrow further.
    pub stuck: bool,
}

/// The bisect in progress, if any.
pub fn bisect_state(git: &GitCli, path: &Path) -> Result<Option<BisectState>> {
    let repo = discover(path)?;
    let git_dir = repo.git_dir();
    if !git_dir.join("BISECT_START").is_file() {
        return Ok(None);
    }
    let workdir = repo.workdir().unwrap_or(git_dir);
    let terms = std::fs::read_to_string(git_dir.join("BISECT_TERMS")).unwrap_or_default();
    let mut terms = terms.lines().map(str::trim);
    let bad_term = terms
        .next()
        .filter(|t| !t.is_empty())
        .unwrap_or("bad")
        .to_owned();
    let good_term = terms
        .next()
        .filter(|t| !t.is_empty())
        .unwrap_or("good")
        .to_owned();

    let out = git.run(
        workdir,
        [
            "for-each-ref",
            "--format=%(refname)%00%(objectname)",
            "refs/bisect",
        ],
    )?;
    let mut state = BisectState {
        bad: None,
        good: Vec::new(),
        skipped: Vec::new(),
        remaining: 0,
        first_bad: None,
        stuck: false,
        bad_term,
        good_term,
    };
    let good_prefix = format!("{}-", state.good_term);
    for line in out.lines() {
        let Some((name, oid)) = line.split_once('\0') else {
            continue;
        };
        let Some(name) = name.strip_prefix("refs/bisect/") else {
            continue;
        };
        if name == state.bad_term {
            state.bad = Some(oid.to_owned());
        } else if name.starts_with(&good_prefix) {
            state.good.push(oid.to_owned());
        } else if name.starts_with("skip-") {
            state.skipped.push(oid.to_owned());
        }
    }
    if let Some(bad) = state.bad.clone().filter(|_| !state.good.is_empty()) {
        let mut args = vec!["rev-list".to_owned(), bad.clone(), "--not".into()];
        args.extend(state.good.iter().cloned());
        let candidates: Vec<String> = git
            .run(workdir, &args)?
            .lines()
            .map(str::to_owned)
            .collect();
        state.remaining = candidates.len() as u32;
        if candidates.len() == 1 {
            state.first_bad = Some(bad.clone());
        } else {
            state.stuck = candidates
                .iter()
                .all(|c| *c == bad || state.skipped.contains(c));
        }
    }
    Ok(Some(state))
}

/// Marks `rev` good, bad or skipped, starting a bisect first if none is in
/// progress. Once both a good and a bad commit are known, git checks out
/// the next commit to test.
pub fn bisect_mark(git: &GitCli, path: &Path, mark: BisectMark, rev: &str) -> Result<()> {
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    let rev = operand(rev)?;
    let state = match bisect_state(git, path)? {
        Some(state) => state,
        None => {
            git.run(workdir, ["bisect", "start"])?;
            bisect_state(git, path)?.expect("a bisect was just started")
        }
    };
    let term = match mark {
        BisectMark::Bad => state.bad_term.as_str(),
        BisectMark::Good => state.good_term.as_str(),
        BisectMark::Skip => "skip",
    };
    let finished = git.run_raw(workdir, ["bisect", term, rev])?;
    // Running out of commits that aren't skipped is not a failure to mark.
    if !finished.success() && !finished.stdout.contains("only 'skip'ped commits") {
        return Err(finished.into_error());
    }
    Ok(())
}
