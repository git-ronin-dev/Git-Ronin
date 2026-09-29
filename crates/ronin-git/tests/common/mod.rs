use std::path::Path;

use ronin_git::GitCli;
use tempfile::TempDir;

/// A throwaway repository isolated from the user's global and system git config.
pub struct TestRepo {
    pub dir: TempDir,
    pub git: GitCli,
}

impl TestRepo {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let git = GitCli::discover()
            .unwrap()
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Ronin Test")
            .env("GIT_AUTHOR_EMAIL", "test@ronin.invalid")
            .env("GIT_COMMITTER_NAME", "Ronin Test")
            .env("GIT_COMMITTER_EMAIL", "test@ronin.invalid");
        let repo = Self { dir, git };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn git(&self, args: &[&str]) -> String {
        self.git.run(self.path(), args).unwrap()
    }

    pub fn commit_file(&self, name: &str, contents: &str, message: &str) -> String {
        std::fs::write(self.path().join(name), contents).unwrap();
        self.git(&["add", name]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}
