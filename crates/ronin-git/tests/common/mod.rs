// Shared by several test binaries; each uses only part of it.
#![allow(dead_code)]

use std::cell::Cell;
use std::path::Path;

use ronin_git::GitCli;
use tempfile::TempDir;

/// A throwaway repository isolated from the user's global and system git config.
pub struct TestRepo {
    pub dir: TempDir,
    pub git: GitCli,
    commits: Cell<i64>,
}

impl TestRepo {
    pub fn new() -> Self {
        let repo = Self::empty();
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    /// Clones `origin` into a new temp dir.
    pub fn clone_of(origin: &TestRepo) -> Self {
        let repo = Self::empty();
        let src = origin.path().to_str().unwrap();
        repo.git(&["clone", "-q", src, "."]);
        repo
    }

    fn empty() -> Self {
        let dir = TempDir::new().unwrap();
        let git = GitCli::discover()
            .unwrap()
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Ronin Test")
            .env("GIT_AUTHOR_EMAIL", "test@ronin.invalid")
            .env("GIT_COMMITTER_NAME", "Ronin Test")
            .env("GIT_COMMITTER_EMAIL", "test@ronin.invalid");
        Self {
            dir,
            git,
            commits: Cell::new(0),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn git(&self, args: &[&str]) -> String {
        self.git.run(self.path(), args).unwrap()
    }

    pub fn write(&self, name: &str, contents: &str) {
        let path = self.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    pub fn commit_file(&self, name: &str, contents: &str, message: &str) -> String {
        self.write(name, contents);
        self.git(&["add", name]);
        self.commit(&["-m", message]);
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Runs `git commit -q <args>` with a timestamp one minute after the
    /// previous commit, so date ordering is deterministic.
    pub fn commit(&self, args: &[&str]) {
        let n = self.commits.get() + 1;
        self.commits.set(n);
        let date = format!("{} +0000", 1_700_000_000 + n * 60);
        let git = self
            .git
            .clone()
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date);
        let mut full = vec!["commit", "-q"];
        full.extend_from_slice(args);
        git.run(self.path(), &full).unwrap();
    }
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}
