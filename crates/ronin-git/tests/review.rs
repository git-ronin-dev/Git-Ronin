mod common;

use common::TestRepo;
use ronin_git::{
    FileStatus, branch_issue, fetch_commits, has_commit, range_files, set_branch_issue,
};

#[test]
fn fetches_pull_request_refs_and_lists_their_changes() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "one\n", "base");
    let base = origin.head();
    origin.git(&["switch", "-q", "-c", "feature"]);
    origin.commit_file("a.txt", "one\ntwo\n", "change a");
    origin.commit_file("b.txt", "new\n", "add b");
    let head = origin.head();
    // What a hosting service does: keep the head under a ref no branch has.
    origin.git(&["update-ref", "refs/pull/1/head", &head]);
    origin.git(&["switch", "-q", "main"]);
    origin.git(&["branch", "-q", "-D", "feature"]);
    origin.commit_file("c.txt", "later\n", "main moves on");

    // A local clone would share the object store; fetching doesn't.
    let local = TestRepo::new();
    let url = origin.path().to_str().unwrap().to_owned();
    local.git(&["remote", "add", "origin", &url]);
    local.git(&["fetch", "-q", "origin"]);
    assert!(!has_commit(&local.git, local.path(), &head).unwrap());
    let source = url.as_str();
    fetch_commits(
        &local.git,
        local.path(),
        source,
        &["refs/pull/1/head"],
        &[&head],
        &mut |_| {},
    )
    .unwrap();
    assert!(has_commit(&local.git, local.path(), &head).unwrap());
    // Already here: nothing to fetch, even from a source that doesn't exist.
    fetch_commits(
        &local.git,
        local.path(),
        "/nowhere",
        &["refs/x"],
        &[&head],
        &mut |_| {},
    )
    .unwrap();

    let diff = range_files(&local.git, local.path(), "origin/main", &head).unwrap();
    assert_eq!(diff.base, base);
    assert_eq!(diff.head, head);
    let files: Vec<(&str, FileStatus)> = diff
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.status))
        .collect();
    assert_eq!(
        files,
        [
            ("a.txt", FileStatus::Modified),
            ("b.txt", FileStatus::Added)
        ]
    );

    let err = fetch_commits(
        &local.git,
        local.path(),
        source,
        &["+refs/x:refs/y"],
        &[],
        &mut |_| {},
    );
    assert!(err.is_err());
    let missing = "0123456789012345678901234567890123456789";
    let err = fetch_commits(
        &local.git,
        local.path(),
        source,
        &["refs/heads/main"],
        &[missing],
        &mut |_| {},
    );
    assert!(err.unwrap_err().to_string().contains("not on the remote"));
}

#[test]
fn remembers_the_issue_a_branch_is_for() {
    let repo = TestRepo::new();
    repo.commit_file("a", "a", "a");
    repo.git(&["branch", "12-fix-login"]);
    assert_eq!(
        branch_issue(&repo.git, repo.path(), "12-fix-login").unwrap(),
        None
    );
    set_branch_issue(&repo.git, repo.path(), "12-fix-login", Some("#12")).unwrap();
    assert_eq!(
        branch_issue(&repo.git, repo.path(), "12-fix-login")
            .unwrap()
            .as_deref(),
        Some("#12")
    );
    repo.git(&["branch", "-m", "12-fix-login", "login"]);
    assert_eq!(
        branch_issue(&repo.git, repo.path(), "login")
            .unwrap()
            .as_deref(),
        Some("#12")
    );
    set_branch_issue(&repo.git, repo.path(), "login", None).unwrap();
    set_branch_issue(&repo.git, repo.path(), "login", None).unwrap();
    assert_eq!(branch_issue(&repo.git, repo.path(), "login").unwrap(), None);
}
