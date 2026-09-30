mod common;

use common::TestRepo;
use ronin_git::{EdgeKind, Graph, GraphFilter, GraphRow, RefKind};

fn all_rows(repo: &TestRepo, filter: &GraphFilter) -> Vec<GraphRow> {
    let mut graph = Graph::open(&repo.git, repo.path(), filter).unwrap();
    let page = graph.page(0, usize::MAX).unwrap();
    assert!(page.complete);
    page.rows
}

fn summaries(rows: &[GraphRow]) -> Vec<&str> {
    rows.iter().map(|r| r.summary.as_str()).collect()
}

/// main: a - b - m (merge of topic)
/// topic:     \- t -/
fn repo_with_merge() -> TestRepo {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a", "a");
    repo.git(&["switch", "-q", "-c", "topic"]);
    repo.commit_file("t.txt", "t", "t");
    repo.git(&["switch", "-q", "main"]);
    repo.commit_file("b.txt", "b", "b");
    // Real clock time: always newer than the fixed test timestamps.
    repo.git(&["merge", "-q", "--no-ff", "-m", "m", "topic"]);
    repo
}

#[test]
fn linear_history_newest_first_in_one_lane() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "1", "one");
    repo.commit_file("a.txt", "2", "two");
    let head = repo.commit_file("a.txt", "3", "three");

    let rows = all_rows(&repo, &GraphFilter::default());
    assert_eq!(summaries(&rows), ["three", "two", "one"]);
    assert!(rows.iter().all(|r| r.lane == 0));
    assert_eq!(rows[0].oid, head);
    assert_eq!(rows[0].parents, [rows[1].oid.clone()]);
    assert_eq!(rows[2].parents, Vec::<String>::new());
    assert_eq!(rows[0].author_name, "Ronin Test");
    assert!(rows[0].time > rows[1].time);

    let label = &rows[0].refs[0];
    assert_eq!(
        (label.name.as_str(), label.kind, label.current),
        ("main", RefKind::Local, true)
    );
}

#[test]
fn commits_made_in_the_same_second_come_after_their_children() {
    let repo = TestRepo::new();
    // Every commit at one timestamp, as a rebase or a script makes them.
    let date = "1700000000 +0000";
    let git = repo
        .git
        .clone()
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date);
    let commit = |name: &str| {
        repo.write(name, name);
        git.run(repo.path(), ["add", name]).unwrap();
        git.run(repo.path(), ["commit", "-q", "-m", name]).unwrap();
    };
    commit("one");
    // `aaa` sorts first among the tips but is the oldest of them.
    repo.git(&["branch", "aaa"]);
    commit("two");
    repo.git(&["branch", "bbb"]);
    commit("three");

    let rows = all_rows(&repo, &GraphFilter::default());
    assert_eq!(summaries(&rows), ["three", "two", "one"]);
    assert!(rows.iter().all(|r| r.lane == 0));
    assert!(
        rows[..2]
            .iter()
            .all(|r| r.edges.iter().any(|e| e.kind == EdgeKind::Out))
    );
}

#[test]
fn merge_opens_and_closes_a_lane() {
    let repo = repo_with_merge();
    let rows = all_rows(&repo, &GraphFilter::default());
    assert_eq!(summaries(&rows), ["m", "b", "t", "a"]);

    let merge = &rows[0];
    assert_eq!(merge.parents.len(), 2);
    let outs: Vec<_> = merge
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Out)
        .map(|e| e.to)
        .collect();
    assert_eq!(outs, [0, 1]);
    assert_eq!(rows[2].lane, 1, "topic commit sits in the second lane");
    assert_eq!(rows[3].lane, 0);
    assert!(
        rows[3]
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::In)
            .count()
            == 2
    );
}

#[test]
fn pages_are_computed_lazily() {
    let repo = TestRepo::new();
    for i in 0..5 {
        repo.commit_file("a.txt", &i.to_string(), &format!("c{i}"));
    }
    let mut graph = Graph::open(&repo.git, repo.path(), &GraphFilter::default()).unwrap();

    let first = graph.page(0, 2).unwrap();
    assert_eq!(summaries(&first.rows), ["c4", "c3"]);
    assert_eq!((first.loaded, first.complete), (2, false));

    let rest = graph.page(2, 100).unwrap();
    assert_eq!(summaries(&rest.rows), ["c2", "c1", "c0"]);
    assert_eq!((rest.start, rest.loaded, rest.complete), (2, 5, true));

    let past_end = graph.page(10, 20).unwrap();
    assert!(past_end.rows.is_empty());
}

#[test]
fn hidden_refs_drop_their_unique_history() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "a", "a");
    repo.git(&["switch", "-q", "-c", "side"]);
    repo.commit_file("s.txt", "s", "side only");
    repo.git(&["switch", "-q", "main"]);
    repo.commit_file("b.txt", "b", "b");

    let hidden = GraphFilter {
        hidden: vec!["refs/heads/side".into()],
        ..Default::default()
    };
    assert_eq!(summaries(&all_rows(&repo, &hidden)), ["b", "a"]);

    let solo = GraphFilter {
        solo: vec!["refs/heads/side".into()],
        ..Default::default()
    };
    assert_eq!(summaries(&all_rows(&repo, &solo)), ["side only", "a"]);
}

#[test]
fn labels_detached_head_and_tags() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a", "a");
    repo.commit_file("a.txt", "b", "b");
    repo.git(&["tag", "-a", "v1", "-m", "release", &first]);
    repo.git(&["checkout", "-q", &first]);

    let rows = all_rows(&repo, &GraphFilter::default());
    let labels: Vec<_> = rows[1]
        .refs
        .iter()
        .map(|l| (l.name.as_str(), l.kind))
        .collect();
    assert_eq!(labels, [("HEAD", RefKind::Head), ("v1", RefKind::Tag)]);
}

#[test]
fn empty_repo_has_no_rows() {
    let repo = TestRepo::new();
    let rows = all_rows(&repo, &GraphFilter::default());
    assert!(rows.is_empty());
}

#[test]
fn searches_message_author_and_id() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "a", "Add parser\n\nHandles UNICODE input");
    repo.commit_file("b.txt", "b", "Fix typo");
    let mut graph = Graph::open(&repo.git, repo.path(), &GraphFilter::default()).unwrap();

    assert_eq!(graph.search("typo").unwrap(), [0]);
    assert_eq!(
        graph.search("unicode").unwrap(),
        [1],
        "searches the body too"
    );
    assert_eq!(graph.search(&first[..8]).unwrap(), [1]);
    assert_eq!(graph.search("ronin test").unwrap(), [0, 1]);
    assert!(graph.search("  ").unwrap().is_empty());
}

#[test]
fn searches_by_path() {
    let repo = TestRepo::new();
    repo.commit_file("src/lib.rs", "1", "lib");
    repo.commit_file("README.md", "r", "docs");
    repo.commit_file("src/lib.rs", "2", "lib again");
    let mut graph = Graph::open(&repo.git, repo.path(), &GraphFilter::default()).unwrap();
    assert_eq!(graph.search_path(&repo.git, "src").unwrap(), [0, 2]);
}

#[test]
fn graph_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Graph>();
}

#[test]
fn topo_walk_with_commit_graph_matches_by_time_walk() {
    let repo = repo_with_merge();
    let by_time = all_rows(&repo, &GraphFilter::default());
    repo.git(&["commit-graph", "write", "--reachable"]);
    let topo = all_rows(&repo, &GraphFilter::default());

    let shape = |rows: &[GraphRow]| -> Vec<(String, u16, usize)> {
        rows.iter()
            .map(|r| (r.summary.clone(), r.lane, r.edges.len()))
            .collect()
    };
    assert_eq!(shape(&topo), shape(&by_time));
}

#[test]
fn clock_skew_does_not_leave_dangling_lanes() {
    let repo = TestRepo::new();
    repo.commit_file("a.txt", "1", "parent");
    // A child dated a day before its parent.
    let git = repo
        .git
        .clone()
        .env("GIT_AUTHOR_DATE", "1690000000 +0000")
        .env("GIT_COMMITTER_DATE", "1690000000 +0000");
    repo.write("a.txt", "2");
    git.run(repo.path(), ["commit", "-q", "-am", "skewed child"])
        .unwrap();

    let rows = all_rows(&repo, &GraphFilter::default());
    assert_eq!(rows.len(), 2);
    let mut graph = Graph::open(&repo.git, repo.path(), &GraphFilter::default()).unwrap();
    assert!(graph.page(0, 10).unwrap().max_lanes <= 2);
    // Whatever the order, the last row must not leave a lane open below it.
    let last = rows.last().unwrap();
    assert!(!last.edges.iter().any(|e| e.kind == EdgeKind::Out));
}

#[test]
fn finds_rows_by_commit_id() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "1", "one");
    repo.commit_file("a.txt", "2", "two");
    let mut graph = Graph::open(&repo.git, repo.path(), &GraphFilter::default()).unwrap();
    graph.page(0, 10).unwrap();
    assert_eq!(graph.row_of(&first), Some(1));
    assert_eq!(
        graph.row_of("0000000000000000000000000000000000000000"),
        None
    );
}

#[test]
fn locate_loads_history_until_found() {
    let repo = TestRepo::new();
    let first = repo.commit_file("a.txt", "0", "c0");
    for i in 1..5 {
        repo.commit_file("a.txt", &i.to_string(), &format!("c{i}"));
    }
    let mut graph = Graph::open(&repo.git, repo.path(), &GraphFilter::default()).unwrap();
    assert_eq!(graph.row_of(&first), None, "nothing loaded yet");
    assert_eq!(graph.locate(&first).unwrap(), Some(4));
    let absent = "0123456789012345678901234567890123456789";
    assert_eq!(graph.locate(absent).unwrap(), None);
}
