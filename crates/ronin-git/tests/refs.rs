mod common;

use common::TestRepo;
use ronin_git::{Upstream, list_refs};

#[test]
fn lists_local_branches_with_tracking() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a", "first");
    origin.commit_file("a.txt", "b", "second");
    let local = TestRepo::clone_of(&origin);

    // local: one commit ahead of origin/main; origin gets one more -> behind after fetch.
    local.commit_file("b.txt", "b", "local work");
    origin.commit_file("a.txt", "c", "upstream work");
    local.git(&["fetch", "-q"]);
    local.git(&["branch", "-q", "topic"]);

    let refs = list_refs(&local.git, local.path()).unwrap();
    let names: Vec<_> = refs.local.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["main", "topic"]);

    let main = &refs.local[0];
    assert!(main.is_head);
    assert_eq!(main.full_name, "refs/heads/main");
    assert_eq!(
        main.upstream,
        Some(Upstream {
            name: "origin/main".into(),
            ahead: 1,
            behind: 1,
            gone: false
        })
    );
    assert!(!refs.local[1].is_head);
    assert_eq!(refs.local[1].upstream, None);
}

#[test]
fn groups_remote_branches_by_remote() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a", "first");
    origin.git(&["branch", "-q", "feature/x"]);
    let local = TestRepo::clone_of(&origin);
    // Remote names may contain slashes.
    let src = origin.path().to_str().unwrap();
    local.git(&["remote", "add", "team/mirror", src]);
    local.git(&["fetch", "-q", "team/mirror"]);

    let refs = list_refs(&local.git, local.path()).unwrap();
    let remotes: Vec<_> = refs
        .remotes
        .iter()
        .map(|r| {
            let branches: Vec<_> = r.branches.iter().map(|b| b.name.as_str()).collect();
            (r.name.as_str(), branches)
        })
        .collect();
    assert_eq!(
        remotes,
        [
            ("origin", vec!["feature/x", "main"]),
            ("team/mirror", vec!["feature/x", "main"]),
        ]
    );
}

#[test]
fn peels_annotated_tags() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a", "first");
    let second = repo.commit_file("a.txt", "b", "second");
    repo.git(&["tag", "light", &first]);
    repo.git(&["tag", "-a", "v1.0", "-m", "release"]);

    let refs = list_refs(&repo.git, repo.path()).unwrap();
    let tags: Vec<_> = refs
        .tags
        .iter()
        .map(|t| (t.name.as_str(), t.oid.as_str()))
        .collect();
    assert_eq!(tags, [("light", first.as_str()), ("v1.0", second.as_str())]);
}

#[test]
fn lists_stashes_newest_first() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a", "first");
    repo.write("a.txt", "one");
    repo.git(&["stash", "push", "-q", "-m", "older"]);
    repo.write("a.txt", "two");
    repo.git(&["stash", "push", "-q", "-m", "newer"]);

    let refs = list_refs(&repo.git, repo.path()).unwrap();
    let stashes: Vec<_> = refs
        .stashes
        .iter()
        .map(|s| (s.index, s.message.as_str()))
        .collect();
    assert_eq!(stashes, [(0, "On main: newer"), (1, "On main: older")]);
}

#[test]
fn lists_submodules() {
    let lib = TestRepo::new();
    lib.commit_file("lib.txt", "lib", "lib");
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a", "first");
    let src = lib.path().to_str().unwrap();
    repo.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "-q",
        src,
        "vendor/lib",
    ]);

    let refs = list_refs(&repo.git, repo.path()).unwrap();
    let subs: Vec<_> = refs
        .submodules
        .iter()
        .map(|s| (s.name.as_str(), s.path.as_str()))
        .collect();
    assert_eq!(subs, [("vendor/lib", "vendor/lib")]);
}

#[test]
fn empty_repo_has_no_refs() {
    let repo = TestRepo::new();
    let refs = list_refs(&repo.git, repo.path()).unwrap();
    assert!(refs.local.is_empty() && refs.tags.is_empty() && refs.stashes.is_empty());
}
