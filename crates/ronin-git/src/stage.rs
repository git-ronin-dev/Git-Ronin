//! Moving changes between the working tree and the index: whole files, or
//! chosen hunks and lines applied as partial patches.

use std::fmt::Write;
use std::path::{Component, Path};

use serde::Deserialize;
use ts_rs::TS;

use crate::repo::discover;
use crate::{Error, FileStatus, GitCli, Hunk, LineKind, Result, StatusEntry};

/// Where a partial patch goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PatchTarget {
    /// Lines from the unstaged diff into the index.
    Stage,
    /// Lines from the staged diff back out of the index.
    Unstage,
    /// Lines from the unstaged diff removed from the working tree.
    Discard,
}

/// Chosen lines of one hunk, as indices into its `lines`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LineSelection {
    pub hunk: u32,
    pub lines: Vec<u32>,
}

/// Stages whole files: modifications, deletions and untracked files alike.
/// Staging a conflicted file marks it resolved.
pub fn stage_files(git: &GitCli, path: &Path, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    with_pathspecs(git, path, &["add", "-A"], paths)
}

/// Unstages whole files. Pass both paths of a staged rename.
pub fn unstage_files(git: &GitCli, path: &Path, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    // Unlike `restore --staged`, `reset` also works before the first commit.
    with_pathspecs(git, path, &["reset", "-q"], paths)
}

/// Throws away working-tree changes: tracked files go back to their staged
/// (or committed) content, untracked files are deleted.
pub fn discard_files(git: &GitCli, path: &Path, entries: &[StatusEntry]) -> Result<()> {
    let workdir = workdir(path)?;
    let (untracked, tracked): (Vec<&StatusEntry>, Vec<&StatusEntry>) = entries
        .iter()
        .partition(|e| e.status == FileStatus::Untracked);
    let tracked: Vec<String> = tracked.iter().map(|e| e.path.clone()).collect();
    if !tracked.is_empty() {
        with_pathspecs(git, &workdir, &["restore", "--worktree"], &tracked)?;
    }
    for entry in untracked {
        let relative = Path::new(&entry.path);
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(Error::Invalid(format!("unexpected path: {}", entry.path)));
        }
        let file = workdir.join(relative);
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}

/// Applies the chosen lines of a file's diff. `hunks` must be the unstaged
/// diff for [`PatchTarget::Stage`] and [`PatchTarget::Discard`], and the
/// staged one for [`PatchTarget::Unstage`], computed without `-w`. If the
/// file changed since, git refuses the patch and nothing happens.
pub fn apply_lines(
    git: &GitCli,
    path: &Path,
    file_path: &str,
    hunks: &[Hunk],
    selection: &[LineSelection],
    target: PatchTarget,
) -> Result<()> {
    let reverse = target != PatchTarget::Stage;
    let Some(patch) = build_patch(file_path, hunks, selection, reverse) else {
        return Ok(());
    };
    let mut args = vec!["apply", "--whitespace=nowarn"];
    if target != PatchTarget::Discard {
        args.push("--cached");
    }
    if reverse {
        args.push("-R");
    }
    args.push("-");
    git.run_with_input(&workdir(path)?, args, patch.as_bytes())?;
    Ok(())
}

/// Builds a patch containing only the chosen lines, or `None` if it would
/// change nothing.
///
/// The patch keeps one side of the file as it is: for a forward patch the
/// old side (what it applies to), for a `reverse` one the new side. Lines
/// left out that exist on that side become context; the rest are dropped.
pub fn build_patch(
    file_path: &str,
    hunks: &[Hunk],
    selection: &[LineSelection],
    reverse: bool,
) -> Option<String> {
    let mut body = String::new();
    // How far the rewritten side has drifted from the kept side, through
    // the hunks written so far.
    let mut shift: i64 = 0;
    for (index, hunk) in hunks.iter().enumerate() {
        let chosen: Vec<u32> = selection
            .iter()
            .filter(|s| s.hunk as usize == index)
            .flat_map(|s| s.lines.iter().copied())
            .collect();
        let mut lines = String::new();
        let (mut old_count, mut new_count, mut changes) = (0u32, 0u32, 0u32);
        for (i, line) in hunk.lines.iter().enumerate() {
            let picked = chosen.contains(&(i as u32));
            let prefix = match (line.kind, picked) {
                (LineKind::Context, _) => ' ',
                (LineKind::Added, true) => '+',
                (LineKind::Removed, true) => '-',
                // Present on the side that stays: keep it as context.
                (LineKind::Added, false) if reverse => ' ',
                (LineKind::Removed, false) if !reverse => ' ',
                _ => continue,
            };
            match prefix {
                ' ' => {
                    old_count += 1;
                    new_count += 1;
                }
                '+' => {
                    new_count += 1;
                    changes += 1;
                }
                _ => {
                    old_count += 1;
                    changes += 1;
                }
            }
            lines.push(prefix);
            lines.push_str(&line.text);
            lines.push('\n');
            if line.no_newline {
                lines.push_str("\\ No newline at end of file\n");
            }
        }
        if changes == 0 {
            continue;
        }
        let (old_start, new_start) = if reverse {
            let old_start = shifted(hunk.new_start, new_count, old_count, shift);
            (old_start, i64::from(hunk.new_start))
        } else {
            let new_start = shifted(hunk.old_start, old_count, new_count, shift);
            (i64::from(hunk.old_start), new_start)
        };
        shift += if reverse {
            i64::from(old_count) - i64::from(new_count)
        } else {
            i64::from(new_count) - i64::from(old_count)
        };
        let _ = writeln!(
            body,
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@"
        );
        body.push_str(&lines);
    }
    if body.is_empty() {
        return None;
    }
    let a = quote_path(&format!("a/{file_path}"));
    let b = quote_path(&format!("b/{file_path}"));
    Some(format!("diff --git {a} {b}\n--- {a}\n+++ {b}\n{body}"))
}

/// Start of the rewritten side of a hunk whose kept side starts at `start`
/// with `count` lines, after earlier hunks moved lines by `shift`. An empty
/// range names the line before it, hence the adjustments.
fn shifted(start: u32, count: u32, other_count: u32, shift: i64) -> i64 {
    let first = i64::from(start) + i64::from(count == 0) + shift;
    first - i64::from(other_count == 0)
}

/// Quotes a path the way git does when it contains special characters.
fn quote_path(path: &str) -> String {
    if !path
        .chars()
        .any(|c| c == '"' || c == '\\' || c.is_control())
    {
        return path.to_owned();
    }
    let mut quoted = String::from("\"");
    for c in path.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(quoted, "\\{:03o}", c as u32);
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

fn workdir(path: &Path) -> Result<std::path::PathBuf> {
    let repo = discover(path)?;
    repo.workdir()
        .map(Path::to_owned)
        .ok_or_else(|| Error::Bare(path.to_owned()))
}

/// Runs `git <args>` on literal pathspecs passed through stdin, which has
/// no command-line length limit.
fn with_pathspecs(git: &GitCli, path: &Path, args: &[&str], paths: &[String]) -> Result<()> {
    let mut full = vec!["--literal-pathspecs"];
    full.extend_from_slice(args);
    full.extend(["--pathspec-from-file=-", "--pathspec-file-nul"]);
    let mut input = paths.join("\0");
    input.push('\0');
    git.run_with_input(&workdir(path)?, full, input.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_unified;

    const TWO_HUNKS: &str = "@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n@@ -10,2 +10,3 @@\n x\n+y\n z\n";

    fn pick(hunk: u32, lines: &[u32]) -> LineSelection {
        LineSelection {
            hunk,
            lines: lines.to_vec(),
        }
    }

    fn body(patch: &str) -> &str {
        patch.split_once("+++ b/f\n").unwrap().1
    }

    #[test]
    fn forward_patch_keeps_old_side_and_shifts_later_hunks() {
        let hunks = parse_unified(TWO_HUNKS).hunks;
        let patch = build_patch("f", &hunks, &[pick(0, &[2]), pick(1, &[1])], false).unwrap();
        assert!(patch.starts_with("diff --git a/f b/f\n--- a/f\n+++ b/f\n"));
        assert_eq!(
            body(&patch),
            "@@ -1,3 +1,4 @@\n a\n b\n+B\n c\n@@ -10,2 +11,3 @@\n x\n+y\n z\n"
        );
    }

    #[test]
    fn reverse_patch_keeps_new_side() {
        let hunks = parse_unified(TWO_HUNKS).hunks;
        // The first hunk contributes nothing and is left out.
        let patch = build_patch("f", &hunks, &[pick(1, &[1])], true).unwrap();
        assert_eq!(body(&patch), "@@ -10,2 +10,3 @@\n x\n+y\n z\n");

        let patch = build_patch("f", &hunks, &[pick(0, &[1]), pick(1, &[1])], true).unwrap();
        assert_eq!(
            body(&patch),
            "@@ -1,4 +1,3 @@\n a\n-b\n B\n c\n@@ -11,2 +10,3 @@\n x\n+y\n z\n"
        );
    }

    #[test]
    fn empty_ranges_and_missing_newlines() {
        let hunks = parse_unified("@@ -0,0 +1,2 @@\n+a\n+b\n\\ No newline at end of file\n").hunks;
        let patch = build_patch("f", &hunks, &[pick(0, &[1])], false).unwrap();
        assert_eq!(
            body(&patch),
            "@@ -0,0 +1,1 @@\n+b\n\\ No newline at end of file\n"
        );

        let hunks = parse_unified("@@ -3,2 +2,0 @@\n-c\n-d\n").hunks;
        let patch = build_patch("f", &hunks, &[pick(0, &[0, 1])], false).unwrap();
        assert_eq!(body(&patch), "@@ -3,2 +2,0 @@\n-c\n-d\n");
    }

    #[test]
    fn nothing_selected_is_no_patch() {
        let hunks = parse_unified(TWO_HUNKS).hunks;
        assert_eq!(build_patch("f", &hunks, &[], false), None);
        assert_eq!(build_patch("f", &hunks, &[pick(0, &[0, 3])], true), None);
    }

    #[test]
    fn quotes_unusual_paths() {
        assert_eq!(quote_path("a/plain name.txt"), "a/plain name.txt");
        assert_eq!(quote_path("a/tab\there\"q"), "\"a/tab\\there\\\"q\"");
    }
}
