//! Submodules: adding them, updating them, and their checkout state.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::cli::operand;
use crate::repo::workdir;
use crate::{GitCli, Progress, Result};

/// How a submodule's checkout compares with the commit the superproject
/// records for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SubmoduleState {
    /// Not cloned yet.
    Uninitialized,
    /// At the recorded commit.
    Current,
    /// At a different commit.
    Moved,
    /// The recorded commit is in conflict.
    Conflicted,
}

/// Adds the repository at `url` as a submodule at `sub_path` (relative to
/// the working tree), cloning it. The new `.gitmodules` entry and the
/// submodule are staged.
pub fn add_submodule(
    git: &GitCli,
    path: &Path,
    url: &str,
    sub_path: &str,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let url = operand(url.trim())?;
    let sub_path = sub_path.trim().trim_end_matches(['/', '\\']);
    let sub_path = operand(sub_path)?;
    git.run_streaming(
        &workdir(path)?,
        ["submodule", "add", "--progress", "--", url, sub_path],
        on_progress,
    )?
    .check(&[0])?;
    Ok(())
}

/// Clones missing submodules and checks each out at its recorded commit,
/// recursively. Updates `paths` only, or every submodule when empty.
pub fn update_submodules(
    git: &GitCli,
    path: &Path,
    paths: &[String],
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let mut args: Vec<&str> = vec![
        "submodule",
        "update",
        "--init",
        "--recursive",
        "--progress",
        "--",
    ];
    for p in paths {
        args.push(operand(p)?);
    }
    git.run_streaming(&workdir(path)?, args, on_progress)?
        .check(&[0])?;
    Ok(())
}

/// The state of each submodule, keyed by path.
pub(crate) fn submodule_states(
    git: &GitCli,
    path: &Path,
) -> Result<HashMap<String, SubmoduleState>> {
    let out = git.run(&workdir(path)?, ["submodule", "status"])?;
    Ok(parse_status(&out))
}

/// Parses `git submodule status`: `<flag><oid> <path>[ (<describe>)]`.
fn parse_status(output: &str) -> HashMap<String, SubmoduleState> {
    output
        .lines()
        .filter_map(|line| {
            let mut chars = line.chars();
            let state = match chars.next()? {
                '-' => SubmoduleState::Uninitialized,
                '+' => SubmoduleState::Moved,
                'U' => SubmoduleState::Conflicted,
                _ => SubmoduleState::Current,
            };
            let (_, rest) = chars.as_str().split_once(' ')?;
            // The describe suffix only follows checked-out submodules.
            let path = match rest.rfind(" (") {
                Some(i) if rest.ends_with(')') => &rest[..i],
                _ => rest,
            };
            Some((path.to_owned(), state))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_submodule_status() {
        let out = " 1111111111111111111111111111111111111111 lib/a (v1.0)\n\
                   -2222222222222222222222222222222222222222 lib/b\n\
                   +3333333333333333333333333333333333333333 lib/c d (heads/main)\n\
                   U4444444444444444444444444444444444444444 lib/e\n";
        let states = parse_status(out);
        assert_eq!(states["lib/a"], SubmoduleState::Current);
        assert_eq!(states["lib/b"], SubmoduleState::Uninitialized);
        assert_eq!(states["lib/c d"], SubmoduleState::Moved);
        assert_eq!(states["lib/e"], SubmoduleState::Conflicted);
    }
}
