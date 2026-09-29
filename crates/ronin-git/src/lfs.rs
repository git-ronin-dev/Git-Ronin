//! Git LFS, through the `git lfs` command: tracked patterns and file locks.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cli::operand;
use crate::repo::workdir;
use crate::{Error, GitCli, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LfsStatus {
    /// `git lfs` is available.
    pub installed: bool,
    /// Its filters are configured, so tracked files are actually stored in LFS.
    pub initialized: bool,
    pub patterns: Vec<LfsPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LfsPattern {
    /// Relative to the repository root.
    pub pattern: String,
    /// The `.gitattributes` file it comes from.
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    pub owner: String,
    /// RFC 3339.
    pub locked_at: String,
    /// Held by the current user; `None` when the server couldn't say.
    pub ours: Option<bool>,
}

/// Whether LFS is installed and set up here, and the patterns it tracks.
pub fn lfs_status(git: &GitCli, path: &Path) -> Result<LfsStatus> {
    let workdir = workdir(path)?;
    let installed = git
        .run_raw(&workdir, ["lfs", "version"])
        .is_ok_and(|f| f.success());
    if !installed {
        return Ok(LfsStatus::default());
    }
    let initialized = git
        .run_raw(&workdir, ["config", "--get", "filter.lfs.process"])
        .is_ok_and(|f| f.success());
    let out = git.run(&workdir, ["lfs", "track", "--no-excluded"])?;
    Ok(LfsStatus {
        installed,
        initialized,
        patterns: parse_track(&out),
    })
}

/// Sets LFS up for this repository only: its filters and hooks.
pub fn lfs_init(git: &GitCli, path: &Path) -> Result<()> {
    git.run(&workdir(path)?, ["lfs", "install", "--local"])?;
    Ok(())
}

/// Stores files matching `pattern` in LFS from now on, via the root
/// `.gitattributes`.
pub fn lfs_track(git: &GitCli, path: &Path, pattern: &str) -> Result<()> {
    let pattern = operand(pattern.trim())?;
    git.run(&workdir(path)?, ["lfs", "track", "--", pattern])?;
    Ok(())
}

/// Stops tracking `pattern`, as listed by [`lfs_status`] with its `source`.
pub fn lfs_untrack(git: &GitCli, path: &Path, pattern: &str, source: &str) -> Result<()> {
    let workdir = workdir(path)?;
    // Patterns from a nested `.gitattributes` are listed with its
    // directory in front, and must be removed from there.
    let dir = source.strip_suffix(".gitattributes").unwrap_or("");
    let local = pattern.strip_prefix(dir).unwrap_or(pattern);
    let cwd = workdir.join(dir);
    if !cwd.starts_with(&workdir) || dir.contains("..") {
        return Err(Error::Invalid(format!("unexpected source: {source}")));
    }
    git.run(&cwd, ["lfs", "untrack", "--", operand(local)?])?;
    Ok(())
}

/// Locks on the remote LFS server.
pub fn lfs_locks(git: &GitCli, path: &Path) -> Result<Vec<LfsLock>> {
    let workdir = workdir(path)?;
    // Telling our locks from others' needs a server that supports it.
    let verified = git.run_raw(&workdir, ["lfs", "locks", "--verify", "--json"])?;
    if verified.success() {
        #[derive(Deserialize)]
        struct Verified {
            #[serde(default)]
            ours: Vec<RawLock>,
            #[serde(default)]
            theirs: Vec<RawLock>,
        }
        let v: Verified = parse_json(&verified.stdout)?;
        let ours = v.ours.into_iter().map(|l| l.into_lock(Some(true)));
        let theirs = v.theirs.into_iter().map(|l| l.into_lock(Some(false)));
        return Ok(ours.chain(theirs).collect());
    }
    let out = git.run(&workdir, ["lfs", "locks", "--json"])?;
    let locks: Vec<RawLock> = parse_json(&out)?;
    Ok(locks.into_iter().map(|l| l.into_lock(None)).collect())
}

/// Locks `file` on the LFS server so others can't push changes to it.
pub fn lfs_lock(git: &GitCli, path: &Path, file: &str) -> Result<()> {
    git.run(&workdir(path)?, ["lfs", "lock", "--", operand(file)?])?;
    Ok(())
}

/// Releases the lock with `id`; `force` releases someone else's.
pub fn lfs_unlock(git: &GitCli, path: &Path, id: &str, force: bool) -> Result<()> {
    let id = format!("--id={}", operand(id)?);
    let mut args = vec!["lfs", "unlock", id.as_str()];
    if force {
        args.push("--force");
    }
    git.run(&workdir(path)?, args)?;
    Ok(())
}

#[derive(Deserialize)]
struct RawLock {
    id: String,
    path: String,
    #[serde(default)]
    owner: Option<Owner>,
    #[serde(default)]
    locked_at: String,
}

#[derive(Deserialize)]
struct Owner {
    name: String,
}

impl RawLock {
    fn into_lock(self, ours: Option<bool>) -> LfsLock {
        LfsLock {
            id: self.id,
            path: self.path,
            owner: self.owner.map(|o| o.name).unwrap_or_default(),
            locked_at: self.locked_at,
            ours,
        }
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T> {
    serde_json::from_str(text.trim())
        .map_err(|e| Error::Invalid(format!("unexpected output from git lfs: {e}")))
}

/// Parses `git lfs track` output: indented `<pattern> (<source>)` lines
/// under "Listing tracked patterns".
fn parse_track(output: &str) -> Vec<LfsPattern> {
    let mut tracked = false;
    output
        .lines()
        .filter_map(|line| {
            if !line.starts_with(' ') {
                tracked = line.starts_with("Listing tracked");
                return None;
            }
            let line = line.trim();
            let (pattern, source) = line.strip_suffix(')')?.rsplit_once(" (")?;
            tracked.then(|| LfsPattern {
                pattern: pattern.to_owned(),
                source: source.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tracked_patterns() {
        let out = "Listing tracked patterns\n    sub/*.zip (sub/.gitattributes)\n    \
                   *.psd (.gitattributes)\nListing excluded patterns\n    *.txt (.gitattributes)\n";
        let patterns = parse_track(out);
        assert_eq!(
            patterns,
            [
                LfsPattern {
                    pattern: "sub/*.zip".into(),
                    source: "sub/.gitattributes".into()
                },
                LfsPattern {
                    pattern: "*.psd".into(),
                    source: ".gitattributes".into()
                },
            ]
        );
    }

    #[test]
    fn parses_locks() {
        let json = r#"[{"id":"7","path":"a.psd","owner":{"name":"Jane"},"locked_at":"2026-01-02T03:04:05Z"}]"#;
        let locks: Vec<RawLock> = parse_json(json).unwrap();
        let lock = locks.into_iter().next().unwrap().into_lock(Some(true));
        assert_eq!(lock.owner, "Jane");
        assert_eq!(lock.path, "a.psd");
        assert_eq!(lock.ours, Some(true));
    }
}
