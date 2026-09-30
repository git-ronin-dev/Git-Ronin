mod common;

use common::TestRepo;
use ronin_git::summary;

#[test]
fn summarises_branch_tracking_and_changes() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a", "first");
    let local = TestRepo::clone_of(&origin);
    local.commit_file("b.txt", "b", "local work");
    origin.commit_file("a.txt", "c", "upstream work");
    origin.commit_file("a.txt", "d", "more upstream work");
    local.git(&["fetch", "-q"]);

    local.write("b.txt", "changed");
    local.write("new/one.txt", "1");
    local.write("new/two.txt", "2");
    local.write("staged.txt", "s");
    local.git(&["add", "staged.txt"]);

    let s = summary(&local.git, local.path()).unwrap();
    assert_eq!(s.upstream.as_deref(), Some("origin/main"));
    assert_eq!((s.ahead, s.behind), (1, 2));
    // b.txt, and the untracked directory once.
    assert_eq!((s.staged, s.unstaged, s.conflicted), (1, 2, 0));
}

#[test]
fn summarises_a_repository_without_upstream_or_commits() {
    let repo = TestRepo::new();
    let s = summary(&repo.git, repo.path()).unwrap();
    assert_eq!(s.upstream, None);
    assert_eq!((s.ahead, s.behind, s.staged, s.unstaged), (0, 0, 0, 0));
    assert!(summary(&repo.git, &repo.path().join("missing")).is_err());
}
