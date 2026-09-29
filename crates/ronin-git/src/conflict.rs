//! Merge conflicts: the versions of a conflicted file, git's merge of them
//! split into agreed and conflicting chunks, and resolving the file.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use gix::ObjectId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::gix_err;
use crate::repo::{discover, operation};
use crate::{Error, GitCli, Operation, Result};

/// A conflicted file as the editor shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Conflict {
    pub path: String,
    /// The checked-out side: the current branch, or for a rebase the
    /// branch being rebased onto.
    pub ours: ConflictSide,
    /// The side being brought in.
    pub theirs: ConflictSide,
    /// Whether the file existed in the common ancestor.
    pub has_base: bool,
    /// Both sides are text, so they can be merged chunk by chunk.
    pub text: bool,
    /// git's merge of the two sides; empty unless `text`.
    pub chunks: Vec<MergeChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConflictSide {
    /// E.g. a branch name, or a short commit id and summary.
    pub label: String,
    /// False when this side deleted the file.
    pub present: bool,
}

/// Part of a merged file. Texts keep their line endings, so concatenating
/// the chosen texts gives the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum MergeChunk {
    /// Both sides agree (or only one changed it).
    Resolved { text: String },
    Conflict {
        ours: String,
        base: String,
        theirs: String,
    },
}

/// How to resolve a conflicted file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Resolution {
    /// The file's new content (as committed: line endings are converted
    /// for the working tree like any checkout).
    Content { text: String },
    /// Our version of the whole file (deleting it if we deleted it).
    Ours,
    /// Their version of the whole file (deleting it if they deleted it).
    Theirs,
    /// Delete the file.
    Delete,
}

/// Longer than git's default, so lines like `=======` in the file itself
/// aren't mistaken for markers.
const MARKER_SIZE: usize = 24;

/// Loads conflicted file `file` for the conflict editor.
pub fn conflict(git: &GitCli, path: &Path, file: &str) -> Result<Conflict> {
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    let stages = stages(git, workdir, file)?;
    if stages.iter().all(Option::is_none) {
        return Err(Error::Invalid(format!("{file} has no conflicts")));
    }
    let (ours_label, theirs_label) = labels(git, &repo, workdir);
    let submodule = stages.iter().flatten().any(|s| s.mode == "160000");
    let read = |stage: &Option<Stage>| -> Result<Option<Vec<u8>>> {
        match stage {
            Some(s) if !submodule => {
                let id = ObjectId::from_hex(s.oid.as_bytes()).map_err(gix_err)?;
                Ok(Some(repo.find_object(id).map_err(gix_err)?.detach().data))
            }
            _ => Ok(None),
        }
    };
    let [base, ours, theirs] = [read(&stages[0])?, read(&stages[1])?, read(&stages[2])?];
    let text = !submodule
        && ours.is_some()
        && theirs.is_some()
        && [&base, &ours, &theirs]
            .into_iter()
            .flatten()
            .all(|b| is_text(b));
    let chunks = if text {
        let merged = merge_file(
            git,
            workdir,
            base.as_deref().unwrap_or_default(),
            ours.as_deref().unwrap_or_default(),
            theirs.as_deref().unwrap_or_default(),
        )?;
        parse_merged(&merged)
    } else {
        Vec::new()
    };
    Ok(Conflict {
        path: file.to_owned(),
        ours: ConflictSide {
            label: ours_label,
            present: stages[1].is_some(),
        },
        theirs: ConflictSide {
            label: theirs_label,
            present: stages[2].is_some(),
        },
        has_base: stages[0].is_some(),
        text,
        chunks,
    })
}

/// Resolves conflicted file `file` and marks it resolved in the index.
pub fn resolve_conflict(
    git: &GitCli,
    path: &Path,
    file: &str,
    resolution: &Resolution,
) -> Result<()> {
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    let stages = stages(git, workdir, file)?;
    if stages.iter().all(Option::is_none) {
        return Err(Error::Invalid(format!("{file} has no conflicts")));
    }
    let side = |n: usize| stages[n].as_ref().map(|s| (s.mode.clone(), s.oid.clone()));
    let entry = match resolution {
        Resolution::Content { text } => {
            let mode = side(1)
                .or_else(|| side(2))
                .map_or_else(|| "100644".to_owned(), |(mode, _)| mode);
            let oid = git.run_with_input(
                workdir,
                ["hash-object", "-w", "--no-filters", "--stdin"],
                text.as_bytes(),
            )?;
            Some((mode, oid.trim().to_owned()))
        }
        Resolution::Ours => side(1),
        Resolution::Theirs => side(2),
        Resolution::Delete => None,
    };
    match entry {
        Some((mode, oid)) => {
            // A stage-0 entry replaces the conflicting ones; checking it out
            // applies the usual line-ending conversion and filters.
            let info = format!("{mode},{oid},{file}");
            git.run(workdir, ["update-index", "--add", "--cacheinfo", &info])?;
            git.run(workdir, ["checkout-index", "--force", "--", file])?;
        }
        None => {
            git.run(
                workdir,
                [
                    "--literal-pathspecs",
                    "rm",
                    "--quiet",
                    "--force",
                    "--ignore-unmatch",
                    "--",
                    file,
                ],
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct Stage {
    mode: String,
    oid: String,
}

/// The base, ours and theirs index entries of `file` (stages 1 to 3).
fn stages(git: &GitCli, workdir: &Path, file: &str) -> Result<[Option<Stage>; 3]> {
    let out = git.run(
        workdir,
        [
            "--literal-pathspecs",
            "ls-files",
            "--unmerged",
            "-z",
            "--",
            file,
        ],
    )?;
    let mut stages: [Option<Stage>; 3] = Default::default();
    for record in out.split('\0') {
        // "<mode> <oid> <stage>\t<path>"
        let Some((info, name)) = record.split_once('\t') else {
            continue;
        };
        let fields: Vec<&str> = info.split(' ').collect();
        let [mode, oid, stage] = fields[..] else {
            continue;
        };
        let index = match stage {
            "1" => 0,
            "2" => 1,
            "3" => 2,
            _ => continue,
        };
        if name == file {
            stages[index] = Some(Stage {
                mode: mode.to_owned(),
                oid: oid.to_owned(),
            });
        }
    }
    Ok(stages)
}

fn is_text(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8000)];
    !head.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

/// Labels for our side and theirs, from the operation in progress.
fn labels(git: &GitCli, repo: &gix::Repository, workdir: &Path) -> (String, String) {
    let git_dir = repo.git_dir();
    let read = |name: &str| {
        std::fs::read_to_string(git_dir.join(name))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    let branch_at = |oid: &str| -> Option<String> {
        let at = format!("--points-at={oid}");
        let out = git
            .run(
                workdir,
                [
                    "for-each-ref",
                    "--format=%(refname:short)",
                    &at,
                    "refs/heads",
                    "refs/remotes",
                ],
            )
            .ok()?;
        out.lines().next().map(str::to_owned)
    };
    let commit = |oid: &str| -> String {
        let summary = git
            .run(workdir, ["log", "-1", "--format=%s", oid, "--"])
            .map(|s| s.trim().to_owned())
            .unwrap_or_default();
        let short = &oid[..oid.len().min(7)];
        if summary.is_empty() {
            short.to_owned()
        } else {
            format!("{short} {summary}")
        }
    };
    let head = || {
        repo.head_name()
            .ok()
            .flatten()
            .map(|n| n.shorten().to_string())
            .unwrap_or_else(|| "HEAD".into())
    };

    match operation(repo) {
        Some(Operation::Rebase) => {
            let onto = read("rebase-merge/onto").or_else(|| read("rebase-apply/onto"));
            let ours = onto
                .map(|o| branch_at(&o).unwrap_or_else(|| commit(&o)))
                .unwrap_or_else(|| "HEAD".into());
            let theirs =
                read("REBASE_HEAD").map_or_else(|| "Commit being rebased".into(), |o| commit(&o));
            (ours, theirs)
        }
        Some(Operation::Merge) => {
            let theirs = read("MERGE_HEAD")
                .and_then(|o| o.lines().next().map(str::to_owned))
                .map_or_else(
                    || "Merged commit".into(),
                    |o| branch_at(&o).unwrap_or_else(|| commit(&o)),
                );
            (head(), theirs)
        }
        Some(Operation::CherryPick) => {
            let theirs =
                read("CHERRY_PICK_HEAD").map_or_else(|| "Picked commit".into(), |o| commit(&o));
            (head(), theirs)
        }
        Some(Operation::Revert) => {
            let theirs = read("REVERT_HEAD").map_or_else(
                || "Reverted commit".into(),
                |o| format!("Revert of {}", commit(&o)),
            );
            (head(), theirs)
        }
        // Applying a stash is the common case without an operation.
        _ => (head(), "Incoming changes".into()),
    }
}

/// Runs `git merge-file` on the three versions and returns its output,
/// with diff3-style markers around each conflict.
fn merge_file(
    git: &GitCli,
    workdir: &Path,
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
) -> Result<String> {
    let dir = ScratchDir::new()?;
    let write = |name: &str, bytes: &[u8]| -> Result<PathBuf> {
        let file = dir.0.join(name);
        std::fs::write(&file, bytes)?;
        Ok(file)
    };
    let files = [
        write("ours", ours)?,
        write("base", base)?,
        write("theirs", theirs)?,
    ];
    let marker = format!("--marker-size={MARKER_SIZE}");
    let mut args: Vec<std::ffi::OsString> = [
        "merge-file",
        "--stdout",
        "--diff3",
        &marker,
        "-L",
        "ours",
        "-L",
        "base",
        "-L",
        "theirs",
    ]
    .map(Into::into)
    .into();
    args.extend(files.iter().map(|f| f.as_os_str().to_owned()));
    // Exits with the number of conflicts (capped at 127), or 255 on error.
    let codes: Vec<i32> = (0..128).collect();
    git.run_accepting(workdir, &args, &codes)
}

/// Splits `git merge-file --diff3` output into chunks.
fn parse_merged(merged: &str) -> Vec<MergeChunk> {
    let marker = |c: char| c.to_string().repeat(MARKER_SIZE);
    let (start, base_mark, mid, end) = (marker('<'), marker('|'), marker('='), marker('>'));
    let is = |line: &str, mark: &str| {
        let body = line.trim_end_matches(['\n', '\r']);
        body.strip_prefix(mark)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    };

    enum In {
        Resolved,
        Ours,
        Base,
        Theirs,
    }
    let mut chunks = Vec::new();
    let mut state = In::Resolved;
    let (mut resolved, mut ours, mut base, mut theirs) =
        (String::new(), String::new(), String::new(), String::new());
    for line in merged.split_inclusive('\n') {
        match state {
            In::Resolved if is(line, &start) => {
                if !resolved.is_empty() {
                    chunks.push(MergeChunk::Resolved {
                        text: std::mem::take(&mut resolved),
                    });
                }
                state = In::Ours;
            }
            In::Resolved => resolved.push_str(line),
            In::Ours if is(line, &base_mark) => state = In::Base,
            In::Ours if is(line, &mid) => state = In::Theirs,
            In::Ours => ours.push_str(line),
            In::Base if is(line, &mid) => state = In::Theirs,
            In::Base => base.push_str(line),
            In::Theirs if is(line, &end) => {
                chunks.push(MergeChunk::Conflict {
                    ours: std::mem::take(&mut ours),
                    base: std::mem::take(&mut base),
                    theirs: std::mem::take(&mut theirs),
                });
                state = In::Resolved;
            }
            In::Theirs => theirs.push_str(line),
        }
    }
    if !resolved.is_empty() {
        chunks.push(MergeChunk::Resolved { text: resolved });
    }
    chunks
}

/// A temporary directory, removed when dropped.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Result<Self> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir();
        loop {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = base.join(format!("ronin-merge-{}-{n}", std::process::id()));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(Self(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_merged_output_into_chunks() {
        let m = |c: char| c.to_string().repeat(MARKER_SIZE);
        let merged = format!(
            "keep\n{} ours\nmine\n{} base\norig\n{}\ntheirs\n{} theirs\ntail\n=======\n",
            m('<'),
            m('|'),
            m('='),
            m('>')
        );
        assert_eq!(
            parse_merged(&merged),
            [
                MergeChunk::Resolved {
                    text: "keep\n".into()
                },
                MergeChunk::Conflict {
                    ours: "mine\n".into(),
                    base: "orig\n".into(),
                    theirs: "theirs\n".into(),
                },
                MergeChunk::Resolved {
                    // Short markers are content.
                    text: "tail\n=======\n".into()
                },
            ]
        );
    }

    #[test]
    fn keeps_empty_sides_and_carriage_returns() {
        let m = |c: char| c.to_string().repeat(MARKER_SIZE);
        let merged = format!(
            "{} ours\r\n{} base\r\na\r\n{}\r\n{} theirs\r\n",
            m('<'),
            m('|'),
            m('='),
            m('>')
        );
        assert_eq!(
            parse_merged(&merged),
            [MergeChunk::Conflict {
                ours: String::new(),
                base: "a\r\n".into(),
                theirs: String::new(),
            }]
        );
    }

    #[test]
    fn detects_binary_content() {
        assert!(is_text(b"plain\ntext\n"));
        assert!(!is_text(b"PNG\0\x01"));
        assert!(!is_text(&[0xff, 0xfe, b'a']));
    }
}
