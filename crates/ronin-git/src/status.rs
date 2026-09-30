use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::detail::parse_status;
use crate::repo::discover;
use crate::{Error, FileStatus, GitCli, RepoInfo, Result, open_repo};

/// Uncommitted changes, split the way `git status` shows them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkingStatus {
    /// Changes in the index, relative to HEAD.
    pub staged: Vec<StatusEntry>,
    /// Changes in the working tree relative to the index, and untracked files.
    pub unstaged: Vec<StatusEntry>,
    /// Unmerged paths. They are neither staged nor unstaged until resolved.
    pub conflicted: Vec<StatusEntry>,
}

impl WorkingStatus {
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty() && self.unstaged.is_empty() && self.conflicted.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatusEntry {
    pub path: String,
    /// Source path of a staged rename or copy.
    pub old_path: Option<String>,
    pub status: FileStatus,
    /// Set when the path is a submodule.
    pub submodule: Option<SubmoduleChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubmoduleChange {
    /// The submodule's checked-out commit differs from the recorded one.
    pub commit_changed: bool,
    /// Tracked files inside the submodule are modified.
    pub modified: bool,
    /// The submodule has untracked files.
    pub untracked: bool,
}

/// Lists uncommitted changes, including every untracked file.
pub fn status(git: &GitCli, path: &Path) -> Result<WorkingStatus> {
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    // No optional locks: a read must not rewrite the index, which would also
    // wake the file watcher.
    let output = git.run(
        workdir,
        [
            "--no-optional-locks",
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
            "--renames",
        ],
    )?;
    Ok(parse_porcelain_v2(&output))
}

/// An at-a-glance state of a repository, for lists of many repositories.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoSummary {
    pub info: RepoInfo,
    /// The current branch's upstream, e.g. `origin/main`.
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    /// Changed and untracked files (an untracked directory counts once).
    pub unstaged: u32,
    pub conflicted: u32,
}

/// Summarises the repository at `path` with one `git status`.
pub fn summary(git: &GitCli, path: &Path) -> Result<RepoSummary> {
    let info = open_repo(path)?;
    let mut summary = RepoSummary {
        info,
        upstream: None,
        ahead: 0,
        behind: 0,
        staged: 0,
        unstaged: 0,
        conflicted: 0,
    };
    if summary.info.is_bare {
        return Ok(summary);
    }
    let output = git.run(
        Path::new(&summary.info.path),
        [
            "--no-optional-locks",
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=normal",
        ],
    )?;
    for header in output
        .split('\0')
        .filter_map(|r| r.strip_prefix("# branch."))
    {
        if let Some(upstream) = header.strip_prefix("upstream ") {
            summary.upstream = Some(upstream.to_owned());
        } else if let Some(ab) = header.strip_prefix("ab ") {
            for part in ab.split(' ') {
                if let Some(n) = part.strip_prefix('+') {
                    summary.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    summary.behind = n.parse().unwrap_or(0);
                }
            }
        }
    }
    let status = parse_porcelain_v2(&output);
    let count = |v: &Vec<StatusEntry>| u32::try_from(v.len()).unwrap_or(u32::MAX);
    summary.staged = count(&status.staged);
    summary.unstaged = count(&status.unstaged);
    summary.conflicted = count(&status.conflicted);
    Ok(summary)
}

/// Parses `git status --porcelain=v2 -z` records.
fn parse_porcelain_v2(output: &str) -> WorkingStatus {
    let mut status = WorkingStatus::default();
    let mut records = output.split('\0').filter(|r| !r.is_empty());
    while let Some(record) = records.next() {
        let (kind, rest) = record.split_at_checked(2).unwrap_or((record, ""));
        match kind {
            "1 " => {
                // <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
                let fields: Vec<&str> = rest.splitn(8, ' ').collect();
                let [xy, sub, .., path] = fields[..] else {
                    continue;
                };
                push_changes(&mut status, xy, sub, path, None);
            }
            "2 " => {
                // <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>, then <origPath>
                let fields: Vec<&str> = rest.splitn(9, ' ').collect();
                let [xy, sub, .., path] = fields[..] else {
                    continue;
                };
                let old_path = records.next();
                push_changes(&mut status, xy, sub, path, old_path);
            }
            "u " => {
                // <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>
                let fields: Vec<&str> = rest.splitn(10, ' ').collect();
                let [_, sub, .., path] = fields[..] else {
                    continue;
                };
                status.conflicted.push(StatusEntry {
                    path: path.to_owned(),
                    old_path: None,
                    status: FileStatus::Conflicted,
                    submodule: parse_submodule(sub),
                });
            }
            "? " => status.unstaged.push(StatusEntry {
                path: rest.to_owned(),
                old_path: None,
                status: FileStatus::Untracked,
                submodule: None,
            }),
            _ => {}
        }
    }
    status
}

fn push_changes(
    status: &mut WorkingStatus,
    xy: &str,
    sub: &str,
    path: &str,
    old_path: Option<&str>,
) {
    let mut codes = xy.chars();
    let (Some(index), Some(worktree)) = (codes.next(), codes.next()) else {
        return;
    };
    let submodule = parse_submodule(sub);
    let entry = |code: char, old_path: Option<&str>| StatusEntry {
        path: path.to_owned(),
        old_path: old_path.map(str::to_owned),
        status: parse_status(code.encode_utf8(&mut [0; 4])),
        submodule,
    };
    if index != '.' {
        status.staged.push(entry(index, old_path));
    }
    if worktree != '.' {
        // The original path belongs to the staged rename, unless the rename
        // is only in the working tree (an intent-to-add file).
        let old_path = old_path.filter(|_| index == '.');
        status.unstaged.push(entry(worktree, old_path));
    }
}

/// `N...` for regular files, else `S<c><m><u>`.
fn parse_submodule(sub: &str) -> Option<SubmoduleChange> {
    let flags = sub.strip_prefix('S')?.as_bytes();
    Some(SubmoduleChange {
        commit_changed: flags.first() == Some(&b'C'),
        modified: flags.get(1) == Some(&b'M'),
        untracked: flags.get(2) == Some(&b'U'),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, old_path: Option<&str>, status: FileStatus) -> StatusEntry {
        StatusEntry {
            path: path.into(),
            old_path: old_path.map(Into::into),
            status,
            submodule: None,
        }
    }

    #[test]
    fn parses_porcelain_v2() {
        let out = "1 M. N... 100644 100644 100644 aaa bbb src/lib.rs\0\
                   1 .D N... 100644 100644 000000 aaa aaa gone.txt\0\
                   1 AM N... 000000 100644 100644 000 ccc new file.txt\0\
                   2 R. N... 100644 100644 100644 ddd ddd R100 new name.rs\0old name.rs\0\
                   u UU N... 100644 100644 100644 100644 e1 e2 e3 both.txt\0\
                   1 .M SCM. 160000 160000 160000 f1 f1 vendor/lib\0\
                   ? notes/todo.md\0";
        let status = parse_porcelain_v2(out);
        assert_eq!(
            status.staged,
            [
                entry("src/lib.rs", None, FileStatus::Modified),
                entry("new file.txt", None, FileStatus::Added),
                entry("new name.rs", Some("old name.rs"), FileStatus::Renamed),
            ]
        );
        let mut submodule = entry("vendor/lib", None, FileStatus::Modified);
        submodule.submodule = Some(SubmoduleChange {
            commit_changed: true,
            modified: true,
            untracked: false,
        });
        assert_eq!(
            status.unstaged,
            [
                entry("gone.txt", None, FileStatus::Deleted),
                entry("new file.txt", None, FileStatus::Modified),
                submodule,
                entry("notes/todo.md", None, FileStatus::Untracked),
            ]
        );
        assert_eq!(
            status.conflicted,
            [entry("both.txt", None, FileStatus::Conflicted)]
        );
    }

    #[test]
    fn empty_output_is_clean() {
        assert!(parse_porcelain_v2("").is_clean());
    }
}
