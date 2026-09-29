mod common;

use common::TestRepo;
use ronin_git::{
    Error, HeadState, Operation, OperationAction, Outcome, Progress, PullMode, PushOutcome,
    PushTarget, ResetMode, add_remote, checkout_branch, checkout_detached, checkout_remote_branch,
    cherry_pick, clone, create_branch, create_tag, delete_branch, delete_remote_ref, delete_tag,
    edit_remote, fetch, init, list_refs, merge, move_branch, open_repo, pending_message, pull,
    push_branch, push_tag, rebase, remove_remote, rename_branch, reset, resolve_operation, revert,
    set_upstream,
};

fn quiet() -> impl FnMut(Progress) {
    |_| {}
}

fn branches(repo: &TestRepo) -> Vec<String> {
    list_refs(&repo.git, repo.path())
        .unwrap()
        .local
        .into_iter()
        .map(|b| b.name)
        .collect()
}

fn current(repo: &TestRepo) -> HeadState {
    open_repo(repo.path()).unwrap().head
}

fn on(name: &str) -> HeadState {
    HeadState::Branch {
        name: name.into(),
        unborn: false,
    }
}

/// A working clone of a bare repository that has one commit on main.
fn published() -> (TestRepo, TestRepo) {
    let remote = TestRepo::bare();
    let seed = TestRepo::clone_of(&remote);
    seed.commit_file("a.txt", "a\n", "first");
    seed.git(&["push", "-q", "origin", "main"]);
    let local = TestRepo::clone_of(&remote);
    (remote, local)
}

#[test]
fn creates_checks_out_renames_and_deletes_branches() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a\n", "first");
    repo.commit_file("a.txt", "b\n", "second");

    create_branch(&repo.git, repo.path(), "topic", &first, false).unwrap();
    assert_eq!(branches(&repo), ["main", "topic"]);
    assert_eq!(current(&repo), on("main"));

    create_branch(&repo.git, repo.path(), "feature/x", "main", true).unwrap();
    assert_eq!(current(&repo), on("feature/x"));

    checkout_branch(&repo.git, repo.path(), "topic").unwrap();
    assert_eq!(current(&repo), on("topic"));
    assert_eq!(repo.read("a.txt"), "a\n");

    rename_branch(&repo.git, repo.path(), "topic", "old-topic").unwrap();
    assert_eq!(current(&repo), on("old-topic"));

    checkout_detached(&repo.git, repo.path(), "main").unwrap();
    assert!(matches!(current(&repo), HeadState::Detached { .. }));

    move_branch(&repo.git, repo.path(), "old-topic", "main").unwrap();
    delete_branch(&repo.git, repo.path(), "old-topic", false).unwrap();
    assert_eq!(branches(&repo), ["feature/x", "main"]);
}

#[test]
fn refuses_unmerged_deletes_and_bad_names() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    repo.git(&["switch", "-q", "-c", "work"]);
    repo.commit_file("a.txt", "b\n", "work");
    repo.git(&["switch", "-q", "main"]);

    let err = delete_branch(&repo.git, repo.path(), "work", false).unwrap_err();
    assert!(
        matches!(err, Error::NotMerged(ref b) if b == "work"),
        "{err}"
    );
    delete_branch(&repo.git, repo.path(), "work", true).unwrap();

    for bad in ["-D", "two words", "", "a..b", "trailing/"] {
        let err = create_branch(&repo.git, repo.path(), bad, "main", false).unwrap_err();
        assert!(matches!(err, Error::Invalid(_)), "{bad}: {err}");
    }
    assert!(create_tag(&repo.git, repo.path(), "bad tag", "main", None).is_err());
    assert!(checkout_detached(&repo.git, repo.path(), "--orphan").is_err());
    assert_eq!(branches(&repo), ["main"]);
}

#[test]
fn creates_and_deletes_tags() {
    let repo = TestRepo::new();
    let oid = repo.commit_file("a.txt", "a\n", "first");

    create_tag(&repo.git, repo.path(), "light", &oid, None).unwrap();
    create_tag(&repo.git, repo.path(), "v1.0", &oid, Some("Release 1.0\n")).unwrap();
    assert_eq!(repo.git(&["cat-file", "-t", "v1.0"]).trim(), "tag");
    assert_eq!(repo.git(&["cat-file", "-t", "light"]).trim(), "commit");
    assert_eq!(
        repo.git(&["tag", "-l", "--format=%(contents)", "v1.0"])
            .trim(),
        "Release 1.0"
    );

    let tags = list_refs(&repo.git, repo.path()).unwrap().tags;
    assert!(tags.iter().all(|t| t.oid == oid));

    delete_tag(&repo.git, repo.path(), "light").unwrap();
    let tags = list_refs(&repo.git, repo.path()).unwrap().tags;
    assert_eq!(tags.len(), 1);
}

#[test]
fn adds_edits_and_removes_remotes() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");

    add_remote(
        &repo.git,
        repo.path(),
        "upstream",
        "https://example.com/a.git",
    )
    .unwrap();
    let remote = &list_refs(&repo.git, repo.path()).unwrap().remotes[0];
    assert_eq!(remote.url.as_deref(), Some("https://example.com/a.git"));
    assert_eq!(remote.push_url, None);

    edit_remote(
        &repo.git,
        repo.path(),
        "upstream",
        "mirror",
        "https://example.com/b.git",
        Some("ssh://example.com/b.git"),
    )
    .unwrap();
    let remote = &list_refs(&repo.git, repo.path()).unwrap().remotes[0];
    assert_eq!(remote.name, "mirror");
    assert_eq!(remote.url.as_deref(), Some("https://example.com/b.git"));
    assert_eq!(remote.push_url.as_deref(), Some("ssh://example.com/b.git"));

    edit_remote(
        &repo.git,
        repo.path(),
        "mirror",
        "mirror",
        "https://example.com/b.git",
        None,
    )
    .unwrap();
    let remote = &list_refs(&repo.git, repo.path()).unwrap().remotes[0];
    assert_eq!(remote.push_url, None);

    remove_remote(&repo.git, repo.path(), "mirror").unwrap();
    assert!(
        list_refs(&repo.git, repo.path())
            .unwrap()
            .remotes
            .is_empty()
    );
}

#[test]
fn fetches_and_pulls_in_each_mode() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a\n", "first");
    let local = TestRepo::clone_of(&origin);

    // Fast-forward.
    origin.commit_file("a.txt", "b\n", "second");
    let mut messages = Vec::new();
    fetch(&local.git, local.path(), None, &mut |p| messages.push(p)).unwrap();
    let refs = list_refs(&local.git, local.path()).unwrap();
    assert_eq!(refs.local[0].upstream.as_ref().unwrap().behind, 1);
    let outcome = pull(
        &local.git,
        local.path(),
        PullMode::FastForwardOnly,
        &mut quiet(),
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(local.read("a.txt"), "b\n");

    // Diverged: fast-forward only refuses, a merge joins, a rebase lines up.
    origin.commit_file("o.txt", "o\n", "origin work");
    local.commit_file("l.txt", "l\n", "local work");
    assert!(
        pull(
            &local.git,
            local.path(),
            PullMode::FastForwardOnly,
            &mut quiet()
        )
        .is_err()
    );

    let before = local.head();
    pull(&local.git, local.path(), PullMode::Default, &mut quiet()).unwrap();
    assert_eq!(
        local
            .git(&["rev-list", "--count", "--merges", "HEAD"])
            .trim(),
        "1"
    );

    local.git(&["reset", "-q", "--hard", &before]);
    // Uncommitted changes are stashed around the pull.
    local.write("a.txt", "dirty\n");
    pull(&local.git, local.path(), PullMode::Rebase, &mut quiet()).unwrap();
    assert_eq!(
        local
            .git(&["rev-list", "--count", "--merges", "HEAD"])
            .trim(),
        "0"
    );
    assert_eq!(
        local.git(&["log", "-1", "--format=%s"]).trim(),
        "local work"
    );
    assert_eq!(local.read("a.txt"), "dirty\n");
}

#[test]
fn pull_that_conflicts_leaves_a_merge_in_progress() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a\n", "first");
    let local = TestRepo::clone_of(&origin);
    origin.commit_file("a.txt", "theirs\n", "theirs");
    local.commit_file("a.txt", "ours\n", "ours");

    let outcome = pull(&local.git, local.path(), PullMode::Merge, &mut quiet()).unwrap();
    assert_eq!(outcome, Outcome::Conflicts);
    assert_eq!(
        open_repo(local.path()).unwrap().operation,
        Some(Operation::Merge)
    );
    resolve_operation(&local.git, local.path(), OperationAction::Abort).unwrap();
    assert_eq!(open_repo(local.path()).unwrap().operation, None);
    assert_eq!(local.read("a.txt"), "ours\n");
}

#[test]
fn pushes_branches_and_tags() {
    let (remote, local) = published();

    local.commit_file("b.txt", "b\n", "second");
    let outcome = push_branch(&local.git, local.path(), "main", None, None, &mut quiet());
    assert_eq!(outcome.unwrap(), PushOutcome::Pushed);
    assert_eq!(remote.head(), local.head());

    // A new branch needs a target, which becomes its upstream.
    local.git(&["switch", "-q", "-c", "topic"]);
    let err = push_branch(&local.git, local.path(), "topic", None, None, &mut quiet());
    assert!(matches!(err, Err(Error::NoUpstream(_))));
    let target = PushTarget {
        remote: "origin".into(),
        branch: "remote-topic".into(),
    };
    push_branch(
        &local.git,
        local.path(),
        "topic",
        Some(&target),
        None,
        &mut quiet(),
    )
    .unwrap();
    let refs = list_refs(&local.git, local.path()).unwrap();
    let topic = refs.local.iter().find(|b| b.name == "topic").unwrap();
    assert_eq!(topic.upstream.as_ref().unwrap().name, "origin/remote-topic");

    create_tag(&local.git, local.path(), "v1", "HEAD", Some("one")).unwrap();
    push_tag(&local.git, local.path(), "origin", "v1", &mut quiet()).unwrap();
    assert!(remote.git(&["tag", "-l"]).contains("v1"));

    delete_remote_ref(
        &local.git,
        local.path(),
        "origin",
        "refs/tags/v1",
        &mut quiet(),
    )
    .unwrap();
    delete_remote_ref(
        &local.git,
        local.path(),
        "origin",
        "refs/heads/remote-topic",
        &mut quiet(),
    )
    .unwrap();
    assert_eq!(remote.git(&["tag", "-l"]).trim(), "");
    let heads = remote.git(&["for-each-ref", "--format=%(refname:short)", "refs/heads"]);
    assert_eq!(heads.trim(), "main");
}

#[test]
fn rejected_push_can_be_forced_over_the_commit_seen() {
    let (remote, local) = published();
    let seen = local.git(&["rev-parse", "origin/main"]).trim().to_owned();
    let other = TestRepo::clone_of(&remote);
    other.commit_file("o.txt", "o\n", "other");
    other.git(&["push", "-q", "origin", "main"]);

    local.commit_file("l.txt", "l\n", "local");
    let outcome = push_branch(&local.git, local.path(), "main", None, None, &mut quiet());
    assert_eq!(outcome.unwrap(), PushOutcome::Rejected);

    // The remote moved past the commit the user saw: nothing is replaced.
    let forced = push_branch(
        &local.git,
        local.path(),
        "main",
        None,
        Some(&seen),
        &mut quiet(),
    );
    let err = forced.unwrap_err().to_string();
    assert!(err.contains("changed since"), "{err}");
    assert_ne!(remote.head(), local.head());

    // Once fetched (and so seen), it can be replaced; no reflog needed.
    fetch(&local.git, local.path(), Some("origin"), &mut quiet()).unwrap();
    let seen = local.git(&["rev-parse", "origin/main"]).trim().to_owned();
    let forced = push_branch(
        &local.git,
        local.path(),
        "main",
        None,
        Some(&seen),
        &mut quiet(),
    );
    assert_eq!(forced.unwrap(), PushOutcome::Pushed);
    assert_eq!(remote.head(), local.head());
    assert!(
        push_branch(
            &local.git,
            local.path(),
            "main",
            None,
            Some("-f"),
            &mut quiet()
        )
        .is_err()
    );
}

#[test]
fn clones_and_inits() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a\n", "first");
    let parent = tempfile::TempDir::new().unwrap();

    let url = origin.path().to_str().unwrap();
    let mut updates = 0;
    let dest = clone(&origin.git, url, parent.path(), "copy", &mut |_| {
        updates += 1
    })
    .unwrap();
    assert_eq!(dest, parent.path().join("copy"));
    assert_eq!(std::fs::read_to_string(dest.join("a.txt")).unwrap(), "a\n");
    assert!(clone(&origin.git, url, parent.path(), "copy", &mut quiet()).is_err());
    assert!(clone(&origin.git, url, parent.path(), "../x", &mut quiet()).is_err());

    let fresh = parent.path().join("new/nested");
    init(&origin.git, &fresh).unwrap();
    let info = open_repo(&fresh).unwrap();
    assert!(matches!(info.head, HeadState::Branch { unborn: true, .. }));
}

#[test]
fn checks_out_remote_branches_with_tracking() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a\n", "first");
    origin.git(&["branch", "-q", "feature"]);
    let local = TestRepo::clone_of(&origin);

    checkout_remote_branch(
        &local.git,
        local.path(),
        "refs/remotes/origin/feature",
        "feature",
    )
    .unwrap();
    assert_eq!(current(&local), on("feature"));
    let refs = list_refs(&local.git, local.path()).unwrap();
    let feature = refs.local.iter().find(|b| b.name == "feature").unwrap();
    assert_eq!(feature.upstream.as_ref().unwrap().name, "origin/feature");

    set_upstream(&local.git, local.path(), "feature", None).unwrap();
    set_upstream(&local.git, local.path(), "main", Some("origin/feature")).unwrap();
    let refs = list_refs(&local.git, local.path()).unwrap();
    let get = |n: &str| refs.local.iter().find(|b| b.name == n).unwrap().clone();
    assert_eq!(get("feature").upstream, None);
    assert_eq!(get("main").upstream.unwrap().name, "origin/feature");
}

#[test]
fn merges_rebases_and_resolves_conflicts() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    repo.git(&["switch", "-q", "-c", "topic"]);
    repo.commit_file("t.txt", "t\n", "topic work");
    repo.git(&["switch", "-q", "main"]);

    // Fast-forward, then an explicit merge commit.
    assert_eq!(
        merge(&repo.git, repo.path(), "topic", false).unwrap(),
        Outcome::Done
    );
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]).trim(), "topic work");
    repo.git(&["reset", "-q", "--hard", "HEAD~"]);
    merge(&repo.git, repo.path(), "topic", true).unwrap();
    assert_eq!(
        repo.git(&["rev-list", "--count", "--merges", "HEAD"])
            .trim(),
        "1"
    );
    repo.git(&["reset", "-q", "--hard", "HEAD~"]);

    // Conflicting change on main: the merge stops and can be concluded.
    repo.commit_file("t.txt", "main\n", "main work");
    assert_eq!(
        merge(&repo.git, repo.path(), "topic", false).unwrap(),
        Outcome::Conflicts
    );
    assert_eq!(
        open_repo(repo.path()).unwrap().operation,
        Some(Operation::Merge)
    );
    let message = pending_message(repo.path()).unwrap().unwrap();
    assert!(message.starts_with("Merge branch 'topic'"), "{message}");
    repo.write("t.txt", "both\n");
    repo.git(&["add", "t.txt"]);
    let outcome = resolve_operation(&repo.git, repo.path(), OperationAction::Continue).unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(open_repo(repo.path()).unwrap().operation, None);
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]).trim(),
        "Merge branch 'topic'"
    );
    assert!(resolve_operation(&repo.git, repo.path(), OperationAction::Abort).is_err());

    // Rebase topic onto main: it conflicts, then skipping its commit empties it.
    repo.git(&["switch", "-q", "topic"]);
    repo.commit_file("u.txt", "u\n", "more topic work");
    repo.git(&["reset", "-q", "--hard", "HEAD~"]);
    repo.git(&["switch", "-q", "main"]);
    repo.git(&["reset", "-q", "--hard", "HEAD~"]);
    repo.git(&["switch", "-q", "topic"]);
    assert_eq!(
        rebase(&repo.git, repo.path(), "main").unwrap(),
        Outcome::Conflicts
    );
    assert_eq!(
        open_repo(repo.path()).unwrap().operation,
        Some(Operation::Rebase)
    );
    resolve_operation(&repo.git, repo.path(), OperationAction::Skip).unwrap();
    assert_eq!(open_repo(repo.path()).unwrap().operation, None);
    assert_eq!(repo.head(), repo.git(&["rev-parse", "main"]).trim());
}

#[test]
fn cherry_picks_reverts_and_resets() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a\n", "first");
    repo.git(&["switch", "-q", "-c", "topic"]);
    let pick = repo.commit_file("b.txt", "b\n", "add b");
    repo.git(&["switch", "-q", "main"]);

    assert_eq!(
        cherry_pick(&repo.git, repo.path(), &pick).unwrap(),
        Outcome::Done
    );
    assert_eq!(repo.read("b.txt"), "b\n");
    let picked = repo.head();

    assert_eq!(
        revert(&repo.git, repo.path(), &picked).unwrap(),
        Outcome::Done
    );
    assert!(!repo.path().join("b.txt").exists());
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]).trim(),
        "Revert \"add b\""
    );

    // A merge commit is cherry-picked relative to its first parent.
    repo.git(&["switch", "-q", "-c", "side", &first]);
    repo.commit_file("c.txt", "c\n", "add c");
    repo.git(&["switch", "-q", "-c", "merged", &first]);
    repo.commit_file("d.txt", "d\n", "add d");
    repo.git(&["merge", "-q", "--no-ff", "--no-edit", "side"]);
    let merge_commit = repo.head();
    repo.git(&["switch", "-q", "main"]);
    assert_eq!(
        cherry_pick(&repo.git, repo.path(), &merge_commit).unwrap(),
        Outcome::Done
    );
    assert_eq!(repo.read("c.txt"), "c\n");
    assert!(!repo.path().join("d.txt").exists());

    reset(&repo.git, repo.path(), &first, ResetMode::Soft).unwrap();
    assert_eq!(repo.head(), first);
    assert!(!repo.git(&["diff", "--cached", "--name-only"]).is_empty());
    reset(&repo.git, repo.path(), &first, ResetMode::Mixed).unwrap();
    assert!(repo.git(&["diff", "--cached", "--name-only"]).is_empty());
    assert_eq!(repo.read("c.txt"), "c\n");
    repo.git(&["add", "-A"]);
    reset(&repo.git, repo.path(), &first, ResetMode::Hard).unwrap();
    assert!(!repo.path().join("c.txt").exists());
    assert!(repo.git(&["status", "--porcelain"]).is_empty());
}
