use std::path::Path;

use serde::Deserialize;
use ts_rs::TS;

use crate::repo::discover;
use crate::{Error, Result};

/// What an "ignore" action covers, relative to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IgnoreScope {
    /// Just this file.
    File,
    /// Every file with this file's extension, anywhere.
    Extension,
    /// The folder containing this file.
    Folder,
}

/// The `.gitignore` pattern for `file_path` (relative to the repository
/// root), or `None` when the scope doesn't apply (no extension, or a file
/// at the root).
pub fn ignore_pattern(file_path: &str, scope: IgnoreScope) -> Option<String> {
    let file_path = file_path.trim_end_matches('/');
    match scope {
        IgnoreScope::File => Some(format!("/{}", escape(file_path))),
        IgnoreScope::Extension => {
            let name = file_path.rsplit('/').next()?;
            let (stem, ext) = name.rsplit_once('.')?;
            (!stem.is_empty() && !ext.is_empty()).then(|| format!("*.{}", escape(ext)))
        }
        IgnoreScope::Folder => {
            let (dir, _) = file_path.rsplit_once('/')?;
            Some(format!("/{}/", escape(dir)))
        }
    }
}

/// Appends the pattern for `file_path` to the `.gitignore` at the root of
/// the working tree, creating it if needed. Returns the pattern.
pub fn add_to_gitignore(path: &Path, file_path: &str, scope: IgnoreScope) -> Result<String> {
    let repo = discover(path)?;
    let workdir = repo.workdir().ok_or_else(|| Error::Bare(path.to_owned()))?;
    let pattern = ignore_pattern(file_path, scope)
        .ok_or_else(|| Error::Invalid(format!("cannot ignore {file_path} that way")))?;
    let file = workdir.join(".gitignore");
    let existing = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    if existing.lines().any(|l| l.trim_end() == pattern) {
        return Ok(pattern);
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&pattern);
    updated.push('\n');
    std::fs::write(&file, updated)?;
    Ok(pattern)
}

/// Escapes glob characters so a name matches only itself.
fn escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | '!' | '#') {
            out.push('\\');
        }
        out.push(c);
    }
    // Trailing spaces are dropped unless escaped.
    let trimmed = out.trim_end_matches(' ');
    let spaces = out.len() - trimmed.len();
    let mut escaped = trimmed.to_owned();
    for _ in 0..spaces {
        escaped.push_str("\\ ");
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_patterns() {
        use IgnoreScope::*;
        assert_eq!(
            ignore_pattern("build/out.log", File).unwrap(),
            "/build/out.log"
        );
        assert_eq!(ignore_pattern("build/out.log", Extension).unwrap(), "*.log");
        assert_eq!(ignore_pattern("build/out.log", Folder).unwrap(), "/build/");
        assert_eq!(
            ignore_pattern("a b/[x]*.txt", File).unwrap(),
            "/a b/\\[x]\\*.txt"
        );
        assert_eq!(ignore_pattern("#notes ", File).unwrap(), "/\\#notes\\ ");
        assert_eq!(ignore_pattern("Makefile", Extension), None);
        assert_eq!(ignore_pattern(".env", Extension), None);
        assert_eq!(ignore_pattern("top.txt", Folder), None);
    }
}
