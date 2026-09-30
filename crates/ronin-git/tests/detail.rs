mod common;

use common::TestRepo;
use ronin_git::{DiffOptions, FileStatus, LineKind, blob_at, commit_detail, file_diff};

#[test]
fn root_commit_lists_added_files() {
    let repo = TestRepo::new();
    let oid = repo.commit_file("a.txt", "one\ntwo\n", "Initial\n\nWith a body.");

    let detail = commit_detail(&repo.git, repo.path(), &oid).unwrap();
    assert!(detail.parents.is_empty());
    assert_eq!(detail.message, "Initial\n\nWith a body.\n");
    assert_eq!(detail.author.name, "Ronin Test");
    assert_eq!(detail.author.email, "test@ronin.invalid");
    assert_eq!(detail.files.len(), 1);
    let f = &detail.files[0];
    assert_eq!(
        (f.path.as_str(), f.status, f.additions, f.deletions),
        ("a.txt", FileStatus::Added, Some(2), Some(0))
    );
}

#[test]
fn reports_modifications_renames_deletions_and_binaries() {
    let repo = TestRepo::new();
    repo.write("keep.txt", "a\nb\nc\n");
    repo.write("gone.txt", "bye\n");
    repo.write("old.txt", "lots\nof\nstable\ncontent\nhere\n");
    repo.git(&["add", "."]);
    repo.commit(&["-m", "base"]);

    repo.write("keep.txt", "a\nB\nc\n");
    std::fs::remove_file(repo.path().join("gone.txt")).unwrap();
    repo.git(&["mv", "old.txt", "new.txt"]);
    std::fs::write(repo.path().join("pic.bin"), [0u8, 159, 146, 150, 0, 1]).unwrap();
    repo.git(&["add", "-A"]);
    repo.commit(&["-m", "changes"]);
    let oid = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

    let detail = commit_detail(&repo.git, repo.path(), &oid).unwrap();
    let mut files: Vec<_> = detail
        .files
        .iter()
        .map(|f| {
            (
                f.path.as_str(),
                f.old_path.as_deref(),
                f.status,
                f.additions,
                f.deletions,
            )
        })
        .collect();
    files.sort_by_key(|f| f.0);
    assert_eq!(
        files,
        [
            ("gone.txt", None, FileStatus::Deleted, Some(0), Some(1)),
            ("keep.txt", None, FileStatus::Modified, Some(1), Some(1)),
            (
                "new.txt",
                Some("old.txt"),
                FileStatus::Renamed,
                Some(0),
                Some(0)
            ),
            ("pic.bin", None, FileStatus::Added, None, None),
        ]
    );
}

#[test]
fn diffs_a_file_between_commits() {
    let repo = TestRepo::new();
    let base = repo.commit_file("f.txt", "one\ntwo\nthree\n", "base");
    let target = repo.commit_file("f.txt", "one\n  two\nthree\nfour\n", "edit");

    let diff = file_diff(
        &repo.git,
        repo.path(),
        Some(&base),
        &target,
        "f.txt",
        None,
        DiffOptions::default(),
    )
    .unwrap();
    let lines: Vec<_> = diff.hunks[0]
        .lines
        .iter()
        .map(|l| (l.kind, l.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            (LineKind::Context, "one"),
            (LineKind::Removed, "two"),
            (LineKind::Added, "  two"),
            (LineKind::Context, "three"),
            (LineKind::Added, "four"),
        ]
    );

    let ignore_ws = DiffOptions {
        ignore_whitespace: true,
        ..Default::default()
    };
    let diff = file_diff(
        &repo.git,
        repo.path(),
        Some(&base),
        &target,
        "f.txt",
        None,
        ignore_ws,
    )
    .unwrap();
    let changed: Vec<_> = diff.hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind != LineKind::Context)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(changed, ["four"]);
}

#[test]
fn diffs_root_commits_and_glob_like_names() {
    let repo = TestRepo::new();
    let oid = repo.commit_file("[ab].txt", "star\n", "odd name");
    repo.commit_file("x.txt", "x\n", "unrelated");

    let diff = file_diff(
        &repo.git,
        repo.path(),
        None,
        &oid,
        "[ab].txt",
        None,
        DiffOptions::default(),
    )
    .unwrap();
    assert_eq!(diff.hunks.len(), 1);
    assert_eq!(diff.hunks[0].lines[0].text, "star");
    assert_eq!(diff.hunks[0].lines[0].new_line, Some(1));
}

#[test]
fn diffs_renamed_files_across_paths() {
    let repo = TestRepo::new();
    let base = repo.commit_file("old.txt", "a\nb\nc\nd\ne\n", "base");
    repo.git(&["mv", "old.txt", "new.txt"]);
    repo.write("new.txt", "a\nb\nc\nd\nE\n");
    repo.git(&["add", "-A"]);
    repo.commit(&["-m", "rename"]);
    let target = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

    let diff = file_diff(
        &repo.git,
        repo.path(),
        Some(&base),
        &target,
        "new.txt",
        Some("old.txt"),
        DiffOptions::default(),
    )
    .unwrap();
    let changed: Vec<_> = diff.hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind != LineKind::Context)
        .map(|l| (l.kind, l.text.as_str()))
        .collect();
    assert_eq!(changed, [(LineKind::Removed, "e"), (LineKind::Added, "E")]);
}

#[test]
fn reads_blobs_at_commits() {
    let repo = TestRepo::new();
    let first = repo.commit_file("dir/f.txt", "v1", "one");
    let second = repo.commit_file("dir/f.txt", "v2", "two");

    assert_eq!(
        blob_at(repo.path(), &first, "dir/f.txt")
            .unwrap()
            .as_deref(),
        Some(&b"v1"[..])
    );
    assert_eq!(
        blob_at(repo.path(), &second, "dir/f.txt")
            .unwrap()
            .as_deref(),
        Some(&b"v2"[..])
    );
    assert_eq!(blob_at(repo.path(), &second, "missing.txt").unwrap(), None);
}
