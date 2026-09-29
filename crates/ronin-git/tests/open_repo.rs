mod common;

use common::TestRepo;
use ronin_git::{Error, HeadState, open_repo};

#[test]
fn fresh_repo_has_unborn_branch() {
    let repo = TestRepo::new();
    let info = open_repo(repo.path()).unwrap();
    assert!(!info.is_bare);
    assert_eq!(
        info.head,
        HeadState::Branch {
            name: "main".into(),
            unborn: true
        }
    );
}

#[test]
fn reports_current_branch_after_commit() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a", "first");
    repo.git(&["switch", "-q", "-c", "feature/x"]);
    let info = open_repo(repo.path()).unwrap();
    assert_eq!(
        info.head,
        HeadState::Branch {
            name: "feature/x".into(),
            unborn: false
        }
    );
}

#[test]
fn reports_detached_head() {
    let repo = TestRepo::new();
    let oid = repo.commit_file("a.txt", "a", "first");
    repo.commit_file("a.txt", "b", "second");
    repo.git(&["checkout", "-q", &oid]);
    assert_eq!(
        open_repo(repo.path()).unwrap().head,
        HeadState::Detached { oid }
    );
}

#[test]
fn discovers_repo_from_subdirectory() {
    let repo = TestRepo::new();
    let sub = repo.path().join("deep/nested");
    std::fs::create_dir_all(&sub).unwrap();
    let info = open_repo(&sub).unwrap();
    assert_eq!(
        std::fs::canonicalize(&info.path).unwrap(),
        std::fs::canonicalize(repo.path()).unwrap()
    );
}

#[test]
fn rejects_non_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    let err = open_repo(dir.path()).unwrap_err();
    assert!(matches!(err, Error::NotARepo(_)), "{err}");
}
