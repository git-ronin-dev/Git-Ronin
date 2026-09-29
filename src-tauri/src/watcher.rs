//! Notices when something outside the app (a terminal, an IDE, a fetch, an
//! editor saving a file) changes a repository, so the UI can refresh.
//!
//! Two kinds of change are reported: refs (HEAD, branches, tags, stashes,
//! config) and the working copy (the index and non-ignored files).
//! Ignored directories such as `node_modules` or `target` are not watched
//! on Linux, where every directory costs an inotify watch, and their events
//! are dropped elsewhere. Git's object store is never watched. Bursts of
//! events are debounced hard: builds and checkouts produce storms.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use ignore::WalkBuilder;
use notify::event::{AccessKind, AccessMode, CreateKind, MetadataKind, ModifyKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Quiet time after the last event before reporting.
const QUIET: Duration = Duration::from_millis(300);
/// Longest a report waits while events keep arriving.
const MAX_WAIT: Duration = Duration::from_secs(2);

/// Per-directory watches where they are cheap and recursion is not native.
const PER_DIRECTORY: bool = cfg!(any(target_os = "linux", target_os = "android"));

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Changes {
    pub refs: bool,
    pub worktree: bool,
}

impl Changes {
    fn any(self) -> bool {
        self.refs || self.worktree
    }
}

/// Stops watching when dropped.
pub struct RepoWatcher {
    tx: Sender<Msg>,
}

impl Drop for RepoWatcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

enum Msg {
    Event(notify::Result<Event>),
    Stop,
}

/// Calls `on_change` (debounced, on a background thread) when the
/// repository's refs or working copy change.
pub fn watch(
    git_dir: &Path,
    common_dir: &Path,
    workdir: Option<&Path>,
    on_change: impl Fn(Changes) + Send + 'static,
) -> notify::Result<RepoWatcher> {
    // Events report canonical paths on some platforms (e.g. /private/var on macOS).
    let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_owned());
    let paths = RepoPaths {
        git_dir: canonical(git_dir),
        common_dir: canonical(common_dir),
        workdir: workdir.map(canonical),
    };
    let (tx, rx) = mpsc::channel();
    let events = tx.clone();
    let watcher = notify::recommended_watcher(move |e| {
        let _ = events.send(Msg::Event(e));
    })?;

    std::thread::Builder::new()
        .name("repo-watcher".into())
        .spawn(move || {
            // Walking a large working tree takes a moment, so it happens here
            // rather than while the repository is being opened.
            let mut state = WatchState {
                paths,
                watcher,
                dirs: HashSet::new(),
            };
            state.watch_git_dirs();
            state.rescan();
            state.run(&rx, on_change);
        })?;
    Ok(RepoWatcher { tx })
}

struct RepoPaths {
    git_dir: PathBuf,
    common_dir: PathBuf,
    workdir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Refs,
    Worktree,
    /// A `.gitignore` or `info/exclude`: the set of watched directories may change.
    IgnoreRules,
}

impl RepoPaths {
    /// What a change to `path` means, if anything.
    fn classify(&self, path: &Path) -> Option<Kind> {
        let first = |base: &Path| {
            path.strip_prefix(base)
                .ok()
                .map(|rel| rel.components().map(|c| c.as_os_str()).collect::<Vec<_>>())
        };
        // Lock files come and go during every update; the rename that
        // follows is what matters.
        if path.extension().is_some_and(|e| e == "lock") {
            return None;
        }
        if let Some(rel) = first(&self.git_dir) {
            match rel.first().and_then(|c| c.to_str()) {
                Some("HEAD") => return Some(Kind::Refs),
                Some("index") => return Some(Kind::Worktree),
                _ => {}
            }
        }
        if let Some(rel) = first(&self.common_dir) {
            let parts: Vec<&str> = rel.iter().filter_map(|c| c.to_str()).collect();
            return match parts[..] {
                ["HEAD" | "packed-refs" | "config", ..] | ["refs", ..] => Some(Kind::Refs),
                // Dropping an older stash only rewrites its reflog.
                ["logs", "refs", "stash"] => Some(Kind::Refs),
                ["info", "exclude"] => Some(Kind::IgnoreRules),
                _ => None,
            };
        }
        if self.is_git_path(path) {
            return None;
        }
        let rel = first(self.workdir.as_deref()?)?;
        if rel.iter().any(|c| *c == ".git") {
            return None;
        }
        if rel.last().is_some_and(|c| *c == ".gitignore") {
            return Some(Kind::IgnoreRules);
        }
        Some(Kind::Worktree)
    }

    fn is_git_path(&self, path: &Path) -> bool {
        path.starts_with(&self.git_dir) || path.starts_with(&self.common_dir)
    }
}

struct WatchState {
    paths: RepoPaths,
    watcher: RecommendedWatcher,
    /// Non-ignored directories of the working tree.
    dirs: HashSet<PathBuf>,
}

impl WatchState {
    fn watch_git_dirs(&mut self) {
        let RepoPaths {
            git_dir,
            common_dir,
            ..
        } = &self.paths;
        if PER_DIRECTORY {
            // Everything that matters except objects/, whose fan-out alone
            // would cost 256 watches. Missing directories are fine.
            let mut targets = vec![
                (git_dir.clone(), RecursiveMode::NonRecursive),
                (common_dir.clone(), RecursiveMode::NonRecursive),
                (common_dir.join("refs"), RecursiveMode::Recursive),
                (common_dir.join("logs/refs"), RecursiveMode::NonRecursive),
                (common_dir.join("info"), RecursiveMode::NonRecursive),
            ];
            targets.dedup_by(|a, b| a.0 == b.0);
            for (dir, mode) in targets {
                let _ = self.watcher.watch(&dir, mode);
            }
        } else {
            // The common dir contains the git dir of linked worktrees too.
            let _ = self.watcher.watch(common_dir, RecursiveMode::Recursive);
            if let Some(workdir) = &self.paths.workdir {
                let _ = self.watcher.watch(workdir, RecursiveMode::Recursive);
            }
        }
    }

    /// Finds the non-ignored directories again, watching new ones and
    /// forgetting those that are gone or now ignored.
    fn rescan(&mut self) {
        let Some(workdir) = self.paths.workdir.clone() else {
            return;
        };
        let found: HashSet<PathBuf> = walk_dirs(&workdir, None).into_iter().collect();
        for gone in self.dirs.difference(&found) {
            if PER_DIRECTORY {
                let _ = self.watcher.unwatch(gone);
            }
        }
        let added: Vec<PathBuf> = found.difference(&self.dirs).cloned().collect();
        self.dirs = found;
        if PER_DIRECTORY {
            for dir in added {
                let _ = self.watcher.watch(&dir, RecursiveMode::NonRecursive);
            }
        }
    }

    /// Picks up directories created (or moved) inside `parent`.
    fn scan_children(&mut self, parent: &Path) {
        if !self.dirs.contains(parent) {
            return;
        }
        for child in walk_dirs(parent, Some(1)) {
            if child == parent || self.dirs.contains(&child) {
                continue;
            }
            for dir in walk_dirs(&child, None) {
                if PER_DIRECTORY {
                    let _ = self.watcher.watch(&dir, RecursiveMode::NonRecursive);
                }
                self.dirs.insert(dir);
            }
        }
    }

    fn run(&mut self, rx: &mpsc::Receiver<Msg>, on_change: impl Fn(Changes)) {
        let mut pending = Changes::default();
        let mut first_at = Instant::now();
        let mut last_at = Instant::now();
        loop {
            let timeout = if pending.any() {
                let due = (last_at + QUIET).min(first_at + MAX_WAIT);
                due.saturating_duration_since(Instant::now())
            } else {
                Duration::from_secs(3600)
            };
            let changes = match rx.recv_timeout(timeout) {
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {
                    if pending.any() {
                        on_change(std::mem::take(&mut pending));
                    }
                    continue;
                }
                Ok(Msg::Event(Ok(event))) => self.handle(&event),
                // E.g. the kernel's event queue overflowed: assume the worst.
                Ok(Msg::Event(Err(_))) => Changes {
                    refs: true,
                    worktree: true,
                },
            };
            if changes.any() {
                if !pending.any() {
                    first_at = Instant::now();
                }
                last_at = Instant::now();
                pending.refs |= changes.refs;
                pending.worktree |= changes.worktree;
            }
        }
    }

    fn handle(&mut self, event: &Event) -> Changes {
        let mut changes = Changes::default();
        if !is_modification(&event.kind) {
            return changes;
        }
        let mut rescan = false;
        for path in &event.paths {
            match self.paths.classify(path) {
                Some(Kind::Refs) => changes.refs = true,
                Some(Kind::IgnoreRules) => {
                    rescan = true;
                    changes.worktree = true;
                }
                Some(Kind::Worktree) => {
                    let parent = path.parent().unwrap_or(path);
                    // Anything inside an ignored directory is noise.
                    if !self.dirs.contains(parent) && !self.dirs.contains(path) {
                        continue;
                    }
                    changes.worktree = true;
                    if may_add_directory(&event.kind) && path.is_dir() {
                        self.scan_children(parent);
                    }
                    if matches!(event.kind, EventKind::Remove(_)) {
                        self.dirs.remove(path);
                    }
                }
                None => {}
            }
        }
        if rescan {
            self.rescan();
        }
        changes
    }
}

/// Reads (including our own `git status`) are not changes.
fn is_modification(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)) => false,
        _ => true,
    }
}

fn may_add_directory(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(CreateKind::Folder | CreateKind::Any)
            | EventKind::Modify(ModifyKind::Name(_) | ModifyKind::Any)
            | EventKind::Any
    )
}

/// Non-ignored directories under `root`, `root` included, honouring
/// `.gitignore` files (including those above `root`), `info/exclude` and
/// the global excludes file.
fn walk_dirs(root: &Path, max_depth: Option<usize>) -> Vec<PathBuf> {
    WalkBuilder::new(root)
        .hidden(false)
        // `.ignore` files are a ripgrep convention; git doesn't read them.
        .ignore(false)
        .require_git(false)
        .max_depth(max_depth)
        .filter_entry(|e| e.file_type().is_some_and(|t| t.is_dir()) && e.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(git_dir: &str, common_dir: &str) -> RepoPaths {
        RepoPaths {
            git_dir: git_dir.into(),
            common_dir: common_dir.into(),
            workdir: Some("/r".into()),
        }
    }

    #[test]
    fn classifies_git_dir_changes() {
        let p = paths("/r/.git", "/r/.git");
        let kind = |s: &str| p.classify(Path::new(s));
        assert_eq!(kind("/r/.git/HEAD"), Some(Kind::Refs));
        assert_eq!(kind("/r/.git/refs/heads/main"), Some(Kind::Refs));
        assert_eq!(kind("/r/.git/packed-refs"), Some(Kind::Refs));
        assert_eq!(kind("/r/.git/config"), Some(Kind::Refs));
        assert_eq!(kind("/r/.git/logs/refs/stash"), Some(Kind::Refs));
        assert_eq!(kind("/r/.git/index"), Some(Kind::Worktree));
        assert_eq!(kind("/r/.git/info/exclude"), Some(Kind::IgnoreRules));
        assert_eq!(kind("/r/.git/refs/heads/main.lock"), None);
        assert_eq!(kind("/r/.git/index.lock"), None);
        assert_eq!(kind("/r/.git/objects/ab/cdef"), None);
        assert_eq!(kind("/r/.git/logs/HEAD"), None);
    }

    #[test]
    fn classifies_working_tree_changes() {
        let p = paths("/r/.git", "/r/.git");
        let kind = |s: &str| p.classify(Path::new(s));
        assert_eq!(kind("/r/src/main.rs"), Some(Kind::Worktree));
        assert_eq!(kind("/r/sub/.gitignore"), Some(Kind::IgnoreRules));
        assert_eq!(kind("/r/vendor/lib/.git/HEAD"), None);
        assert_eq!(kind("/elsewhere/file"), None);
    }

    #[test]
    fn linked_worktree_uses_its_own_head_and_index() {
        let p = paths("/r/.git/worktrees/wt", "/r/.git");
        let kind = |s: &str| p.classify(Path::new(s));
        assert_eq!(kind("/r/.git/worktrees/wt/HEAD"), Some(Kind::Refs));
        assert_eq!(kind("/r/.git/worktrees/wt/index"), Some(Kind::Worktree));
        // Another worktree's index is not ours.
        assert_eq!(kind("/r/.git/worktrees/other/index"), None);
    }

    #[test]
    fn walk_skips_ignored_directories() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        for d in ["src/deep", "target/debug", "node_modules/x", ".git/objects"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join(".gitignore"), "/target\nnode_modules/\n").unwrap();
        let mut found: Vec<String> = walk_dirs(root, None)
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, ["", "src", "src/deep"]);
    }

    #[test]
    fn reads_are_not_modifications() {
        assert!(!is_modification(&EventKind::Access(AccessKind::Open(
            notify::event::AccessMode::Any
        ))));
        assert!(!is_modification(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(is_modification(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
        assert!(is_modification(&EventKind::Create(CreateKind::File)));
    }

    #[test]
    fn reports_real_changes_only() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(root.join(".gitignore"), "/target\n").unwrap();
        std::fs::create_dir(root.join("target")).unwrap();

        let (tx, rx) = mpsc::channel();
        let git_dir = root.join(".git");
        let _watcher = watch(&git_dir, &git_dir, Some(&root), move |c| {
            let _ = tx.send(c);
        })
        .unwrap();
        // Let the initial scan finish.
        std::thread::sleep(Duration::from_millis(500));
        let wait = || rx.recv_timeout(Duration::from_millis(1500)).ok();
        let worktree = Some(Changes {
            refs: false,
            worktree: true,
        });

        let _ = std::fs::read(root.join(".gitignore")).unwrap();
        std::fs::write(root.join("target/build.o"), "x").unwrap();
        assert_eq!(wait(), None, "reads and ignored files are not changes");

        std::fs::write(root.join("file.txt"), "x").unwrap();
        assert_eq!(wait(), worktree);

        std::fs::create_dir(root.join("sub")).unwrap();
        assert_eq!(wait(), worktree);
        std::fs::write(root.join("sub/new.txt"), "x").unwrap();
        assert_eq!(wait(), worktree, "new directories are watched");

        std::fs::write(git_dir.join("refs/heads/topic"), "0".repeat(40)).unwrap();
        assert_eq!(
            wait(),
            Some(Changes {
                refs: true,
                worktree: false
            })
        );
    }
}
