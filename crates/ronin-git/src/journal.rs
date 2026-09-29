//! Undo and redo for actions taken in the app.
//!
//! Each action is bracketed by two [`Snapshot`]s of HEAD, the local branches
//! and the tags. Their difference is what the action did, and applying it
//! backwards (or forwards again) is undo (or redo). Before either runs, the
//! refs involved must still be where the action left them; if anything else
//! moved them, the history is dropped instead of guessed at.
//!
//! Undo never rewrites commits that were pushed after the action: once a
//! remote-tracking branch contains them, undo is blocked.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::error::gix_err;
use crate::repo::{discover, operation, workdir};
use crate::{Error, GitCli, Result};

/// Undo history is capped; older entries are forgotten.
const LIMIT: usize = 100;

/// Where HEAD points: a branch (full ref name) and/or a commit. A branch
/// without a commit is unborn.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HeadPos {
    branch: Option<String>,
    oid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RefValue {
    oid: String,
    /// Full name of a branch's upstream, restored with the branch.
    upstream: Option<String>,
}

/// HEAD, local branches and tags at one moment.
#[derive(Debug, Clone)]
pub struct Snapshot {
    head: HeadPos,
    refs: BTreeMap<String, RefValue>,
    in_progress: bool,
}

pub fn snapshot(git: &GitCli, path: &Path) -> Result<Snapshot> {
    let repo = discover(path)?;
    let head = HeadPos {
        branch: repo
            .head_name()
            .map_err(gix_err)?
            .map(|n| n.as_bstr().to_string()),
        oid: repo.head_id().ok().map(|id| id.to_string()),
    };
    let out = git.run(
        path,
        [
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(upstream)",
            "refs/heads",
            "refs/tags",
        ],
    )?;
    let refs = out
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let (name, oid, upstream) = (fields.next()?, fields.next()?, fields.next()?);
            Some((
                name.to_owned(),
                RefValue {
                    oid: oid.to_owned(),
                    upstream: Some(upstream.to_owned()).filter(|u| !u.is_empty()),
                },
            ))
        })
        .collect();
    Ok(Snapshot {
        head,
        refs,
        in_progress: operation(&repo).is_some(),
    })
}

/// How the action moved HEAD, which decides how undo moves it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoStyle {
    /// Commit, amend, soft reset: move the branch and keep the index.
    Soft,
    /// Mixed reset: move the branch and reset the index, not the files.
    Mixed,
    /// Merge, rebase, pull, cherry-pick, revert, hard reset: move the branch
    /// and update the files, refusing to overwrite uncommitted changes.
    Keep,
    /// Switching branches or detaching HEAD.
    Checkout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResetKind {
    Soft,
    Mixed,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeadMode {
    Checkout,
    Reset(ResetKind),
}

#[derive(Debug, Clone)]
enum Change {
    Head {
        before: HeadPos,
        after: HeadPos,
        mode: HeadMode,
    },
    Ref {
        name: String,
        before: Option<String>,
        after: Option<String>,
        /// A branch's upstream before and after the action, for recreating
        /// the branch in either direction.
        upstreams: (Option<String>, Option<String>),
    },
    /// Short branch names.
    Rename { before: String, after: String },
}

#[derive(Debug, Clone)]
struct Entry {
    label: String,
    changes: Vec<Change>,
    /// Commits the action added to HEAD that a remote already had.
    pushed: u32,
}

/// The next undo and redo steps, for the toolbar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JournalState {
    pub undo: Option<JournalStep>,
    pub redo: Option<JournalStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JournalStep {
    pub label: String,
    /// Why the step can't run, if it can't.
    pub blocked: Option<String>,
}

/// Undo and redo stacks for one repository.
#[derive(Debug, Default)]
pub struct Journal {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl Journal {
    /// Records what happened between `before` and `after`. Nothing is
    /// recorded if no ref moved, or if the action stopped part-way (e.g. a
    /// merge on conflicts): its end state isn't known yet.
    pub fn record(
        &mut self,
        git: &GitCli,
        path: &Path,
        label: impl Into<String>,
        before: &Snapshot,
        after: &Snapshot,
        style: UndoStyle,
    ) {
        if after.in_progress {
            self.clear();
            return;
        }
        let changes = diff(before, after, style);
        if changes.is_empty() {
            return;
        }
        let pushed = head_reset(&changes)
            .and_then(|(before, after)| pushed_count(git, path, after, before).ok())
            .unwrap_or(0);
        self.push(Entry {
            label: label.into(),
            changes,
            pushed,
        });
    }

    /// Records a branch rename; `before` and `after` are short names.
    pub fn record_rename(&mut self, label: impl Into<String>, before: &str, after: &str) {
        self.push(Entry {
            label: label.into(),
            changes: vec![Change::Rename {
                before: before.to_owned(),
                after: after.to_owned(),
            }],
            pushed: 0,
        });
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// The next steps. Stacks whose next step no longer matches the
    /// repository are dropped.
    pub fn state(&mut self, git: &GitCli, path: &Path) -> Result<JournalState> {
        if self.undo.is_empty() && self.redo.is_empty() {
            return Ok(JournalState::default());
        }
        let now = snapshot(git, path)?;
        if self.undo.last().is_some_and(|e| !matches(&now, e, true)) {
            self.undo.clear();
        }
        if self.redo.last().is_some_and(|e| !matches(&now, e, false)) {
            self.redo.clear();
        }
        let undo = match self.undo.last() {
            Some(entry) => Some(JournalStep {
                label: entry.label.clone(),
                blocked: blocked(git, path, entry)?,
            }),
            None => None,
        };
        let redo = self.redo.last().map(|entry| JournalStep {
            label: entry.label.clone(),
            blocked: None,
        });
        Ok(JournalState { undo, redo })
    }

    /// Undoes the last action and returns its label.
    pub fn undo(&mut self, git: &GitCli, path: &Path) -> Result<String> {
        let entry = self.next(git, path, true)?;
        if let Some(reason) = blocked(git, path, &entry)? {
            self.undo.push(entry);
            return Err(Error::Invalid(reason));
        }
        let workdir = workdir(path)?;
        for (i, change) in entry.changes.iter().rev().enumerate() {
            if let Err(e) = apply(git, &workdir, change, true) {
                self.failed(entry, i, true);
                return Err(e);
            }
        }
        let label = entry.label.clone();
        self.redo.push(entry);
        Ok(label)
    }

    /// Redoes the last undone action and returns its label.
    pub fn redo(&mut self, git: &GitCli, path: &Path) -> Result<String> {
        let entry = self.next(git, path, false)?;
        let workdir = workdir(path)?;
        for (i, change) in entry.changes.iter().enumerate() {
            if let Err(e) = apply(git, &workdir, change, false) {
                self.failed(entry, i, false);
                return Err(e);
            }
        }
        let label = entry.label.clone();
        self.undo.push(entry);
        Ok(label)
    }

    /// After step `step` of an undo (or redo) failed: if nothing had been
    /// applied yet (say, uncommitted changes were in the way), the entry can
    /// be retried; otherwise the repository is somewhere in between.
    fn failed(&mut self, entry: Entry, step: usize, undo: bool) {
        if step > 0 {
            self.clear();
        } else if undo {
            self.undo.push(entry);
        } else {
            self.redo.push(entry);
        }
    }

    fn push(&mut self, entry: Entry) {
        self.redo.clear();
        self.undo.push(entry);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// Pops the next undo (or redo) entry if the repository is still where
    /// that entry expects it.
    fn next(&mut self, git: &GitCli, path: &Path, undo: bool) -> Result<Entry> {
        let stack = if undo { &mut self.undo } else { &mut self.redo };
        let what = if undo { "undo" } else { "redo" };
        let Some(entry) = stack.pop() else {
            return Err(Error::Invalid(format!("nothing to {what}")));
        };
        if !matches(&snapshot(git, path)?, &entry, undo) {
            self.clear();
            return Err(Error::Invalid(format!(
                "cannot {what} “{}”: the repository changed since",
                entry.label
            )));
        }
        Ok(entry)
    }
}

/// What changed between two snapshots, in the order to replay them.
fn diff(before: &Snapshot, after: &Snapshot, style: UndoStyle) -> Vec<Change> {
    let same_branch = before.head.branch == after.head.branch;
    let head_moved = before.head != after.head;
    // A move of the checked-out branch is part of the HEAD change.
    let implied = same_branch && head_moved;
    let names: BTreeSet<&String> = before.refs.keys().chain(after.refs.keys()).collect();
    let mut changes: Vec<Change> = names
        .into_iter()
        .filter(|name| !(implied && after.head.branch.as_ref() == Some(*name)))
        .filter_map(|name| {
            let old = before.refs.get(name);
            let new = after.refs.get(name);
            (old.map(|v| &v.oid) != new.map(|v| &v.oid)).then(|| Change::Ref {
                name: name.clone(),
                before: old.map(|v| v.oid.clone()),
                after: new.map(|v| v.oid.clone()),
                upstreams: (
                    old.and_then(|v| v.upstream.clone()),
                    new.and_then(|v| v.upstream.clone()),
                ),
            })
        })
        .collect();
    if head_moved {
        let mode = match style {
            _ if !same_branch => HeadMode::Checkout,
            UndoStyle::Checkout => HeadMode::Checkout,
            UndoStyle::Soft => HeadMode::Reset(ResetKind::Soft),
            UndoStyle::Mixed => HeadMode::Reset(ResetKind::Mixed),
            UndoStyle::Keep => HeadMode::Reset(ResetKind::Keep),
        };
        // Refs first: redo creates a branch before switching to it, and undo
        // (in reverse) switches away before deleting it.
        changes.push(Change::Head {
            before: before.head.clone(),
            after: after.head.clone(),
            mode,
        });
    }
    changes
}

/// Whether `now` is where `entry` left the repository (`undo`) or where it
/// found it (redo).
fn matches(now: &Snapshot, entry: &Entry, undo: bool) -> bool {
    entry.changes.iter().all(|change| match change {
        Change::Head { before, after, .. } => now.head == *if undo { after } else { before },
        Change::Ref {
            name,
            before,
            after,
            ..
        } => now.refs.get(name).map(|v| &v.oid) == if undo { after } else { before }.as_ref(),
        Change::Rename { before, after } => {
            let (present, absent) = if undo {
                (after, before)
            } else {
                (before, after)
            };
            now.refs.contains_key(&format!("refs/heads/{present}"))
                && !now.refs.contains_key(&format!("refs/heads/{absent}"))
        }
    })
}

/// The HEAD commit before and after, if the entry resets the current branch.
fn head_reset(changes: &[Change]) -> Option<(Option<&str>, &str)> {
    changes.iter().find_map(|c| match c {
        Change::Head {
            before,
            after,
            mode: HeadMode::Reset(_),
        } => Some((before.oid.as_deref(), after.oid.as_deref()?)),
        _ => None,
    })
}

/// Why undoing `entry` would be wrong now, if it would.
fn blocked(git: &GitCli, path: &Path, entry: &Entry) -> Result<Option<String>> {
    let Some((before, after)) = head_reset(&entry.changes) else {
        return Ok(None);
    };
    let pushed = pushed_count(git, path, after, before)?;
    Ok((pushed > entry.pushed).then(|| {
        format!(
            "“{}” was pushed; undoing it would rewrite commits a remote already has",
            entry.label
        )
    }))
}

/// How many commits reachable from `tip` but not from `base` are on a
/// remote-tracking branch.
fn pushed_count(git: &GitCli, path: &Path, tip: &str, base: Option<&str>) -> Result<u32> {
    let exclude = base.map(|b| format!("^{b}"));
    let mut args = vec!["rev-list", "--count", tip];
    args.extend(exclude.as_deref());
    let count = |args: &[&str]| -> Result<u32> {
        let out = git.run(path, args)?;
        out.trim()
            .parse()
            .map_err(|_| Error::Invalid(format!("unexpected rev-list output: {out}")))
    };
    let all = count(&args)?;
    args.extend(["--not", "--remotes"]);
    let local_only = count(&args)?;
    Ok(all.saturating_sub(local_only))
}

/// Applies `change` backwards (`undo`) or forwards.
fn apply(git: &GitCli, workdir: &Path, change: &Change, undo: bool) -> Result<()> {
    let pick = |before, after| if undo { before } else { after };
    match change {
        Change::Head {
            before,
            after,
            mode,
        } => {
            let (from, to) = if undo {
                (after, before)
            } else {
                (before, after)
            };
            move_head(git, workdir, from, to, *mode)
        }
        Change::Ref {
            name,
            before,
            after,
            upstreams,
        } => {
            let (from, to) = if undo {
                (after, before)
            } else {
                (before, after)
            };
            move_ref(git, workdir, name, from.as_deref(), to.as_deref())?;
            let upstream = if undo { &upstreams.0 } else { &upstreams.1 };
            // Best effort: the upstream's remote may be gone by now.
            if let (Some(short), Some(upstream), None) =
                (name.strip_prefix("refs/heads/"), upstream, from)
            {
                let flag = format!("--set-upstream-to={upstream}");
                let _ = git.run(workdir, ["branch", "--quiet", &flag, short]);
            }
            Ok(())
        }
        Change::Rename { before, after } => {
            let (from, to) = (pick(after, before), pick(before, after));
            git.run(workdir, ["branch", "--move", from, to])?;
            Ok(())
        }
    }
}

fn move_head(
    git: &GitCli,
    workdir: &Path,
    from: &HeadPos,
    to: &HeadPos,
    mode: HeadMode,
) -> Result<()> {
    let args: Vec<&str> = match (mode, &to.branch, &to.oid) {
        (HeadMode::Checkout, Some(branch), _) => {
            let short = branch.strip_prefix("refs/heads/").unwrap_or(branch);
            vec!["switch", "--quiet", "--no-guess", short]
        }
        (HeadMode::Checkout, None, Some(oid)) => vec!["switch", "--quiet", "--detach", oid],
        (HeadMode::Reset(kind), _, Some(oid)) => {
            let flag = match kind {
                ResetKind::Soft => "--soft",
                ResetKind::Mixed => "--mixed",
                ResetKind::Keep => "--keep",
            };
            vec!["reset", "--quiet", flag, oid]
        }
        // Back to before the first commit: the branch becomes unborn again.
        (HeadMode::Reset(ResetKind::Soft), Some(branch), None) => {
            let old = from.oid.as_deref().unwrap_or_default();
            vec!["update-ref", "-d", branch, old]
        }
        _ => return Err(Error::Invalid("cannot restore HEAD there".into())),
    };
    git.run(workdir, args)?;
    Ok(())
}

fn move_ref(
    git: &GitCli,
    workdir: &Path,
    name: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<()> {
    match (from, to) {
        (Some(_), None) if name.starts_with("refs/heads/") => {
            // `branch -D` also drops the branch's config (its upstream).
            let short = &name["refs/heads/".len()..];
            git.run(workdir, ["branch", "-D", short])?;
        }
        (Some(old), None) => {
            git.run(workdir, ["update-ref", "-d", name, old])?;
        }
        (from, Some(new)) => {
            // An all-zero old value means "must not exist yet".
            let zero = "0".repeat(new.len());
            git.run(workdir, ["update-ref", name, new, from.unwrap_or(&zero)])?;
        }
        (None, None) => {}
    }
    Ok(())
}
