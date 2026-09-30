use std::collections::HashMap;
use std::path::Path;

use gix::ObjectId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::gix_err;
use crate::repo::discover;
use crate::{Error, GitCli, Result};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitDetail {
    pub oid: String,
    pub parents: Vec<String>,
    pub author: Signature,
    pub committer: Signature,
    pub message: String,
    /// Changes relative to the first parent (or to nothing, for root commits).
    pub files: Vec<FileChange>,
    /// Why `files` is empty when listing changes failed (e.g. a partial clone
    /// that can't fetch the blobs). The rest of the detail is still valid.
    pub files_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Signature {
    pub name: String,
    pub email: String,
    /// Seconds since the Unix epoch.
    #[ts(type = "number")]
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileChange {
    pub path: String,
    /// Source path for renames and copies.
    pub old_path: Option<String>,
    pub status: FileStatus,
    /// `None` for binary files.
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    /// Only in the working tree (uncommitted changes).
    Untracked,
    /// Unmerged, with conflicts to resolve (uncommitted changes).
    Conflicted,
    Unknown,
}

pub fn commit_detail(git: &GitCli, path: &Path, oid: &str) -> Result<CommitDetail> {
    let repo = discover(path)?;
    let id = ObjectId::from_hex(oid.as_bytes()).map_err(gix_err)?;
    let commit = repo.find_commit(id).map_err(gix_err)?;
    let signature = |s: gix::actor::SignatureRef<'_>| {
        let s = s.trim();
        Signature {
            name: s.name.to_string(),
            email: s.email.to_string(),
            time: s.seconds(),
        }
    };
    let parents: Vec<String> = commit.parent_ids().map(|p| p.to_string()).collect();
    let base = match parents.first() {
        Some(p) => p.clone(),
        None => repo.object_hash().empty_tree().to_string(),
    };
    let workdir = repo.workdir().unwrap_or(repo.git_dir());
    let args = [
        "diff-tree",
        "-r",
        "-z",
        "-M",
        "--raw",
        "--numstat",
        "--no-commit-id",
        &base,
        oid,
    ];
    let files = git.run(workdir, args).map(|raw| parse_diff_tree(&raw));

    Ok(CommitDetail {
        oid: oid.to_owned(),
        author: signature(commit.author().map_err(gix_err)?),
        committer: signature(commit.committer().map_err(gix_err)?),
        message: commit.message_raw_sloppy().to_string(),
        parents,
        files_error: files.as_ref().err().map(ToString::to_string),
        files: files.unwrap_or_default(),
    })
}

/// Reads the file at `file_path` as of commit `oid`. `None` if it doesn't exist there.
pub fn blob_at(path: &Path, oid: &str, file_path: &str) -> Result<Option<Vec<u8>>> {
    let repo = discover(path)?;
    let id = ObjectId::from_hex(oid.as_bytes()).map_err(gix_err)?;
    let tree = repo
        .find_commit(id)
        .map_err(gix_err)?
        .tree()
        .map_err(gix_err)?;
    let Some(entry) = tree.lookup_entry_by_path(file_path).map_err(gix_err)? else {
        return Ok(None);
    };
    let object = entry.object().map_err(gix_err)?;
    Ok(Some(object.detach().data))
}

/// A version of a file with uncommitted changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BlobSource {
    Head,
    Index,
    Worktree,
}

/// Reads `file_path` from HEAD, the index or the working tree. `None` if it
/// doesn't exist there.
pub fn working_blob(path: &Path, file_path: &str, source: BlobSource) -> Result<Option<Vec<u8>>> {
    let repo = discover(path)?;
    let id = match source {
        BlobSource::Head => {
            let Ok(commit) = repo.head_commit() else {
                return Ok(None);
            };
            let tree = commit.tree().map_err(gix_err)?;
            match tree.lookup_entry_by_path(file_path).map_err(gix_err)? {
                Some(entry) => entry.object_id(),
                None => return Ok(None),
            }
        }
        BlobSource::Index => {
            let index = repo.index_or_empty().map_err(gix_err)?;
            match index.entry_by_path(file_path.into()) {
                Some(entry) => entry.id,
                None => return Ok(None),
            }
        }
        BlobSource::Worktree => {
            let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
            return match std::fs::read(workdir.join(file_path)) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            };
        }
    };
    let object = repo.find_object(id).map_err(gix_err)?;
    Ok(Some(object.detach().data))
}

/// Parses `git diff-tree -z --raw --numstat`: all raw records, then all numstat records.
pub(crate) fn parse_diff_tree(output: &str) -> Vec<FileChange> {
    let mut tokens = output.split('\0').filter(|t| !t.is_empty());
    let mut files = Vec::new();
    let mut stats: HashMap<String, (Option<u32>, Option<u32>)> = HashMap::new();

    while let Some(token) = tokens.next() {
        if let Some(raw) = token.strip_prefix(':') {
            // ":100644 100644 <old> <new> R087"
            let status = raw.rsplit(' ').next().unwrap_or("");
            let (old_path, path) = if status.starts_with(['R', 'C']) {
                let old = tokens.next().unwrap_or_default();
                (Some(old.to_owned()), tokens.next().unwrap_or_default())
            } else {
                (None, tokens.next().unwrap_or_default())
            };
            files.push(FileChange {
                path: path.to_owned(),
                old_path,
                status: parse_status(status),
                additions: None,
                deletions: None,
            });
        } else {
            // "<added>\t<deleted>\t<path>", or "...\t" followed by old and new paths.
            let mut parts = token.splitn(3, '\t');
            let (Some(added), Some(deleted)) = (parts.next(), parts.next()) else {
                continue;
            };
            let path = match parts.next() {
                Some(p) if !p.is_empty() => p.to_owned(),
                _ => {
                    tokens.next();
                    tokens.next().unwrap_or_default().to_owned()
                }
            };
            stats.insert(path, (added.parse().ok(), deleted.parse().ok()));
        }
    }

    for file in &mut files {
        if let Some(&(additions, deletions)) = stats.get(&file.path) {
            file.additions = additions;
            file.deletions = deletions;
        }
    }
    files
}

pub(crate) fn parse_status(status: &str) -> FileStatus {
    match status.chars().next() {
        Some('A') => FileStatus::Added,
        Some('M') => FileStatus::Modified,
        Some('D') => FileStatus::Deleted,
        Some('R') => FileStatus::Renamed,
        Some('C') => FileStatus::Copied,
        Some('T') => FileStatus::TypeChanged,
        _ => FileStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_and_numstat_records() {
        let out = ":100644 100644 aaa bbb M\0src/a.rs\0\
                   :000000 100644 000 ccc A\0img.png\0\
                   :100644 100644 ddd eee R090\0old name.txt\0new name.txt\0\
                   3\t1\tsrc/a.rs\0-\t-\timg.png\0\
                   2\t0\t\0old name.txt\0new name.txt\0";
        let files = parse_diff_tree(out);
        assert_eq!(
            files,
            [
                FileChange {
                    path: "src/a.rs".into(),
                    old_path: None,
                    status: FileStatus::Modified,
                    additions: Some(3),
                    deletions: Some(1),
                },
                FileChange {
                    path: "img.png".into(),
                    old_path: None,
                    status: FileStatus::Added,
                    additions: None,
                    deletions: None,
                },
                FileChange {
                    path: "new name.txt".into(),
                    old_path: Some("old name.txt".into()),
                    status: FileStatus::Renamed,
                    additions: Some(2),
                    deletions: Some(0),
                },
            ]
        );
    }
}
