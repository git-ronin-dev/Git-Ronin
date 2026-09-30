//! Opt-in sync of the portable settings through a git repository the user
//! owns.
//!
//! The config directory itself becomes the repository, but only
//! `portable.toml` (and the `.gitignore` saying so) is ever committed.
//! Changes are committed locally as they happen and pushed now and then;
//! pulling merges the settings key by key ([`merge::merge3`]), so two
//! machines changing different settings never conflict. A setting both
//! changed differently is left for the user to choose.

use std::fs;
use std::path::Path;

use ronin_git::GitCli;
use serde::Serialize;
use toml::Value;
use ts_rs::TS;

use crate::merge::{self, Change, Conflict};
use crate::secrets::find_secret;
use crate::{Error, PORTABLE_FILE, Portable, Result, SyncSettings, write_text_atomic};

const REMOTE: &str = "origin";
const IGNORE: &str =
    "# Git Ronin syncs only the portable settings.\n/*\n!/.gitignore\n!/portable.toml\n";

/// Identity and switches for the sync repository's own commits: they must
/// not depend on (or prompt through) the user's signing or hook setup.
const COMMIT: &[&str] = &[
    "-c",
    "user.name=Git Ronin",
    "-c",
    "user.email=git-ronin@localhost",
    "-c",
    "commit.gpgSign=false",
];

/// What the UI shows about sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncStatus {
    /// `None` while sync is off.
    pub settings: Option<SyncSettings>,
    pub running: bool,
    /// When the last sync finished, in seconds since the Unix epoch.
    #[ts(type = "number | null")]
    pub last_sync: Option<u64>,
    pub error: Option<String>,
    /// Settings changed differently here (`current`) and on another
    /// machine (`incoming`), waiting for a choice.
    pub conflicts: Vec<Change>,
}

pub fn is_enabled(dir: &Path) -> bool {
    dir.join(".git").is_dir()
}

/// Makes `dir` a repository syncing to `remote`. Existing history (from an
/// earlier enable) is kept; the remote is replaced.
pub fn enable(git: &GitCli, dir: &Path, remote: &str, branch: &str) -> Result<()> {
    check_branch(git, dir, branch)?;
    if remote.trim().is_empty() || remote.starts_with('-') {
        return Err(Error::Invalid(format!("invalid remote: “{remote}”")));
    }
    fs::create_dir_all(dir).map_err(|source| Error::Write {
        path: dir.to_owned(),
        source,
    })?;
    if !is_enabled(dir) {
        git.run(dir, ["init", "--quiet", "--initial-branch", branch])?;
    }
    write_text_atomic(&dir.join(".gitignore"), IGNORE)?;
    let has_remote = git.run_raw(dir, ["remote", "get-url", REMOTE])?.success();
    let verb = if has_remote { "set-url" } else { "add" };
    git.run(dir, ["remote", verb, REMOTE, remote.trim()])?;
    Ok(())
}

/// Stops syncing: `dir` is no longer a repository. The remote is untouched.
pub fn disable(dir: &Path) -> Result<()> {
    let remove_err = |source| Error::Write {
        path: dir.join(".git"),
        source,
    };
    if is_enabled(dir) {
        make_writable(&dir.join(".git"));
        fs::remove_dir_all(dir.join(".git")).map_err(remove_err)?;
    }
    match fs::remove_file(dir.join(".gitignore")) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(remove_err(e)),
        _ => Ok(()),
    }
}

/// git makes object files read-only, which Windows refuses to delete.
fn make_writable(path: &Path) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            make_writable(&path);
        } else if let Ok(meta) = entry.metadata() {
            let mut perms = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = fs::set_permissions(&path, perms);
        }
    }
}

/// Commits the current settings if they changed. Refuses settings that look
/// like they contain a secret.
pub fn commit(git: &GitCli, dir: &Path) -> Result<bool> {
    guard(dir)?;
    git.run(dir, ["add", "--all"])?;
    let staged = git.run_raw(dir, ["diff", "--cached", "--quiet"])?;
    if staged.success() {
        return Ok(false);
    }
    let mut args = COMMIT.to_vec();
    args.extend(["commit", "--quiet", "--no-verify", "-m", "Update settings"]);
    git.run(dir, args)?;
    Ok(true)
}

fn guard(dir: &Path) -> Result<()> {
    let path = dir.join(PORTABLE_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::Read { path, source }),
    };
    match find_secret(&text) {
        Some(reason) => Err(Error::Invalid(format!(
            "not syncing: {PORTABLE_FILE} {reason}. Secrets belong in the system keyring."
        ))),
        None => Ok(()),
    }
}

/// A merge waiting for the user to settle conflicting settings.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingMerge {
    /// The remote commit being merged.
    pub theirs: String,
    /// The merge so far, with this machine's side of every conflict.
    pub merged: Value,
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PullOutcome {
    UpToDate,
    /// `portable.toml` changed; reload it.
    Updated,
    Conflicts(PendingMerge),
}

/// Commits local changes, fetches, and merges the remote's settings.
pub fn pull(git: &GitCli, dir: &Path, branch: &str) -> Result<PullOutcome> {
    commit(git, dir)?;
    git.run(dir, ["fetch", "--quiet", "--no-tags", REMOTE])?;
    let Some(theirs) = rev(git, dir, &format!("refs/remotes/{REMOTE}/{branch}"))? else {
        return Ok(PullOutcome::UpToDate);
    };
    let Some(ours) = rev(git, dir, "HEAD")? else {
        // Nothing here yet: take the remote's settings as they are.
        git.run(dir, ["reset", "--quiet", "--hard", &theirs])?;
        return Ok(PullOutcome::Updated);
    };
    let base = git.run_raw(dir, ["merge-base", &ours, &theirs])?;
    let base = base.success().then(|| base.stdout.trim().to_owned());
    if base.as_deref() == Some(theirs.as_str()) {
        return Ok(PullOutcome::UpToDate);
    }
    if base.as_deref() == Some(ours.as_str()) {
        git.run(dir, ["merge", "--quiet", "--ff-only", &theirs])?;
        return Ok(PullOutcome::Updated);
    }

    // Settings nobody has written yet are the defaults, so two machines
    // that started syncing separately only conflict where both moved away
    // from a default differently.
    let defaults = Value::try_from(Portable::default())?;
    let at = |commit: Option<&str>| -> Result<Value> {
        Ok(match commit {
            Some(commit) => settings_at(git, dir, commit)?,
            None => None,
        }
        .unwrap_or_else(|| defaults.clone()))
    };
    let merged = merge::merge3(
        Some(&at(base.as_deref())?),
        &at(Some(&ours))?,
        &at(Some(&theirs))?,
    );
    if merged.conflicts.is_empty() {
        finish(git, dir, &theirs, &merged.value)?;
        return Ok(PullOutcome::Updated);
    }
    Ok(PullOutcome::Conflicts(PendingMerge {
        theirs,
        merged: merged.value,
        conflicts: merged.conflicts,
    }))
}

/// Records the merge of `theirs` with `settings` as its result.
pub fn finish(git: &GitCli, dir: &Path, theirs: &str, settings: &Value) -> Result<()> {
    let portable: Portable = settings
        .clone()
        .try_into()
        .map_err(|e: toml::de::Error| Error::Invalid(e.to_string()))?;
    let text = toml::to_string_pretty(&portable.normalized())?;
    if let Some(reason) = find_secret(&text) {
        return Err(Error::Invalid(format!("not syncing: {reason}")));
    }
    git.run(
        dir,
        COMMIT.iter().copied().chain([
            "merge",
            "--quiet",
            "--no-ff",
            "--no-commit",
            "--strategy=ours",
            "--allow-unrelated-histories",
            theirs,
        ]),
    )?;
    let result = write_text_atomic(&dir.join(PORTABLE_FILE), &text)
        .and_then(|()| Ok(git.run(dir, ["add", PORTABLE_FILE])?))
        .and_then(|_| {
            let mut args = COMMIT.to_vec();
            args.extend([
                "commit",
                "--quiet",
                "--no-verify",
                "-m",
                "Merge settings from another machine",
            ]);
            Ok(git.run(dir, args)?)
        });
    if let Err(e) = result {
        let _ = git.run(dir, ["merge", "--abort"]);
        return Err(e);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    UpToDate,
    Pushed,
    /// The remote has commits this machine hasn't merged; pull first.
    Rejected,
}

/// Pushes committed settings to the remote branch.
pub fn push(git: &GitCli, dir: &Path, branch: &str) -> Result<PushOutcome> {
    let Some(ours) = rev(git, dir, "HEAD")? else {
        return Ok(PushOutcome::UpToDate);
    };
    if rev(git, dir, &format!("refs/remotes/{REMOTE}/{branch}"))?.as_ref() == Some(&ours) {
        return Ok(PushOutcome::UpToDate);
    }
    let target = format!("HEAD:refs/heads/{branch}");
    let finished = git.run_raw(dir, ["push", "--porcelain", REMOTE, &target])?;
    if finished.success() {
        return Ok(PushOutcome::Pushed);
    }
    if finished.stdout.lines().any(|l| l.starts_with('!')) {
        return Ok(PushOutcome::Rejected);
    }
    Err(finished.into_error().into())
}

/// Pull, then push; a push that lost a race with another machine pulls
/// and pushes once more.
pub fn sync(git: &GitCli, dir: &Path, branch: &str) -> Result<PullOutcome> {
    let mut pulled = pull(git, dir, branch)?;
    for _ in 0..2 {
        if matches!(pulled, PullOutcome::Conflicts(_)) {
            return Ok(pulled);
        }
        if push(git, dir, branch)? != PushOutcome::Rejected {
            return Ok(pulled);
        }
        match pull(git, dir, branch)? {
            PullOutcome::UpToDate => {}
            other => pulled = other,
        }
    }
    Ok(pulled)
}

fn rev(git: &GitCli, dir: &Path, name: &str) -> Result<Option<String>> {
    let spec = format!("{name}^{{commit}}");
    let out = git.run_raw(dir, ["rev-parse", "--verify", "--quiet", &spec])?;
    Ok(out.success().then(|| out.stdout.trim().to_owned()))
}

/// The settings committed in `commit`, if it has any.
fn settings_at(git: &GitCli, dir: &Path, commit: &str) -> Result<Option<Value>> {
    let spec = format!("{commit}:{PORTABLE_FILE}");
    let out = git.run_raw(dir, ["show", &spec])?;
    if !out.success() {
        return Ok(None);
    }
    // Read as settings, so a file written before a setting existed (or
    // edited by hand) has its defaults filled in: a missing key and its
    // default must not look like a change.
    let portable = crate::parse_portable(&out.stdout)
        .map_err(|e| Error::Invalid(format!("the synced settings in {commit} are invalid: {e}")))?;
    Ok(Some(Value::try_from(portable)?))
}

fn check_branch(git: &GitCli, dir: &Path, branch: &str) -> Result<()> {
    let cwd = if dir.is_dir() { dir } else { Path::new(".") };
    let ok = !branch.starts_with('-')
        && git
            .run_raw(cwd, ["check-ref-format", "--branch", branch])?
            .success();
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(format!("invalid branch name: “{branch}”")))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;
    use crate::{ConfigStore, Theme};

    fn git() -> GitCli {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        GitCli::discover()
            .unwrap()
            .env("GIT_CONFIG_GLOBAL", null)
            .env("GIT_CONFIG_NOSYSTEM", "1")
    }

    /// A bare remote and two machines' config dirs.
    struct Setup {
        _tmp: TempDir,
        remote: PathBuf,
        a: PathBuf,
        b: PathBuf,
        git: GitCli,
    }

    fn setup() -> Setup {
        let tmp = TempDir::new().unwrap();
        let git = git();
        let remote = tmp.path().join("remote.git");
        fs::create_dir(&remote).unwrap();
        git.run(&remote, ["init", "--quiet", "--bare"]).unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        for dir in [&a, &b] {
            enable(&git, dir, remote.to_str().unwrap(), "main").unwrap();
        }
        Setup {
            _tmp: tmp,
            remote,
            a,
            b,
            git,
        }
    }

    fn edit(dir: &Path, f: impl FnOnce(&mut Portable)) {
        let (mut store, _) = ConfigStore::load(dir);
        store.update_portable(f).unwrap();
        // Machine-local state is never synced.
        store
            .update_local(|l| l.touch_recent("/secret/path"))
            .unwrap();
    }

    fn load(dir: &Path) -> Portable {
        ConfigStore::load(dir).0.get().portable.clone()
    }

    #[test]
    fn first_machine_pushes_and_second_takes_its_settings() {
        let s = setup();
        edit(&s.a, |p| p.ui.theme = Theme::Light);
        assert_eq!(sync(&s.git, &s.a, "main").unwrap(), PullOutcome::UpToDate);

        let files = s
            .git
            .run(&s.remote, ["ls-tree", "--name-only", "main"])
            .unwrap();
        assert_eq!(files, ".gitignore\nportable.toml\n");

        assert_eq!(sync(&s.git, &s.b, "main").unwrap(), PullOutcome::Updated);
        assert_eq!(load(&s.b).ui.theme, Theme::Light);
    }

    #[test]
    fn different_settings_changed_on_two_machines_merge() {
        let s = setup();
        edit(&s.a, |p| p.ui.theme = Theme::Light);
        sync(&s.git, &s.a, "main").unwrap();
        sync(&s.git, &s.b, "main").unwrap();

        edit(&s.a, |p| p.git.auto_fetch_minutes = 3);
        edit(&s.b, |p| p.ui.show_avatars = false);
        sync(&s.git, &s.a, "main").unwrap();
        // b's push is behind: it merges a's change and pushes the merge.
        assert_eq!(sync(&s.git, &s.b, "main").unwrap(), PullOutcome::Updated);
        let b = load(&s.b);
        assert_eq!((b.git.auto_fetch_minutes, b.ui.show_avatars), (3, false));

        assert_eq!(sync(&s.git, &s.a, "main").unwrap(), PullOutcome::Updated);
        assert_eq!(load(&s.a), b);
    }

    #[test]
    fn unrelated_histories_merge_too() {
        let s = setup();
        edit(&s.a, |p| p.ui.theme = Theme::Light);
        edit(&s.b, |p| p.git.auto_fetch_minutes = 0);
        sync(&s.git, &s.a, "main").unwrap();
        assert_eq!(sync(&s.git, &s.b, "main").unwrap(), PullOutcome::Updated);
        let b = load(&s.b);
        assert_eq!((b.ui.theme, b.git.auto_fetch_minutes), (Theme::Light, 0));
    }

    #[test]
    fn missing_settings_count_as_defaults() {
        let s = setup();
        // A file with only some settings, as an older version or a hand
        // edit leaves it.
        fs::write(s.a.join(PORTABLE_FILE), "[git]\nautoFetchMinutes = 7\n").unwrap();
        sync(&s.git, &s.a, "main").unwrap();
        sync(&s.git, &s.b, "main").unwrap();

        let other = s.b.join("..").join("other");
        s.git
            .run(
                s.b.parent().unwrap(),
                [
                    "clone",
                    "--quiet",
                    "--branch",
                    "main",
                    s.remote.to_str().unwrap(),
                    "other",
                ],
            )
            .unwrap();
        fs::write(
            other.join(PORTABLE_FILE),
            "[git]\nautoFetchMinutes = 7\n[ui]\ntheme = \"dark\"\n",
        )
        .unwrap();
        let mut commit = COMMIT.to_vec();
        commit.extend(["commit", "--quiet", "-am", "Theme"]);
        s.git.run(&other, commit).unwrap();
        s.git.run(&other, ["push", "--quiet"]).unwrap();

        edit(&s.a, |p| p.ui.show_avatars = false);
        assert_eq!(sync(&s.git, &s.a, "main").unwrap(), PullOutcome::Updated);
        let a = load(&s.a);
        assert_eq!((a.ui.theme, a.ui.show_avatars), (Theme::Dark, false));
        assert_eq!(a.git.auto_fetch_minutes, 7);
    }

    #[test]
    fn the_same_setting_changed_differently_waits_for_a_choice() {
        let s = setup();
        edit(&s.a, |p| p.ui.theme = Theme::Dark);
        sync(&s.git, &s.a, "main").unwrap();
        sync(&s.git, &s.b, "main").unwrap();

        edit(&s.a, |p| p.ui.theme = Theme::Light);
        edit(&s.b, |p| p.ui.theme = Theme::System);
        sync(&s.git, &s.a, "main").unwrap();
        let PullOutcome::Conflicts(mut pending) = sync(&s.git, &s.b, "main").unwrap() else {
            panic!("expected a conflict");
        };
        assert_eq!(pending.conflicts.len(), 1);
        assert_eq!(merge::display(&pending.conflicts[0].path), "ui.theme");
        // Nothing was changed while waiting.
        assert_eq!(load(&s.b).ui.theme, Theme::System);

        let c = pending.conflicts.remove(0);
        merge::set(&mut pending.merged, &c.path, c.theirs);
        finish(&s.git, &s.b, &pending.theirs, &pending.merged).unwrap();
        assert_eq!(load(&s.b).ui.theme, Theme::Light);
        assert_eq!(push(&s.git, &s.b, "main").unwrap(), PushOutcome::Pushed);
    }

    #[test]
    fn refuses_to_commit_secrets() {
        let s = setup();
        edit(&s.a, |p| {
            p.keybindings
                .insert("x".into(), format!("ghp_{}", "x".repeat(36)));
        });
        let err = sync(&s.git, &s.a, "main").unwrap_err().to_string();
        assert!(err.contains("GitHub token"), "{err}");
    }

    #[test]
    fn disable_removes_the_repository_only() {
        let s = setup();
        edit(&s.a, |p| p.ui.theme = Theme::Light);
        sync(&s.git, &s.a, "main").unwrap();
        disable(&s.a).unwrap();
        assert!(!is_enabled(&s.a));
        assert!(!s.a.join(".gitignore").exists());
        assert_eq!(load(&s.a).ui.theme, Theme::Light);
    }

    #[test]
    fn rejects_bad_branch_and_remote_names() {
        let tmp = TempDir::new().unwrap();
        let git = git();
        assert!(enable(&git, tmp.path(), "x", "a..b").is_err());
        assert!(enable(&git, tmp.path(), "--upload-pack=x", "main").is_err());
        assert!(!is_enabled(tmp.path()));
    }
}
