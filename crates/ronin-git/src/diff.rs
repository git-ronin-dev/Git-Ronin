use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::repo::discover;
use crate::{GitCli, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileDiff {
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Hunk {
    /// The full `@@ -a,b +c,d @@ context` line.
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
    /// Followed by "\ No newline at end of file".
    pub no_newline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffOptions {
    pub ignore_whitespace: bool,
    pub context_lines: u32,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            ignore_whitespace: false,
            context_lines: 3,
        }
    }
}

/// Diffs one file between `base` (or the empty tree when `None`) and `target`.
pub fn file_diff(
    git: &GitCli,
    path: &Path,
    base: Option<&str>,
    target: &str,
    file_path: &str,
    old_path: Option<&str>,
    options: DiffOptions,
) -> Result<FileDiff> {
    let repo = discover(path)?;
    let empty_tree = repo.object_hash().empty_tree().to_string();
    let workdir = repo.workdir().unwrap_or(repo.git_dir());
    let context = format!("-U{}", options.context_lines);

    // Literal pathspecs: file names like `*.txt` or `:x` must not be interpreted.
    let mut args = vec![
        "--literal-pathspecs",
        "diff",
        "--no-color",
        "--no-ext-diff",
        "-M",
        &context,
    ];
    if options.ignore_whitespace {
        args.push("-w");
    }
    args.extend([base.unwrap_or(&empty_tree), target, "--"]);
    args.extend(old_path);
    args.push(file_path);

    Ok(parse_unified(&git.run(workdir, &args)?))
}

/// Parses the output of `git diff` for a single file.
pub fn parse_unified(output: &str) -> FileDiff {
    let mut diff = FileDiff::default();
    let (mut old_no, mut new_no) = (0, 0);

    for line in output.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            let Some((old_start, old_lines, new_start, new_lines)) = parse_hunk_header(header)
            else {
                continue;
            };
            (old_no, new_no) = (old_start, new_start);
            diff.hunks.push(Hunk {
                header: line.to_owned(),
                old_start,
                old_lines,
                new_start,
                new_lines,
                lines: Vec::new(),
            });
            continue;
        }
        let Some(hunk) = diff.hunks.last_mut() else {
            // File header, before the first hunk.
            if line.starts_with("Binary files ") || line == "GIT binary patch" {
                diff.binary = true;
            }
            continue;
        };
        let (kind, text) = match line.split_at_checked(1) {
            Some((" ", text)) => (LineKind::Context, text),
            Some(("+", text)) => (LineKind::Added, text),
            Some(("-", text)) => (LineKind::Removed, text),
            Some(("\\", _)) => {
                if let Some(last) = hunk.lines.last_mut() {
                    last.no_newline = true;
                }
                continue;
            }
            // An empty context line can lose its leading space in some tooling.
            None => (LineKind::Context, ""),
            _ => continue,
        };
        let (old_line, new_line) = match kind {
            LineKind::Context => (Some(old_no), Some(new_no)),
            LineKind::Added => (None, Some(new_no)),
            LineKind::Removed => (Some(old_no), None),
        };
        if old_line.is_some() {
            old_no += 1;
        }
        if new_line.is_some() {
            new_no += 1;
        }
        hunk.lines.push(DiffLine {
            kind,
            old_line,
            new_line,
            text: text.to_owned(),
            no_newline: false,
        });
    }
    diff
}

/// Parses "-a,b +c,d @@ ..." (counts default to 1 when omitted).
fn parse_hunk_header(header: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = header.split(' ');
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let range = |r: &str| -> Option<(u32, u32)> {
        match r.split_once(',') {
            Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
            None => Some((r.parse().ok()?, 1)),
        }
    };
    let (old_start, old_lines) = range(old)?;
    let (new_start, new_lines) = range(new)?;
    Some((old_start, old_lines, new_start, new_lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hunks_with_line_numbers() {
        let out = "diff --git a/f b/f\nindex 1..2 100644\n--- a/f\n+++ b/f\n\
                   @@ -1,3 +1,3 @@ fn main\n keep\n-old\n+new\n tail\n\
                   @@ -10 +10,2 @@\n x\n+y\n\\ No newline at end of file\n";
        let diff = parse_unified(out);
        assert!(!diff.binary);
        assert_eq!(diff.hunks.len(), 2);

        let h = &diff.hunks[0];
        assert_eq!(
            (h.old_start, h.old_lines, h.new_start, h.new_lines),
            (1, 3, 1, 3)
        );
        assert_eq!(h.header, "@@ -1,3 +1,3 @@ fn main");
        let lines: Vec<_> = h
            .lines
            .iter()
            .map(|l| (l.kind, l.old_line, l.new_line, l.text.as_str()))
            .collect();
        assert_eq!(
            lines,
            [
                (LineKind::Context, Some(1), Some(1), "keep"),
                (LineKind::Removed, Some(2), None, "old"),
                (LineKind::Added, None, Some(2), "new"),
                (LineKind::Context, Some(3), Some(3), "tail"),
            ]
        );

        let h = &diff.hunks[1];
        assert_eq!((h.old_start, h.old_lines), (10, 1));
        assert!(h.lines[1].no_newline);
        assert!(!h.lines[0].no_newline);
    }

    #[test]
    fn detects_binary_files() {
        let out =
            "diff --git a/i.png b/i.png\nindex 1..2\nBinary files a/i.png and b/i.png differ\n";
        let diff = parse_unified(out);
        assert!(diff.binary);
        assert!(diff.hunks.is_empty());
    }
}
