//! Interactive rebase: the commits it would replay, and running it with the
//! user's plan. git is driven through `GIT_SEQUENCE_EDITOR`, which copies
//! the prepared todo list over the one git offers; new commit messages are
//! applied by `exec` lines that amend the commit just made.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cli::operand;
use crate::history::finish;
use crate::repo::{discover, operation};
use crate::{Error, GitCli, Outcome, Result};

/// What to do with one commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RebaseAction {
    Pick,
    /// Pick with a new message.
    Reword,
    /// Pick, then stop so the commit can be amended.
    Edit,
    /// Meld into the previous commit, keeping both messages.
    Squash,
    /// Meld into the previous commit, keeping only its message.
    Fixup,
    Drop,
}

/// A commit an interactive rebase would replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebaseCommit {
    pub oid: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    #[ts(type = "number")]
    pub time: i64,
}

/// The commits between `base` and HEAD, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebasePlan {
    /// `None` to rewrite from the root commit.
    pub base: Option<String>,
    /// HEAD when the plan was made; the rebase refuses to run if it moved.
    pub head: String,
    pub commits: Vec<RebaseCommit>,
    /// Merge commits in the range; rebasing flattens them away.
    pub merges: u32,
}

/// One line of the user's plan, in the order to replay.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebaseStep {
    pub oid: String,
    pub action: RebaseAction,
    /// The new message for `Reword`, or for the commit a `Squash` produces.
    pub message: Option<String>,
}

/// The commits an interactive rebase onto `base` (or from the root, when
/// `None`) would replay: those on HEAD but not on `base`.
pub fn rebase_plan(git: &GitCli, path: &Path, base: Option<&str>) -> Result<RebasePlan> {
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    let head = repo
        .head_id()
        .map_err(|_| Error::Invalid("there are no commits yet".into()))?
        .to_string();
    let base = match base {
        Some(base) => {
            let oid = git.run(
                workdir,
                [
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{}^{{commit}}", operand(base)?),
                ],
            )?;
            let oid = oid.trim().to_owned();
            let ancestor = git.run_raw(workdir, ["merge-base", "--is-ancestor", &oid, &head])?;
            if !ancestor.success() {
                return Err(Error::Invalid(
                    "that commit is not on the checked-out branch".into(),
                ));
            }
            Some(oid)
        }
        None => None,
    };
    let range = match &base {
        Some(b) => format!("{b}..{head}"),
        None => head.clone(),
    };
    let out = git.run(
        workdir,
        [
            "log",
            "--reverse",
            "--topo-order",
            "--no-merges",
            "-z",
            "--format=%H%x00%an%x00%ae%x00%at%x00%B",
            &range,
            "--",
        ],
    )?;
    let mut fields = out.split('\0');
    let mut commits = Vec::new();
    while let (Some(oid), Some(name), Some(email), Some(time), Some(message)) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) {
        // `-z` separates commits with a NUL, which follows the message.
        let oid = oid.trim_start_matches('\n');
        if oid.is_empty() {
            break;
        }
        commits.push(RebaseCommit {
            oid: oid.to_owned(),
            message: message.trim_end().to_owned(),
            author_name: name.to_owned(),
            author_email: email.to_owned(),
            time: time.parse().unwrap_or(0),
        });
    }
    let merges = git.run(workdir, ["rev-list", "--count", "--merges", &range, "--"])?;
    Ok(RebasePlan {
        base,
        head,
        commits,
        merges: merges.trim().parse().unwrap_or(0),
    })
}

/// Runs an interactive rebase of HEAD onto `base` (or from the root) with
/// `steps`, which must list every commit of [`rebase_plan`] exactly once.
/// `head` is the HEAD the plan was made for. Uncommitted changes are
/// stashed first and restored afterwards.
pub fn interactive_rebase(
    git: &GitCli,
    path: &Path,
    base: Option<&str>,
    head: &str,
    steps: &[RebaseStep],
) -> Result<Outcome> {
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    if operation(&repo).is_some() {
        return Err(Error::Invalid("another operation is in progress".into()));
    }
    let plan = rebase_plan(git, path, base)?;
    if plan.head != head {
        return Err(Error::Invalid(
            "the branch moved since the rebase was planned; start again".into(),
        ));
    }
    let expected: HashSet<&str> = plan.commits.iter().map(|c| c.oid.as_str()).collect();
    let given: HashSet<&str> = steps.iter().map(|s| s.oid.as_str()).collect();
    if steps.len() != expected.len() || given != expected {
        return Err(Error::Invalid(
            "the plan doesn't match the commits being rebased".into(),
        ));
    }
    let kept = steps.iter().find(|s| s.action != RebaseAction::Drop);
    if kept.is_some_and(|s| matches!(s.action, RebaseAction::Squash | RebaseAction::Fixup)) {
        return Err(Error::Invalid(
            "the first commit kept can't be squashed: there is nothing before it to meld into"
                .into(),
        ));
    }

    // Scratch files live in the git dir until the next interactive rebase:
    // the `exec` lines may run long after this call, once the user
    // continues past a conflict.
    let scratch = repo.git_dir().join("ronin-rebase");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch)?;
    let mut todo = String::new();
    for (i, step) in steps.iter().enumerate() {
        let command = match step.action {
            RebaseAction::Pick | RebaseAction::Reword => "pick",
            RebaseAction::Edit => "edit",
            RebaseAction::Squash => "squash",
            RebaseAction::Fixup => "fixup",
            RebaseAction::Drop => "drop",
        };
        todo.push_str(&format!("{command} {}\n", step.oid));
        let message = step
            .message
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty());
        let rewords = matches!(step.action, RebaseAction::Reword | RebaseAction::Squash);
        if let Some(message) = message.filter(|_| rewords) {
            let file = scratch.join(format!("message-{i}"));
            std::fs::write(&file, format!("{message}\n"))?;
            todo.push_str(&format!(
                "exec git commit --amend --allow-empty --quiet --file={}\n",
                sh_quote(&file)
            ));
        } else if step.action == RebaseAction::Reword {
            return Err(Error::Invalid("a reworded commit needs a message".into()));
        }
    }
    let todo_file = scratch.join("todo");
    std::fs::write(&todo_file, todo)?;

    let editor = format!("cp {}", sh_quote(&todo_file));
    let git = git.clone().env("GIT_SEQUENCE_EDITOR", editor);
    let mut args = vec![
        "-c",
        "rebase.abbreviateCommands=false",
        "rebase",
        "--interactive",
        "--autostash",
    ];
    match &plan.base {
        Some(base) => args.push(base),
        None => args.push("--root"),
    }
    finish(path, git.run_raw(workdir, args)?)
}

/// Quotes a path for the POSIX shell git runs editors and `exec` lines in
/// (also on Windows, where git brings its own). Forward slashes work there
/// too.
fn sh_quote(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_for_the_shell() {
        assert_eq!(sh_quote(Path::new("/tmp/a b/todo")), "'/tmp/a b/todo'");
        assert_eq!(sh_quote(Path::new("/it's")), r"'/it'\''s'");
        assert_eq!(
            sh_quote(Path::new(r"C:\repo\.git\todo")),
            "'C:/repo/.git/todo'"
        );
    }
}
