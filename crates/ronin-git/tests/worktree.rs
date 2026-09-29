mod common;

use common::TestRepo;
use ronin_git::{
    BlobSource, CommitOptions, DiffOptions, FileStatus, IgnoreScope, LineKind, LineSelection,
    PatchTarget, StashOptions, StatusEntry, WorkingStatus, add_to_gitignore, apply_lines, commit,
    discard_files, head_message, list_refs, stage_files, stash_apply, stash_drop, stash_push,
    status, unstage_files, working_blob, working_diff,
};

fn paths(entries: &[StatusEntry]) -> Vec<(&str, FileStatus)> {
    entries
        .iter()
        .map(|e| (e.path.as_str(), e.status))
        .collect()
}

fn st(repo: &TestRepo) -> WorkingStatus {
    status(&repo.git, repo.path()).unwrap()
}

fn read(repo: &TestRepo, name: &str) -> String {
    std::fs::read_to_string(repo.path().join(name)).unwrap()
}

fn staged_blob(repo: &TestRepo, name: &str) -> String {
    repo.git(&["show", &format!(":{name}")])
}

fn entry(repo: &TestRepo, path: &str, staged: bool) -> StatusEntry {
    let s = st(repo);
    let list = if staged { s.staged } else { s.unstaged };
    list.into_iter().find(|e| e.path == path).unwrap()
}

const NUMBERS: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n";

#[test]
fn reports_staged_unstaged_and_untracked_changes() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "init");
    repo.commit_file("old.txt", "keep me\n", "second");

    repo.write("a.txt", "a2\n");
    repo.write("new dir/b.txt", "b\n");
    repo.git(&["mv", "old.txt", "renamed.txt"]);
    repo.write("staged.txt", "s\n");
    repo.git(&["add", "staged.txt"]);

    let s = st(&repo);
    assert_eq!(
        paths(&s.staged),
        [
            ("renamed.txt", FileStatus::Renamed),
            ("staged.txt", FileStatus::Added)
        ]
    );
    assert_eq!(s.staged[0].old_path.as_deref(), Some("old.txt"));
    assert_eq!(
        paths(&s.unstaged),
        [
            ("a.txt", FileStatus::Modified),
            ("new dir/b.txt", FileStatus::Untracked)
        ]
    );
    assert!(s.conflicted.is_empty());
}

#[test]
fn clean_repo_and_unborn_branch() {
    let repo = TestRepo::new();
    assert!(st(&repo).is_clean());
    repo.write("first.txt", "x\n");
    repo.git(&["add", "first.txt"]);
    assert_eq!(paths(&st(&repo).staged), [("first.txt", FileStatus::Added)]);

    // Unstaging works before the first commit too.
    unstage_files(&repo.git, repo.path(), &["first.txt".into()]).unwrap();
    assert_eq!(
        paths(&st(&repo).unstaged),
        [("first.txt", FileStatus::Untracked)]
    );
}

#[test]
fn diffs_working_tree_index_and_untracked_files() {
    let repo = TestRepo::new();
    repo.commit_file("f.txt", "one\ntwo\n", "init");
    repo.write("f.txt", "one\n2\n");
    repo.git(&["add", "f.txt"]);
    repo.write("f.txt", "one\n2\nthree\n");
    repo.write("u.txt", "hello\n");
    let options = DiffOptions::default();

    let staged = working_diff(
        &repo.git,
        repo.path(),
        &entry(&repo, "f.txt", true),
        true,
        options,
    )
    .unwrap();
    let texts = |d: &ronin_git::FileDiff| -> Vec<(LineKind, String)> {
        d.hunks[0]
            .lines
            .iter()
            .map(|l| (l.kind, l.text.clone()))
            .collect()
    };
    assert_eq!(
        texts(&staged),
        [
            (LineKind::Context, "one".into()),
            (LineKind::Removed, "two".into()),
            (LineKind::Added, "2".into()),
        ]
    );

    let unstaged = working_diff(
        &repo.git,
        repo.path(),
        &entry(&repo, "f.txt", false),
        false,
        options,
    )
    .unwrap();
    assert_eq!(
        texts(&unstaged).last().unwrap(),
        &(LineKind::Added, "three".into())
    );

    let untracked = working_diff(
        &repo.git,
        repo.path(),
        &entry(&repo, "u.txt", false),
        false,
        options,
    )
    .unwrap();
    assert_eq!(texts(&untracked), [(LineKind::Added, "hello".into())]);
}

#[test]
fn stages_unstages_and_discards_whole_files() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "init");
    repo.commit_file("gone.txt", "g\n", "second");
    repo.write("a.txt", "changed\n");
    std::fs::remove_file(repo.path().join("gone.txt")).unwrap();
    repo.write("*.txt", "literal name\n");

    let all: Vec<String> = ["a.txt", "gone.txt", "*.txt"].map(Into::into).into();
    stage_files(&repo.git, repo.path(), &all).unwrap();
    let s = st(&repo);
    assert!(s.unstaged.is_empty());
    assert_eq!(
        paths(&s.staged),
        [
            ("*.txt", FileStatus::Added),
            ("a.txt", FileStatus::Modified),
            ("gone.txt", FileStatus::Deleted)
        ]
    );

    // A literal pathspec: `*.txt` must not unstage every text file.
    unstage_files(&repo.git, repo.path(), &["*.txt".into()]).unwrap();
    assert_eq!(st(&repo).staged.len(), 2);

    unstage_files(&repo.git, repo.path(), &all).unwrap();
    let s = st(&repo);
    assert!(s.staged.is_empty());
    discard_files(&repo.git, repo.path(), &s.unstaged).unwrap();
    assert!(st(&repo).is_clean());
    assert_eq!(read(&repo, "a.txt"), "a\n");
    assert_eq!(read(&repo, "gone.txt"), "g\n");
    assert!(!repo.path().join("*.txt").exists());
}

#[test]
fn stages_and_unstages_single_lines() {
    let repo = TestRepo::new();
    repo.commit_file("n.txt", NUMBERS, "init");
    // Two hunks: 2 → two (line 2) and a new line after 17.
    repo.write(
        "n.txt",
        &NUMBERS
            .replace("2\n3\n", "two\n3\n")
            .replace("17\n", "17\n17.5\n"),
    );

    let file = entry(&repo, "n.txt", false);
    let diff = working_diff(&repo.git, repo.path(), &file, false, DiffOptions::default()).unwrap();
    assert_eq!(diff.hunks.len(), 2);
    let added = |h: usize| {
        diff.hunks[h]
            .lines
            .iter()
            .position(|l| l.kind == LineKind::Added)
            .unwrap() as u32
    };

    // Only the addition from the second hunk.
    let pick = [LineSelection {
        hunk: 1,
        lines: vec![added(1)],
    }];
    apply_lines(
        &repo.git,
        repo.path(),
        "n.txt",
        &diff.hunks,
        &pick,
        PatchTarget::Stage,
    )
    .unwrap();
    assert_eq!(
        staged_blob(&repo, "n.txt"),
        NUMBERS.replace("17\n", "17\n17.5\n")
    );

    // Now just the "+two" of the first hunk; its "-2" stays unstaged.
    let diff = working_diff(&repo.git, repo.path(), &file, false, DiffOptions::default()).unwrap();
    let pick = [LineSelection {
        hunk: 0,
        lines: vec![added(0)],
    }];
    apply_lines(
        &repo.git,
        repo.path(),
        "n.txt",
        &diff.hunks,
        &pick,
        PatchTarget::Stage,
    )
    .unwrap();
    assert_eq!(
        staged_blob(&repo, "n.txt"),
        NUMBERS
            .replace("2\n3\n", "2\ntwo\n3\n")
            .replace("17\n", "17\n17.5\n")
    );

    // Unstage the 17.5 line again, from the staged diff.
    let staged = entry(&repo, "n.txt", true);
    let diff = working_diff(
        &repo.git,
        repo.path(),
        &staged,
        true,
        DiffOptions::default(),
    )
    .unwrap();
    let hunk = diff
        .hunks
        .iter()
        .position(|h| h.lines.iter().any(|l| l.text == "17.5"))
        .unwrap();
    let line = diff.hunks[hunk]
        .lines
        .iter()
        .position(|l| l.text == "17.5")
        .unwrap();
    let pick = [LineSelection {
        hunk: hunk as u32,
        lines: vec![line as u32],
    }];
    apply_lines(
        &repo.git,
        repo.path(),
        "n.txt",
        &diff.hunks,
        &pick,
        PatchTarget::Unstage,
    )
    .unwrap();
    assert_eq!(
        staged_blob(&repo, "n.txt"),
        NUMBERS.replace("2\n3\n", "2\ntwo\n3\n")
    );
    // The working tree is untouched throughout.
    assert!(read(&repo, "n.txt").contains("17.5"));
}

#[test]
fn discards_a_hunk_from_the_working_tree() {
    let repo = TestRepo::new();
    repo.commit_file("n.txt", NUMBERS, "init");
    let edited = NUMBERS
        .replace("\n2\n", "\ntwo\n")
        .replace("18\n", "eighteen\n");
    repo.write("n.txt", &edited);

    let file = entry(&repo, "n.txt", false);
    let diff = working_diff(&repo.git, repo.path(), &file, false, DiffOptions::default()).unwrap();
    let all = |h: usize| LineSelection {
        hunk: h as u32,
        lines: (0..diff.hunks[h].lines.len() as u32).collect(),
    };
    apply_lines(
        &repo.git,
        repo.path(),
        "n.txt",
        &diff.hunks,
        &[all(0)],
        PatchTarget::Discard,
    )
    .unwrap();
    assert_eq!(read(&repo, "n.txt"), NUMBERS.replace("18\n", "eighteen\n"));
    assert!(st(&repo).staged.is_empty());
}

#[test]
fn stages_lines_of_crlf_files() {
    let repo = TestRepo::new();
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.commit_file("w.txt", "a\r\nb\r\n", "init");
    repo.write("w.txt", "a\r\nB\r\nc\r\n");

    let file = entry(&repo, "w.txt", false);
    let diff = working_diff(&repo.git, repo.path(), &file, false, DiffOptions::default()).unwrap();
    let lines = &diff.hunks[0].lines;
    let pick: Vec<u32> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.text == "B\r" || l.text == "b\r")
        .map(|(i, _)| i as u32)
        .collect();
    let pick = [LineSelection {
        hunk: 0,
        lines: pick,
    }];
    apply_lines(
        &repo.git,
        repo.path(),
        "w.txt",
        &diff.hunks,
        &pick,
        PatchTarget::Stage,
    )
    .unwrap();
    assert_eq!(staged_blob(&repo, "w.txt"), "a\r\nB\r\n");
}

#[test]
fn stale_patches_are_refused() {
    let repo = TestRepo::new();
    repo.commit_file("f.txt", "one\n", "init");
    repo.write("f.txt", "uno\n");
    let file = entry(&repo, "f.txt", false);
    let diff = working_diff(&repo.git, repo.path(), &file, false, DiffOptions::default()).unwrap();
    // Someone else stages a different version meanwhile.
    repo.write("f.txt", "eins\n");
    repo.git(&["add", "f.txt"]);
    let pick = [LineSelection {
        hunk: 0,
        lines: vec![0, 1],
    }];
    assert!(
        apply_lines(
            &repo.git,
            repo.path(),
            "f.txt",
            &diff.hunks,
            &pick,
            PatchTarget::Stage
        )
        .is_err()
    );
    assert_eq!(staged_blob(&repo, "f.txt"), "eins\n");
}

#[test]
fn commits_amends_and_signs_off() {
    let repo = TestRepo::new();
    assert_eq!(head_message(repo.path()).unwrap(), None);
    repo.write("a.txt", "a\n");
    repo.git(&["add", "a.txt"]);

    let oid = commit(
        &repo.git,
        repo.path(),
        "Add a\n\nWith a body.\n# not a comment\n",
        CommitOptions::default(),
    )
    .unwrap()
    .oid;
    assert_eq!(oid, repo.git(&["rev-parse", "HEAD"]).trim());
    assert_eq!(
        head_message(repo.path()).unwrap().unwrap(),
        "Add a\n\nWith a body.\n# not a comment\n"
    );

    repo.write("b.txt", "b\n");
    repo.git(&["add", "b.txt"]);
    let amended = commit(
        &repo.git,
        repo.path(),
        "Add a and b",
        CommitOptions {
            amend: true,
            signoff: true,
            no_verify: false,
        },
    )
    .unwrap()
    .oid;
    assert_ne!(amended, oid);
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]).trim(), "1");
    let message = head_message(repo.path()).unwrap().unwrap();
    assert!(message.starts_with("Add a and b\n"));
    assert!(message.contains("Signed-off-by: Ronin Test <test@ronin.invalid>"));
    assert!(st(&repo).is_clean());

    assert!(commit(&repo.git, repo.path(), "  \n", CommitOptions::default()).is_err());
    // Nothing staged: git's own error comes through.
    assert!(commit(&repo.git, repo.path(), "Empty", CommitOptions::default()).is_err());
}

#[test]
fn stash_round_trip() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "init");
    let nothing = stash_push(&repo.git, repo.path(), &StashOptions::default()).unwrap();
    assert!(!nothing);

    repo.write("a.txt", "a2\n");
    repo.write("u.txt", "untracked\n");
    let options = StashOptions {
        message: Some("wip".into()),
        include_untracked: true,
        keep_index: false,
    };
    assert!(stash_push(&repo.git, repo.path(), &options).unwrap());
    assert!(st(&repo).is_clean());

    repo.write("a.txt", "a3\n");
    assert!(
        stash_push(&repo.git, repo.path(), &StashOptions::default()).unwrap(),
        "second stash"
    );
    let stashes = list_refs(&repo.git, repo.path()).unwrap().stashes;
    assert_eq!(stashes.len(), 2);
    assert!(stashes[1].message.ends_with("wip"));

    // A stale index/oid pair is refused.
    assert!(stash_drop(&repo.git, repo.path(), 0, &stashes[1].oid).is_err());

    stash_drop(&repo.git, repo.path(), 0, &stashes[0].oid).unwrap();
    let wip = &list_refs(&repo.git, repo.path()).unwrap().stashes[0];
    stash_apply(&repo.git, repo.path(), 0, &wip.oid, false).unwrap();
    assert_eq!(read(&repo, "a.txt"), "a2\n");
    assert_eq!(read(&repo, "u.txt"), "untracked\n");
    assert_eq!(list_refs(&repo.git, repo.path()).unwrap().stashes.len(), 1);

    repo.git(&["checkout", "--", "a.txt"]);
    std::fs::remove_file(repo.path().join("u.txt")).unwrap();
    stash_apply(&repo.git, repo.path(), 0, &wip.oid, true).unwrap();
    assert_eq!(read(&repo, "a.txt"), "a2\n");
    assert!(
        list_refs(&repo.git, repo.path())
            .unwrap()
            .stashes
            .is_empty()
    );
}

#[test]
fn keep_index_leaves_staged_changes() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "init");
    repo.commit_file("b.txt", "b\n", "second");
    repo.write("a.txt", "staged\n");
    repo.git(&["add", "a.txt"]);
    repo.write("b.txt", "unstaged\n");
    let options = StashOptions {
        keep_index: true,
        ..Default::default()
    };
    assert!(stash_push(&repo.git, repo.path(), &options).unwrap());
    let s = st(&repo);
    assert_eq!(paths(&s.staged), [("a.txt", FileStatus::Modified)]);
    assert!(s.unstaged.is_empty());
}

#[test]
fn adds_patterns_to_gitignore() {
    let repo = TestRepo::new();
    repo.write("logs/today.log", "x\n");
    repo.write("keep.txt", "k\n");
    repo.write(".gitignore", "target");

    let pattern = add_to_gitignore(repo.path(), "logs/today.log", IgnoreScope::Extension).unwrap();
    assert_eq!(pattern, "*.log");
    // Adding it twice doesn't duplicate it.
    add_to_gitignore(repo.path(), "logs/today.log", IgnoreScope::Extension).unwrap();
    assert_eq!(read(&repo, ".gitignore"), "target\n*.log\n");
    assert_eq!(
        paths(&st(&repo).unstaged),
        [
            (".gitignore", FileStatus::Untracked),
            ("keep.txt", FileStatus::Untracked)
        ]
    );
    assert!(add_to_gitignore(repo.path(), "keep.txt", IgnoreScope::Folder).is_err());
}

#[test]
fn reads_blobs_from_head_index_and_working_tree() {
    let repo = TestRepo::new();
    let blob = |source| working_blob(repo.path(), "f.bin", source).unwrap();
    assert_eq!(blob(BlobSource::Head), None);
    repo.commit_file("f.bin", "head", "init");
    repo.write("f.bin", "index");
    repo.git(&["add", "f.bin"]);
    repo.write("f.bin", "worktree");
    assert_eq!(blob(BlobSource::Head).unwrap(), b"head");
    assert_eq!(blob(BlobSource::Index).unwrap(), b"index");
    assert_eq!(blob(BlobSource::Worktree).unwrap(), b"worktree");
    assert_eq!(
        working_blob(repo.path(), "nope", BlobSource::Index).unwrap(),
        None
    );
}
