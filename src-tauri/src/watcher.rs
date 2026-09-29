//! Notices when something outside the app (a terminal, an IDE, a fetch)
//! changes a repository's refs, so the UI can refresh.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::notify::{self, RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

pub type RepoWatcher = Debouncer<RecommendedWatcher>;

/// Calls `on_change` (debounced) when HEAD, refs or config change.
pub fn watch(
    git_dir: &Path,
    common_dir: &Path,
    on_change: impl Fn() + Send + 'static,
) -> notify::Result<RepoWatcher> {
    let dirs = GitDirs {
        // Events report canonical paths on some platforms (e.g. /private/var on macOS).
        git_dir: git_dir
            .canonicalize()
            .unwrap_or_else(|_| git_dir.to_owned()),
        common_dir: common_dir
            .canonicalize()
            .unwrap_or_else(|_| common_dir.to_owned()),
    };
    let watched = dirs.common_dir.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        move |result: DebounceEventResult| {
            if let Ok(events) = result
                && events.iter().any(|e| dirs.is_relevant(&e.path))
            {
                on_change();
            }
        },
    )?;
    // The common dir contains the git dir of linked worktrees too.
    debouncer
        .watcher()
        .watch(&watched, RecursiveMode::Recursive)?;
    Ok(debouncer)
}

struct GitDirs {
    git_dir: PathBuf,
    common_dir: PathBuf,
}

impl GitDirs {
    fn is_relevant(&self, path: &Path) -> bool {
        // Lock files come and go during every ref update; the rename that
        // follows is what matters.
        if path.extension().is_some_and(|e| e == "lock") {
            return false;
        }
        let first = |base: &Path| {
            path.strip_prefix(base)
                .ok()
                .and_then(|rel| rel.components().next())
                .and_then(|c| c.as_os_str().to_str().map(str::to_owned))
        };
        if first(&self.git_dir).as_deref() == Some("HEAD") {
            return true;
        }
        matches!(
            first(&self.common_dir).as_deref(),
            Some("HEAD" | "refs" | "packed-refs" | "config")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ref_changes_are_relevant() {
        let dirs = GitDirs {
            git_dir: "/r/.git".into(),
            common_dir: "/r/.git".into(),
        };
        let relevant = |p: &str| dirs.is_relevant(Path::new(p));
        assert!(relevant("/r/.git/HEAD"));
        assert!(relevant("/r/.git/refs/heads/main"));
        assert!(relevant("/r/.git/packed-refs"));
        assert!(relevant("/r/.git/config"));
        assert!(!relevant("/r/.git/refs/heads/main.lock"));
        assert!(!relevant("/r/.git/objects/ab/cdef"));
        assert!(!relevant("/r/.git/index"));
        assert!(!relevant("/r/src/main.rs"));
    }

    #[test]
    fn linked_worktree_head_is_relevant() {
        let dirs = GitDirs {
            git_dir: "/r/.git/worktrees/wt".into(),
            common_dir: "/r/.git".into(),
        };
        assert!(dirs.is_relevant(Path::new("/r/.git/worktrees/wt/HEAD")));
        assert!(!dirs.is_relevant(Path::new("/r/.git/worktrees/wt/index")));
    }
}
