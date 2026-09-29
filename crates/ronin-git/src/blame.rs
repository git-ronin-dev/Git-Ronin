//! Line-by-line authorship of a file, and the commits that changed it.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::cli::operand;
use crate::detail::parse_status;
use crate::repo::discover;
use crate::{FileStatus, GitCli, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Blame {
    /// Each commit once, in order of first appearance.
    pub commits: Vec<BlameCommit>,
    pub lines: Vec<BlameLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlameCommit {
    /// All zeros for lines not committed yet.
    pub oid: String,
    pub author_name: String,
    pub author_email: String,
    #[ts(type = "number")]
    pub time: i64,
    pub summary: String,
    /// The file's path in that commit, which differs after a rename.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlameLine {
    /// Index into `commits`.
    pub commit: u32,
    /// The line's number in that commit's version of the file.
    pub orig_line: u32,
    /// Without the line ending; a `\r` of CRLF files is kept.
    pub text: String,
}

/// Who last changed each line of `file` as of `rev`, or of the working
/// tree when `None`.
pub fn blame(
    git: &GitCli,
    path: &Path,
    file: &str,
    rev: Option<&str>,
    ignore_whitespace: bool,
) -> Result<Blame> {
    let repo = discover(path)?;
    let workdir = repo.workdir().unwrap_or(repo.git_dir());
    let mut args = vec!["blame", "--porcelain"];
    if ignore_whitespace {
        args.push("-w");
    }
    if let Some(rev) = rev {
        args.push(operand(rev)?);
    }
    args.extend(["--", file]);
    Ok(parse_porcelain(&git.run(workdir, args)?))
}

fn parse_porcelain(output: &str) -> Blame {
    let mut blame = Blame::default();
    let mut index: HashMap<String, u32> = HashMap::new();
    // The commit and original line of the line whose content comes next.
    let mut current: Option<(u32, u32)> = None;
    for line in output.split_terminator('\n') {
        if let Some(text) = line.strip_prefix('\t') {
            if let Some((commit, orig_line)) = current.take() {
                blame.lines.push(BlameLine {
                    commit,
                    orig_line,
                    text: text.to_owned(),
                });
            }
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key.len() >= 40 && key.bytes().all(|b| b.is_ascii_hexdigit()) {
            // "<oid> <orig line> <final line> [<lines in group>]"
            let orig_line = value.split(' ').next().and_then(|n| n.parse().ok());
            let next = blame.commits.len() as u32;
            let commit = *index.entry(key.to_owned()).or_insert_with(|| {
                blame.commits.push(BlameCommit {
                    oid: key.to_owned(),
                    author_name: String::new(),
                    author_email: String::new(),
                    time: 0,
                    summary: String::new(),
                    path: String::new(),
                });
                next
            });
            current = Some((commit, orig_line.unwrap_or(0)));
            continue;
        }
        let Some(commit) = current.and_then(|(c, _)| blame.commits.get_mut(c as usize)) else {
            continue;
        };
        match key {
            "author" => commit.author_name = value.to_owned(),
            "author-mail" => {
                commit.author_email = value
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .to_owned()
            }
            "author-time" => commit.time = value.parse().unwrap_or(0),
            "summary" => commit.summary = value.to_owned(),
            "filename" => commit.path = value.to_owned(),
            _ => {}
        }
    }
    blame
}

/// A commit that changed a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileCommit {
    pub oid: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    #[ts(type = "number")]
    pub time: i64,
    pub summary: String,
    /// The file's path in this commit.
    pub path: String,
    /// Its path before, when this commit renamed it.
    pub old_path: Option<String>,
    pub status: FileStatus,
}

/// Commits that changed `file`, newest first, following renames. Starts at
/// `rev` (HEAD when `None`), skipping the first `skip` commits and
/// returning at most `limit`.
pub fn file_log(
    git: &GitCli,
    path: &Path,
    file: &str,
    rev: Option<&str>,
    skip: u32,
    limit: u32,
) -> Result<Vec<FileCommit>> {
    let repo = discover(path)?;
    let workdir = repo.workdir().unwrap_or(repo.git_dir());
    let skip = format!("--skip={skip}");
    let limit = format!("--max-count={limit}");
    let mut args = vec![
        "--literal-pathspecs",
        "log",
        "--follow",
        "-M",
        "-z",
        "--name-status",
        "--format=%x1e%H%x00%P%x00%an%x00%ae%x00%at%x00%s",
        &skip,
        &limit,
    ];
    args.push(match rev {
        Some(rev) => operand(rev)?,
        None => "HEAD",
    });
    args.extend(["--", file]);
    Ok(parse_file_log(&git.run(workdir, args)?, file))
}

fn parse_file_log(output: &str, file: &str) -> Vec<FileCommit> {
    let mut commits = Vec::new();
    // Going back in time, the path the file had last.
    let mut path = file.to_owned();
    for record in output.split('\x1e').filter(|r| !r.is_empty()) {
        let mut fields = record.split('\0');
        let mut next = || fields.next().unwrap_or_default();
        let (oid, parents, name, email, time, summary) =
            (next(), next(), next(), next(), next(), next());
        let status = next().trim_start_matches('\n');
        let (mut status_value, mut old_path) = (FileStatus::Modified, None);
        if !status.is_empty() {
            status_value = parse_status(status);
            let first = next();
            if status.starts_with(['R', 'C']) {
                old_path = Some(first.to_owned());
                path = next().to_owned();
            } else if !first.is_empty() {
                path = first.to_owned();
            }
        }
        commits.push(FileCommit {
            oid: oid.to_owned(),
            parents: parents
                .split(' ')
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect(),
            author_name: name.to_owned(),
            author_email: email.to_owned(),
            time: time.parse().unwrap_or(0),
            summary: summary.to_owned(),
            path: path.clone(),
            old_path: old_path.clone(),
            status: status_value,
        });
        if let Some(old) = old_path {
            path = old;
        }
    }
    commits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blame_porcelain() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let out = format!(
            "{a} 1 1 2\nauthor Ann\nauthor-mail <ann@x>\nauthor-time 100\nsummary First\n\
             boundary\nfilename old.txt\n\tone\n{a} 2 2\n\ttwo\r\n\
             {b} 5 3 1\nauthor Bob\nauthor-mail <bob@x>\nauthor-time 200\nsummary Second\n\
             previous {a} old.txt\nfilename new.txt\n\t\n"
        );
        let blame = parse_porcelain(&out);
        assert_eq!(blame.commits.len(), 2);
        assert_eq!(blame.commits[0].author_email, "ann@x");
        assert_eq!(blame.commits[0].path, "old.txt");
        assert_eq!(blame.commits[1].summary, "Second");
        assert_eq!(blame.commits[1].time, 200);
        let lines: Vec<_> = blame
            .lines
            .iter()
            .map(|l| (l.commit, l.orig_line, l.text.as_str()))
            .collect();
        assert_eq!(lines, [(0, 1, "one"), (0, 2, "two\r"), (1, 5, "")]);
    }

    #[test]
    fn parses_file_log_across_renames() {
        let out = "\x1eccc\x00bbb\x00Ann\x00a@x\x00300\x00Edit\x00\nM\x00new.txt\x00\
                   \x1ebbb\x00aaa\x00Ann\x00a@x\x00200\x00Rename\x00\nR090\x00old.txt\x00new.txt\x00\
                   \x1eaaa\x00\x00Bob\x00b@x\x00100\x00Add\x00\nA\x00old.txt\x00";
        let log = parse_file_log(out, "new.txt");
        let summary: Vec<_> = log
            .iter()
            .map(|c| {
                (
                    c.oid.as_str(),
                    c.path.as_str(),
                    c.old_path.as_deref(),
                    c.status,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("ccc", "new.txt", None, FileStatus::Modified),
                ("bbb", "new.txt", Some("old.txt"), FileStatus::Renamed),
                ("aaa", "old.txt", None, FileStatus::Added),
            ]
        );
        assert_eq!(log[0].parents, ["bbb"]);
        assert!(log[2].parents.is_empty());
    }
}
