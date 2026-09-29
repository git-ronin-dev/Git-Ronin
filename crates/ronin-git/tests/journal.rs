mod common;

use common::TestRepo;
use ronin_git::{
    CommitOptions, HeadState, Journal, JournalStep, Outcome, PullMode, ResetMode, UndoStyle,
    commit, create_branch, create_tag, delete_branch, merge, open_repo, pull, rename_branch, reset,
    snapshot,
};

/// Runs `action` and records it in `journal`, as the app does.
fn act(
    journal: &mut Journal,
    repo: &TestRepo,
    label: &str,
    style: UndoStyle,
    action: impl FnOnce(),
) {
    let before = snapshot(&repo.git, repo.path()).unwrap();
    action();
    let after = snapshot(&repo.git, repo.path()).unwrap();
    journal.record(&repo.git, repo.path(), label, &before, &after, style);
}

fn undo_label(journal: &mut Journal, repo: &TestRepo) -> Option<JournalStep> {
    journal.state(&repo.git, repo.path()).unwrap().undo
}

fn head(repo: &TestRepo) -> HeadState {
    open_repo(repo.path()).unwrap().head
}

fn on(name: &str) -> HeadState {
    HeadState::Branch {
        name: name.into(),
        unborn: false,
    }
}

#[test]
fn undoes_and_redoes_a_commit_keeping_it_staged() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a\n", "first");
    let mut journal = Journal::default();

    repo.write("a.txt", "b\n");
    repo.git(&["add", "a.txt"]);
    act(&mut journal, &repo, "Commit", UndoStyle::Soft, || {
        commit(&repo.git, repo.path(), "second", CommitOptions::default()).unwrap();
    });
    let second = repo.head();
    assert_eq!(undo_label(&mut journal, &repo).unwrap().label, "Commit");

    assert_eq!(journal.undo(&repo.git, repo.path()).unwrap(), "Commit");
    assert_eq!(repo.head(), first);
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]).trim(),
        "a.txt"
    );
    let state = journal.state(&repo.git, repo.path()).unwrap();
    assert_eq!(state.undo, None);
    assert_eq!(state.redo.unwrap().label, "Commit");

    journal.redo(&repo.git, repo.path()).unwrap();
    assert_eq!(repo.head(), second);
    assert!(repo.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn undoes_the_first_commit() {
    let repo = TestRepo::new();
    repo.write("a.txt", "a\n");
    repo.git(&["add", "a.txt"]);
    let mut journal = Journal::default();
    act(&mut journal, &repo, "Commit", UndoStyle::Soft, || {
        commit(&repo.git, repo.path(), "first", CommitOptions::default()).unwrap();
    });
    journal.undo(&repo.git, repo.path()).unwrap();
    assert!(matches!(
        head(&repo),
        HeadState::Branch { unborn: true, .. }
    ));
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]).trim(),
        "a.txt"
    );
    journal.redo(&repo.git, repo.path()).unwrap();
    assert_eq!(head(&repo), on("main"));
}

#[test]
fn undoes_branch_creation_with_checkout() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    let mut journal = Journal::default();

    act(
        &mut journal,
        &repo,
        "Create branch",
        UndoStyle::Checkout,
        || {
            create_branch(&repo.git, repo.path(), "topic", "HEAD", true).unwrap();
        },
    );
    assert_eq!(head(&repo), on("topic"));

    journal.undo(&repo.git, repo.path()).unwrap();
    assert_eq!(head(&repo), on("main"));
    assert!(repo.git(&["branch", "--list", "topic"]).is_empty());

    journal.redo(&repo.git, repo.path()).unwrap();
    assert_eq!(head(&repo), on("topic"));
}

#[test]
fn restores_a_deleted_branch_with_its_upstream() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a\n", "first");
    origin.git(&["branch", "-q", "feature"]);
    let repo = TestRepo::clone_of(&origin);
    repo.git(&["branch", "-q", "--track", "feature", "origin/feature"]);
    let mut journal = Journal::default();

    act(
        &mut journal,
        &repo,
        "Delete branch",
        UndoStyle::Keep,
        || {
            delete_branch(&repo.git, repo.path(), "feature", false).unwrap();
        },
    );
    journal.undo(&repo.git, repo.path()).unwrap();
    let upstream = repo.git(&["rev-parse", "--abbrev-ref", "feature@{upstream}"]);
    assert_eq!(upstream.trim(), "origin/feature");

    journal.redo(&repo.git, repo.path()).unwrap();
    assert!(repo.git(&["branch", "--list", "feature"]).is_empty());
}

#[test]
fn redo_recreates_a_tracking_branch_with_its_upstream() {
    let origin = TestRepo::new();
    origin.commit_file("a.txt", "a\n", "first");
    origin.git(&["branch", "-q", "feature"]);
    let repo = TestRepo::clone_of(&origin);
    let mut journal = Journal::default();

    act(
        &mut journal,
        &repo,
        "Check out",
        UndoStyle::Checkout,
        || {
            ronin_git::checkout_remote_branch(
                &repo.git,
                repo.path(),
                "refs/remotes/origin/feature",
                "feature",
            )
            .unwrap();
        },
    );
    journal.undo(&repo.git, repo.path()).unwrap();
    assert!(repo.git(&["branch", "--list", "feature"]).is_empty());
    journal.redo(&repo.git, repo.path()).unwrap();
    assert_eq!(head(&repo), on("feature"));
    let upstream = repo.git(&["rev-parse", "--abbrev-ref", "feature@{upstream}"]);
    assert_eq!(upstream.trim(), "origin/feature");
}

#[test]
fn undoes_resets_merges_renames_and_tags() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a\n", "first");
    let second = repo.commit_file("a.txt", "b\n", "second");
    let mut journal = Journal::default();

    act(&mut journal, &repo, "Hard reset", UndoStyle::Keep, || {
        reset(&repo.git, repo.path(), &first, ResetMode::Hard).unwrap();
    });
    act(&mut journal, &repo, "Undo me", UndoStyle::Keep, || {});
    assert_eq!(undo_label(&mut journal, &repo).unwrap().label, "Hard reset");
    journal.undo(&repo.git, repo.path()).unwrap();
    assert_eq!(repo.head(), second);
    assert_eq!(repo.read("a.txt"), "b\n");

    act(&mut journal, &repo, "Mixed reset", UndoStyle::Mixed, || {
        reset(&repo.git, repo.path(), &first, ResetMode::Mixed).unwrap();
    });
    journal.undo(&repo.git, repo.path()).unwrap();
    assert_eq!(repo.head(), second);
    assert!(repo.git(&["status", "--porcelain"]).is_empty());

    repo.git(&["switch", "-q", "-c", "topic", &first]);
    repo.commit_file("t.txt", "t\n", "topic");
    repo.git(&["switch", "-q", "main"]);
    act(&mut journal, &repo, "Merge", UndoStyle::Keep, || {
        assert_eq!(
            merge(&repo.git, repo.path(), "topic", false).unwrap(),
            Outcome::Done
        );
    });
    journal.undo(&repo.git, repo.path()).unwrap();
    assert_eq!(repo.head(), second);
    assert!(!repo.path().join("t.txt").exists());

    rename_branch(&repo.git, repo.path(), "topic", "renamed").unwrap();
    journal.record_rename("Rename", "topic", "renamed");
    journal.undo(&repo.git, repo.path()).unwrap();
    assert!(!repo.git(&["branch", "--list", "topic"]).is_empty());
    journal.redo(&repo.git, repo.path()).unwrap();
    assert!(!repo.git(&["branch", "--list", "renamed"]).is_empty());

    act(&mut journal, &repo, "Tag", UndoStyle::Keep, || {
        create_tag(&repo.git, repo.path(), "v1", "HEAD", Some("one")).unwrap();
    });
    journal.undo(&repo.git, repo.path()).unwrap();
    assert!(repo.git(&["tag", "-l"]).is_empty());
    journal.redo(&repo.git, repo.path()).unwrap();
    assert_eq!(repo.git(&["cat-file", "-t", "v1"]).trim(), "tag");
}

#[test]
fn keep_mode_undo_refuses_to_overwrite_local_changes() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a\n", "first");
    repo.commit_file("a.txt", "b\n", "second");
    let mut journal = Journal::default();
    act(&mut journal, &repo, "Hard reset", UndoStyle::Keep, || {
        reset(&repo.git, repo.path(), &first, ResetMode::Hard).unwrap();
    });
    repo.write("a.txt", "precious\n");
    assert!(journal.undo(&repo.git, repo.path()).is_err());
    assert_eq!(repo.read("a.txt"), "precious\n");

    // Still there to retry once the change is out of the way.
    repo.git(&["checkout", "--", "a.txt"]);
    journal.undo(&repo.git, repo.path()).unwrap();
    assert_eq!(repo.read("a.txt"), "b\n");
}

#[test]
fn blocks_undo_once_pushed_but_not_for_pulled_commits() {
    let remote = TestRepo::bare();
    let seed = TestRepo::clone_of(&remote);
    seed.commit_file("a.txt", "a\n", "first");
    seed.git(&["push", "-q", "origin", "main"]);
    let repo = TestRepo::clone_of(&remote);
    let mut journal = Journal::default();

    // Pulled commits were on the remote all along; undoing the pull is fine.
    seed.commit_file("a.txt", "b\n", "second");
    seed.git(&["push", "-q", "origin", "main"]);
    act(&mut journal, &repo, "Pull", UndoStyle::Keep, || {
        pull(&repo.git, repo.path(), PullMode::Default, &mut |_| {}).unwrap();
    });
    assert_eq!(undo_label(&mut journal, &repo).unwrap().blocked, None);

    repo.write("c.txt", "c\n");
    repo.git(&["add", "c.txt"]);
    act(&mut journal, &repo, "Commit", UndoStyle::Soft, || {
        commit(&repo.git, repo.path(), "third", CommitOptions::default()).unwrap();
    });
    assert_eq!(undo_label(&mut journal, &repo).unwrap().blocked, None);

    repo.git(&["push", "-q", "origin", "main"]);
    let step = undo_label(&mut journal, &repo).unwrap();
    assert!(step.blocked.unwrap().contains("pushed"));
    let before = repo.head();
    assert!(journal.undo(&repo.git, repo.path()).is_err());
    assert_eq!(repo.head(), before);
}

#[test]
fn forgets_history_when_the_repository_moves_underneath() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a\n", "first");
    let mut journal = Journal::default();
    act(&mut journal, &repo, "Branch", UndoStyle::Keep, || {
        create_branch(&repo.git, repo.path(), "topic", "HEAD", false).unwrap();
    });
    // Moved outside the app.
    repo.commit_file("a.txt", "b\n", "second");
    repo.git(&["branch", "-q", "-f", "topic", "HEAD"]);

    assert_eq!(undo_label(&mut journal, &repo), None);
    assert!(journal.undo(&repo.git, repo.path()).is_err());
}

#[test]
fn does_not_record_operations_that_stopped_on_conflicts() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a\n", "first");
    repo.git(&["switch", "-q", "-c", "topic"]);
    repo.commit_file("a.txt", "topic\n", "topic");
    repo.git(&["switch", "-q", "main"]);
    repo.commit_file("a.txt", "main\n", "main");
    let mut journal = Journal::default();
    act(&mut journal, &repo, "Reset", UndoStyle::Soft, || {
        reset(&repo.git, repo.path(), "HEAD", ResetMode::Soft).unwrap();
    });
    act(&mut journal, &repo, "Branch", UndoStyle::Keep, || {
        create_branch(&repo.git, repo.path(), "old", &first, false).unwrap();
    });
    act(&mut journal, &repo, "Merge", UndoStyle::Keep, || {
        assert_eq!(
            merge(&repo.git, repo.path(), "topic", false).unwrap(),
            Outcome::Conflicts
        );
    });
    let state = journal.state(&repo.git, repo.path()).unwrap();
    assert_eq!((state.undo, state.redo), (None, None));
}
