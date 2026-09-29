mod common;

use common::TestRepo;
use ronin_git::{
    BisectMark, CommitOptions, FlowConfig, FlowKind, HeadState, Journal, MergeChunk, Operation,
    OperationAction, Outcome, RebaseAction, RebaseStep, Resolution, SubmoduleState, UndoStyle,
    add_submodule, add_worktree, bisect_mark, bisect_state, blame, commit, conflict, file_log,
    flow_config, flow_finish, flow_init, flow_start, interactive_rebase, lfs_status, lfs_track,
    lfs_untrack, list_refs, list_worktrees, merge, open_repo, rebase_plan, remove_worktree,
    resolve_conflict, resolve_operation, snapshot, status, update_submodules,
};

fn on(name: &str) -> HeadState {
    HeadState::Branch {
        name: name.into(),
        unborn: false,
    }
}

fn log(repo: &TestRepo) -> Vec<String> {
    repo.git(&["log", "--format=%s"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// main and topic both changed line 2 of a.txt; main is checked out.
fn diverged() -> TestRepo {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "one\ntwo\nthree\n", "base");
    repo.git(&["switch", "-q", "-c", "topic"]);
    repo.commit_file("a.txt", "one\nTWO (topic)\nthree\n", "topic change");
    repo.git(&["switch", "-q", "main"]);
    repo.commit_file("a.txt", "one\nTWO (main)\nthree\n", "main change");
    repo
}

#[test]
fn loads_and_resolves_a_text_conflict() {
    let repo = diverged();
    assert_eq!(
        merge(&repo.git, repo.path(), "topic", false).unwrap(),
        Outcome::Conflicts
    );

    let c = conflict(&repo.git, repo.path(), "a.txt").unwrap();
    assert!(c.text && c.has_base && c.ours.present && c.theirs.present);
    assert_eq!(c.ours.label, "main");
    assert_eq!(c.theirs.label, "topic");
    assert_eq!(
        c.chunks,
        [
            MergeChunk::Resolved {
                text: "one\n".into()
            },
            MergeChunk::Conflict {
                ours: "TWO (main)\n".into(),
                base: "two\n".into(),
                theirs: "TWO (topic)\n".into(),
            },
            MergeChunk::Resolved {
                text: "three\n".into()
            },
        ]
    );

    let text = "one\nTWO (main)\nTWO (topic)\nthree\n".to_owned();
    resolve_conflict(
        &repo.git,
        repo.path(),
        "a.txt",
        &Resolution::Content { text },
    )
    .unwrap();
    let s = status(&repo.git, repo.path()).unwrap();
    assert!(s.conflicted.is_empty());
    assert_eq!(s.staged.len(), 1);
    assert_eq!(repo.read("a.txt"), "one\nTWO (main)\nTWO (topic)\nthree\n");
}

#[test]
fn resolves_whole_files_and_deletions() {
    let repo = TestRepo::new();
    repo.commit_file("keep.txt", "base\n", "base");
    repo.write("gone.txt", "base\n");
    repo.git(&["add", "gone.txt"]);
    repo.commit(&["-m", "add gone"]);
    repo.git(&["switch", "-q", "-c", "topic"]);
    repo.commit_file("keep.txt", "topic\n", "topic keep");
    repo.git(&["rm", "-q", "gone.txt"]);
    repo.commit(&["-m", "topic deletes"]);
    repo.git(&["switch", "-q", "main"]);
    repo.commit_file("keep.txt", "main\n", "main keep");
    repo.commit_file("gone.txt", "main edit\n", "main edits");
    assert_eq!(
        merge(&repo.git, repo.path(), "topic", false).unwrap(),
        Outcome::Conflicts
    );

    let gone = conflict(&repo.git, repo.path(), "gone.txt").unwrap();
    assert!(gone.ours.present && !gone.theirs.present);
    assert!(!gone.text && gone.chunks.is_empty());

    resolve_conflict(&repo.git, repo.path(), "keep.txt", &Resolution::Theirs).unwrap();
    assert_eq!(repo.read("keep.txt"), "topic\n");
    // Theirs deleted it.
    resolve_conflict(&repo.git, repo.path(), "gone.txt", &Resolution::Theirs).unwrap();
    assert!(!repo.path().join("gone.txt").exists());
    assert!(
        status(&repo.git, repo.path())
            .unwrap()
            .conflicted
            .is_empty()
    );
    assert!(conflict(&repo.git, repo.path(), "keep.txt").is_err());
}

/// A branch with four commits, each adding its own file, on top of "base".
fn four_commits() -> (TestRepo, String) {
    let repo = TestRepo::new();
    let base = repo.commit_file("base.txt", "base\n", "base");
    for n in 1..=4 {
        repo.commit_file(&format!("{n}.txt"), "x\n", &format!("c{n}"));
    }
    (repo, base)
}

fn steps(plan: &[(usize, RebaseAction, Option<&str>)], oids: &[String]) -> Vec<RebaseStep> {
    plan.iter()
        .map(|&(n, action, message)| RebaseStep {
            oid: oids[n - 1].clone(),
            action,
            message: message.map(str::to_owned),
        })
        .collect()
}

#[test]
fn plans_and_runs_an_interactive_rebase() {
    let (repo, base) = four_commits();
    let plan = rebase_plan(&repo.git, repo.path(), Some(&base)).unwrap();
    let messages: Vec<&str> = plan.commits.iter().map(|c| c.message.as_str()).collect();
    assert_eq!(messages, ["c1", "c2", "c3", "c4"]);
    assert_eq!(plan.merges, 0);
    let oids: Vec<String> = plan.commits.iter().map(|c| c.oid.clone()).collect();

    use RebaseAction::*;
    let outcome = interactive_rebase(
        &repo.git,
        repo.path(),
        Some(&base),
        &plan.head,
        &steps(
            &[
                (1, Reword, Some("C1 reworded\n\nWith a body")),
                (3, Pick, None),
                (2, Fixup, None),
                (4, Drop, None),
            ],
            &oids,
        ),
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(log(&repo), ["c3", "C1 reworded", "base"]);
    assert_eq!(
        repo.git(&["log", "-1", "--format=%B", "HEAD~1"]).trim(),
        "C1 reworded\n\nWith a body"
    );
    assert!(repo.path().join("2.txt").exists());
    assert!(!repo.path().join("4.txt").exists());
    assert_eq!(open_repo(repo.path()).unwrap().head, on("main"));
}

#[test]
fn squashes_with_and_without_a_new_message() {
    use RebaseAction::*;
    let (repo, base) = four_commits();
    let plan = rebase_plan(&repo.git, repo.path(), Some(&base)).unwrap();
    let oids: Vec<String> = plan.commits.iter().map(|c| c.oid.clone()).collect();
    let steps = steps(
        &[
            (1, Pick, None),
            (2, Squash, None),
            (3, Pick, None),
            (4, Squash, Some("Three and four")),
        ],
        &oids,
    );
    let outcome = interactive_rebase(&repo.git, repo.path(), Some(&base), &plan.head, &steps);
    assert_eq!(outcome.unwrap(), Outcome::Done);
    assert_eq!(log(&repo), ["Three and four", "c1", "base"]);
    // git's combined message, without its comment lines.
    assert_eq!(
        repo.git(&["log", "-1", "--format=%B", "HEAD~1"]).trim(),
        "c1\n\nc2"
    );
}

#[test]
fn stops_to_edit_and_refuses_bad_plans() {
    use RebaseAction::*;
    let (repo, base) = four_commits();
    let plan = rebase_plan(&repo.git, repo.path(), Some(&base)).unwrap();
    let oids: Vec<String> = plan.commits.iter().map(|c| c.oid.clone()).collect();

    let first_squash = steps(
        &[
            (1, Squash, None),
            (2, Pick, None),
            (3, Pick, None),
            (4, Pick, None),
        ],
        &oids,
    );
    assert!(
        interactive_rebase(
            &repo.git,
            repo.path(),
            Some(&base),
            &plan.head,
            &first_squash
        )
        .is_err()
    );
    let missing = steps(&[(1, Pick, None), (2, Pick, None)], &oids);
    assert!(interactive_rebase(&repo.git, repo.path(), Some(&base), &plan.head, &missing).is_err());
    let all = steps(
        &[
            (1, Pick, None),
            (2, Edit, None),
            (3, Pick, None),
            (4, Pick, None),
        ],
        &oids,
    );
    assert!(interactive_rebase(&repo.git, repo.path(), Some(&base), &oids[0], &all).is_err());

    let outcome =
        interactive_rebase(&repo.git, repo.path(), Some(&base), &plan.head, &all).unwrap();
    assert_eq!(outcome, Outcome::Stopped);
    let info = open_repo(repo.path()).unwrap();
    assert_eq!(info.operation, Some(Operation::Rebase));
    let progress = info.rebase.unwrap();
    assert!(progress.editing);
    assert_eq!((progress.step, progress.total), (2, 4));
    assert_eq!(progress.branch.as_deref(), Some("main"));

    repo.write("2.txt", "edited\n");
    repo.git(&["add", "2.txt"]);
    repo.git(&["commit", "-q", "--amend", "--no-edit"]);
    let done = resolve_operation(&repo.git, repo.path(), OperationAction::Continue).unwrap();
    assert_eq!(done, Outcome::Done);
    assert_eq!(log(&repo), ["c4", "c3", "c2", "c1", "base"]);
    assert_eq!(repo.git(&["show", "HEAD~2:2.txt"]), "edited\n");
}

#[test]
fn rewrites_from_the_root_and_stops_on_conflicts() {
    use RebaseAction::*;
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "1\n", "first");
    repo.commit_file("a.txt", "2\n", "second");
    let plan = rebase_plan(&repo.git, repo.path(), None).unwrap();
    assert_eq!(plan.base, None);
    let oids: Vec<String> = plan.commits.iter().map(|c| c.oid.clone()).collect();

    let reword = steps(&[(1, Reword, Some("Root")), (2, Pick, None)], &oids);
    let outcome = interactive_rebase(&repo.git, repo.path(), None, &plan.head, &reword);
    assert_eq!(outcome.unwrap(), Outcome::Done);
    assert_eq!(log(&repo), ["second", "Root"]);

    // Swapping two edits of the same line conflicts.
    let plan = rebase_plan(&repo.git, repo.path(), None).unwrap();
    let oids: Vec<String> = plan.commits.iter().map(|c| c.oid.clone()).collect();
    let swap = steps(&[(2, Pick, None), (1, Pick, None)], &oids);
    let outcome = interactive_rebase(&repo.git, repo.path(), None, &plan.head, &swap);
    assert_eq!(outcome.unwrap(), Outcome::Conflicts);
    resolve_operation(&repo.git, repo.path(), OperationAction::Abort).unwrap();
    assert_eq!(repo.head(), plan.head);
}

#[test]
fn blames_lines_and_follows_file_history() {
    let repo = TestRepo::new();
    let first = repo.commit_file("old.txt", "a\nb\n", "add");
    repo.git(&["mv", "old.txt", "new.txt"]);
    repo.commit(&["-m", "rename"]);
    let third = repo.commit_file("new.txt", "a\nB\n", "edit");
    repo.write("new.txt", "a\nB\nc\n");

    let b = blame(&repo.git, repo.path(), "new.txt", Some("HEAD"), false).unwrap();
    let who: Vec<(&str, &str)> = b
        .lines
        .iter()
        .map(|l| (b.commits[l.commit as usize].oid.as_str(), l.text.as_str()))
        .collect();
    assert_eq!(who, [(first.as_str(), "a"), (third.as_str(), "B")]);
    assert_eq!(b.commits[0].path, "old.txt");
    assert_eq!(b.commits[1].summary, "edit");

    let working = blame(&repo.git, repo.path(), "new.txt", None, false).unwrap();
    let last = &working.commits[working.lines[2].commit as usize];
    assert!(last.oid.bytes().all(|c| c == b'0'));

    let history = file_log(&repo.git, repo.path(), "new.txt", None, 0, 10).unwrap();
    let paths: Vec<(&str, &str)> = history
        .iter()
        .map(|c| (c.summary.as_str(), c.path.as_str()))
        .collect();
    assert_eq!(
        paths,
        [
            ("edit", "new.txt"),
            ("rename", "new.txt"),
            ("add", "old.txt")
        ]
    );
    assert_eq!(history[1].old_path.as_deref(), Some("old.txt"));
    let page = file_log(&repo.git, repo.path(), "new.txt", None, 1, 1).unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].summary, "rename");
}

#[test]
fn adds_and_updates_submodules() {
    let library = TestRepo::new();
    library.commit_file("lib.txt", "lib\n", "library");
    let repo = TestRepo::new();
    repo.commit_file("app.txt", "app\n", "app");
    // Local clones of submodules are refused unless allowed.
    let git = repo
        .git
        .clone()
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
        .env("GIT_CONFIG_VALUE_0", "always");
    let url = library.path().to_str().unwrap();
    add_submodule(&git, repo.path(), url, "vendor/lib", &mut |_| {}).unwrap();
    assert_eq!(repo.read("vendor/lib/lib.txt"), "lib\n");
    repo.commit(&["-m", "add submodule"]);

    let refs = list_refs(&git, repo.path()).unwrap();
    assert_eq!(refs.submodules.len(), 1);
    assert_eq!(refs.submodules[0].state, SubmoduleState::Current);

    let clone = TestRepo::clone_of(&repo);
    let git = clone
        .git
        .clone()
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
        .env("GIT_CONFIG_VALUE_0", "always");
    let refs = list_refs(&git, clone.path()).unwrap();
    assert_eq!(refs.submodules[0].state, SubmoduleState::Uninitialized);
    update_submodules(&git, clone.path(), &[], &mut |_| {}).unwrap();
    assert_eq!(clone.read("vendor/lib/lib.txt"), "lib\n");
    let refs = list_refs(&git, clone.path()).unwrap();
    assert_eq!(refs.submodules[0].state, SubmoduleState::Current);
}

#[test]
fn adds_lists_and_removes_worktrees() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    let parent = tempfile::TempDir::new().unwrap();
    let dest = parent.path().join("feature wt");
    add_worktree(&repo.git, repo.path(), &dest, "feature", true, None).unwrap();
    assert_eq!(std::fs::read_to_string(dest.join("a.txt")).unwrap(), "a\n");

    let wts = list_worktrees(&repo.git, repo.path()).unwrap();
    assert_eq!(wts.len(), 2);
    assert!(wts[0].is_main && wts[0].is_current);
    assert_eq!(wts[1].branch.as_deref(), Some("feature"));
    assert!(!wts[1].is_current);

    let refs = list_refs(&repo.git, repo.path()).unwrap();
    let feature = refs.local.iter().find(|b| b.name == "feature").unwrap();
    assert!(feature.worktree.is_some());
    assert!(
        refs.local
            .iter()
            .find(|b| b.name == "main")
            .unwrap()
            .worktree
            .is_none()
    );

    // From the linked worktree, it is the current one.
    let wts = list_worktrees(&repo.git, &dest).unwrap();
    assert!(wts[1].is_current && !wts[0].is_current);

    std::fs::write(dest.join("dirty.txt"), "x").unwrap();
    let path = wts[1].path.clone();
    assert!(remove_worktree(&repo.git, repo.path(), &path, false).is_err());
    remove_worktree(&repo.git, repo.path(), &path, true).unwrap();
    assert_eq!(list_worktrees(&repo.git, repo.path()).unwrap().len(), 1);
    assert!(remove_worktree(&repo.git, repo.path(), repo.path().to_str().unwrap(), true).is_err());
}

fn flow() -> FlowConfig {
    FlowConfig {
        master: "main".into(),
        develop: "develop".into(),
        feature_prefix: "feature/".into(),
        release_prefix: "release/".into(),
        hotfix_prefix: "hotfix/".into(),
        version_tag_prefix: "v".into(),
    }
}

#[test]
fn runs_git_flow_features_and_releases() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    assert_eq!(flow_config(&repo.git, repo.path()).unwrap(), None);
    flow_init(&repo.git, repo.path(), &flow()).unwrap();
    assert_eq!(flow_config(&repo.git, repo.path()).unwrap(), Some(flow()));

    flow_start(&repo.git, repo.path(), FlowKind::Feature, "login").unwrap();
    assert_eq!(open_repo(repo.path()).unwrap().head, on("feature/login"));
    repo.commit_file("login.txt", "x\n", "login");

    // Finishing is recorded for undo like any other action.
    let mut journal = Journal::default();
    let before = snapshot(&repo.git, repo.path()).unwrap();
    let outcome = flow_finish(
        &repo.git,
        repo.path(),
        FlowKind::Feature,
        "feature/login",
        None,
        false,
    );
    assert_eq!(outcome.unwrap(), Outcome::Done);
    let after = snapshot(&repo.git, repo.path()).unwrap();
    journal.record(
        &repo.git,
        repo.path(),
        "Finish",
        &before,
        &after,
        UndoStyle::Keep,
    );
    assert_eq!(open_repo(repo.path()).unwrap().head, on("develop"));
    assert!(repo.git(&["branch", "--list", "feature/login"]).is_empty());
    assert!(repo.path().join("login.txt").exists());

    journal.undo(&repo.git, repo.path()).unwrap();
    assert_eq!(open_repo(repo.path()).unwrap().head, on("feature/login"));
    assert!(!repo.git(&["branch", "--list", "develop"]).is_empty());
    assert!(
        repo.git(&["log", "--format=%s", "develop"])
            .lines()
            .all(|l| l != "login")
    );
    journal.redo(&repo.git, repo.path()).unwrap();
    assert_eq!(open_repo(repo.path()).unwrap().head, on("develop"));

    flow_start(&repo.git, repo.path(), FlowKind::Release, "1.0").unwrap();
    repo.commit_file("version.txt", "1.0\n", "bump");
    let outcome = flow_finish(
        &repo.git,
        repo.path(),
        FlowKind::Release,
        "release/1.0",
        None,
        false,
    );
    assert_eq!(outcome.unwrap(), Outcome::Done);
    assert_eq!(repo.git(&["show", "v1.0:version.txt"]), "1.0\n");
    assert_eq!(repo.git(&["show", "main:version.txt"]), "1.0\n");
    assert_eq!(repo.git(&["show", "develop:version.txt"]), "1.0\n");
    assert_eq!(
        repo.git(&["tag", "-l", "--format=%(contents:subject)", "v1.0"])
            .trim(),
        "Release 1.0"
    );
    assert!(
        flow_finish(
            &repo.git,
            repo.path(),
            FlowKind::Hotfix,
            "feature/x",
            None,
            false
        )
        .is_err()
    );
}

#[test]
fn bisects_to_the_first_bad_commit() {
    let repo = TestRepo::new();
    let mut oids = Vec::new();
    for n in 1..=8 {
        oids.push(repo.commit_file("n.txt", &format!("{n}\n"), &format!("c{n}")));
    }
    // Commits from c6 on are "bad".
    let is_bad = |repo: &TestRepo| repo.read("n.txt").trim().parse::<u32>().unwrap() >= 6;
    assert!(bisect_state(&repo.git, repo.path()).unwrap().is_none());

    bisect_mark(&repo.git, repo.path(), BisectMark::Bad, "HEAD").unwrap();
    let state = bisect_state(&repo.git, repo.path()).unwrap().unwrap();
    assert_eq!(state.bad.as_deref(), Some(oids[7].as_str()));
    assert_eq!(state.remaining, 0);
    assert_eq!(
        open_repo(repo.path()).unwrap().operation,
        Some(Operation::Bisect)
    );

    bisect_mark(&repo.git, repo.path(), BisectMark::Good, &oids[0]).unwrap();
    let mut steps = 0;
    let found = loop {
        let state = bisect_state(&repo.git, repo.path()).unwrap().unwrap();
        if let Some(found) = state.first_bad {
            break found;
        }
        assert!(state.remaining > 1 && !state.stuck);
        let mark = if is_bad(&repo) {
            BisectMark::Bad
        } else {
            BisectMark::Good
        };
        bisect_mark(&repo.git, repo.path(), mark, "HEAD").unwrap();
        steps += 1;
        assert!(steps < 8);
    };
    assert_eq!(found, oids[5]);

    resolve_operation(&repo.git, repo.path(), OperationAction::Abort).unwrap();
    assert!(bisect_state(&repo.git, repo.path()).unwrap().is_none());
    assert_eq!(open_repo(repo.path()).unwrap().head, on("main"));
}

#[test]
fn reports_hook_output_and_skips_hooks() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    let hook = repo.path().join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho 'lint: all good'\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    repo.write("a.txt", "b\n");
    repo.git(&["add", "a.txt"]);
    let result = commit(&repo.git, repo.path(), "second", CommitOptions::default()).unwrap();
    if cfg!(unix) {
        assert_eq!(result.output, "lint: all good");
    }

    std::fs::write(&hook, "#!/bin/sh\necho 'lint failed' >&2\nexit 1\n").unwrap();
    repo.write("a.txt", "c\n");
    repo.git(&["add", "a.txt"]);
    let err = commit(&repo.git, repo.path(), "third", CommitOptions::default()).unwrap_err();
    if cfg!(unix) {
        assert!(err.to_string().contains("lint failed"));
    }
    let skipped = CommitOptions {
        no_verify: true,
        ..Default::default()
    };
    let result = commit(&repo.git, repo.path(), "third", skipped).unwrap();
    assert_eq!(result.output, "");
}

#[test]
fn tracks_patterns_with_lfs_when_installed() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    let status = lfs_status(&repo.git, repo.path()).unwrap();
    if !status.installed {
        eprintln!("git lfs is not installed; skipping");
        return;
    }
    assert!(status.patterns.is_empty());
    lfs_track(&repo.git, repo.path(), "*.psd").unwrap();
    std::fs::create_dir(repo.path().join("sub")).unwrap();
    repo.git
        .run(&repo.path().join("sub"), ["lfs", "track", "*.zip"])
        .unwrap();
    let patterns = lfs_status(&repo.git, repo.path()).unwrap().patterns;
    assert_eq!(patterns.len(), 2);
    let zip = patterns.iter().find(|p| p.pattern == "sub/*.zip").unwrap();
    lfs_untrack(&repo.git, repo.path(), &zip.pattern, &zip.source).unwrap();
    let psd = patterns.iter().find(|p| p.pattern == "*.psd").unwrap();
    lfs_untrack(&repo.git, repo.path(), &psd.pattern, &psd.source).unwrap();
    assert!(
        lfs_status(&repo.git, repo.path())
            .unwrap()
            .patterns
            .is_empty()
    );
}
