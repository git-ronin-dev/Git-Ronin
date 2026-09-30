use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::path::Path;

use gix::ObjectId;
use gix::traverse::commit::simple::{CommitTimeOrder, Sorting as SimpleSorting};
use gix::traverse::commit::topo::{Builder, Sorting};
use gix::traverse::commit::{Info, Simple, Topo};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::gix_err;
use crate::lanes::{Edge, LaneLayout};
use crate::repo::discover;
use crate::{GitCli, Result, list_refs};

/// Which refs contribute their history to the graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct GraphFilter {
    /// Full ref names whose history is left out.
    pub hidden: Vec<String>,
    /// When non-empty, only these refs are shown and `hidden` is ignored.
    pub solo: Vec<String>,
}

impl GraphFilter {
    fn includes(&self, full_name: &str) -> bool {
        if self.solo.is_empty() {
            !self.hidden.iter().any(|h| h == full_name)
        } else {
            self.solo.iter().any(|s| s == full_name)
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GraphRow {
    pub oid: String,
    pub parents: Vec<String>,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    /// Author time, seconds since the Unix epoch.
    #[ts(type = "number")]
    pub time: i64,
    pub refs: Vec<RefLabel>,
    pub lane: u16,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RefLabel {
    pub name: String,
    pub full_name: String,
    pub kind: RefKind,
    /// The checked-out branch, or HEAD itself when detached.
    pub current: bool,
    /// For remote branches, the remote's name; `name` is `<remote>/<branch>`.
    pub remote: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
    /// A stash entry; `name` is `stash@{n}`, `full_name` is `refs/stash`.
    Stash,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GraphPage {
    pub start: u32,
    pub rows: Vec<GraphRow>,
    /// Rows computed so far; the total once `complete`.
    pub loaded: u32,
    pub complete: bool,
    pub max_lanes: u16,
}

fn label(name: &str, full_name: &str, kind: RefKind, current: bool) -> RefLabel {
    RefLabel {
        name: name.to_owned(),
        full_name: full_name.to_owned(),
        kind,
        current,
        remote: None,
    }
}

type NoFilter = fn(&gix::hash::oid) -> bool;

/// With a commit-graph file, a true topological walk starts instantly. Without
/// one it must first read the whole history (seconds on large repos), so we
/// stream by commit time instead, like plain `git log`. That order only breaks
/// "children before parents" under clock skew, which costs a missing edge.
enum Walk {
    // Both are large; boxed so the enum isn't.
    Topo(Box<Topo<gix::OdbHandle, NoFilter>>),
    ByTime(Box<TieOrdered>),
}

impl Iterator for Walk {
    type Item = Result<Info>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Walk::Topo(w) => w.next().map(|r| r.map_err(gix_err)),
            Walk::ByTime(w) => w.next(),
        }
    }
}

/// Runs of equal commit times longer than this are passed on as they come.
const MAX_TIE_RUN: usize = 10_000;

/// A walk by commit time whose commits with equal times come children first.
///
/// Commit time alone can't order commits made in the same second, which a
/// rebase or a script produces routinely; from two tips, a commit could then
/// come before its own child and lose its edge. Each run of equal times is
/// buffered and put in topological order, keeping the walk's order otherwise.
struct TieOrdered {
    inner: Simple<gix::OdbHandle, NoFilter>,
    /// The first commit after the current run.
    next: Option<Result<Info>>,
    ready: VecDeque<Info>,
}

impl TieOrdered {
    fn new(inner: Simple<gix::OdbHandle, NoFilter>) -> Self {
        Self {
            inner,
            next: None,
            ready: VecDeque::new(),
        }
    }

    fn pull(&mut self) -> Option<Result<Info>> {
        self.next
            .take()
            .or_else(|| self.inner.next().map(|r| r.map_err(gix_err)))
    }
}

impl Iterator for TieOrdered {
    type Item = Result<Info>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(info) = self.ready.pop_front() {
            return Some(Ok(info));
        }
        let first = match self.pull()? {
            Ok(info) => info,
            Err(e) => return Some(Err(e)),
        };
        let Some(time) = first.commit_time else {
            return Some(Ok(first));
        };
        let mut run = vec![first];
        while run.len() < MAX_TIE_RUN {
            match self.pull() {
                Some(Ok(info)) if info.commit_time == Some(time) => run.push(info),
                other => {
                    self.next = other;
                    break;
                }
            }
        }
        self.ready = children_first(run).into();
        self.ready.pop_front().map(Ok)
    }
}

/// Orders `run` so every commit comes after its children within the run,
/// otherwise keeping the order it came in.
fn children_first(run: Vec<Info>) -> Vec<Info> {
    if run.len() < 2 {
        return run;
    }
    let index: HashMap<ObjectId, usize> = run.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
    let mut children = vec![0usize; run.len()];
    for commit in &run {
        for parent in &commit.parent_ids {
            if let Some(&p) = index.get(parent) {
                children[p] += 1;
            }
        }
    }
    let mut free: BinaryHeap<Reverse<usize>> = (0..run.len())
        .filter(|&i| children[i] == 0)
        .map(Reverse)
        .collect();
    let mut order = Vec::with_capacity(run.len());
    while let Some(Reverse(i)) = free.pop() {
        order.push(i);
        for parent in &run[i].parent_ids {
            if let Some(&p) = index.get(parent) {
                children[p] -= 1;
                if children[p] == 0 {
                    free.push(Reverse(p));
                }
            }
        }
    }
    let mut slots: Vec<Option<Info>> = run.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

/// An incrementally computed commit graph. Rows are produced on demand, so
/// opening a huge repository costs only the rows actually looked at.
pub struct Graph {
    repo: gix::Repository,
    walk: Option<Walk>,
    layout: LaneLayout<ObjectId>,
    labels: HashMap<ObjectId, Vec<RefLabel>>,
    rows: Vec<GraphRow>,
    row_of: HashMap<ObjectId, u32>,
    /// Stash commits and the commit each was made on, their only parent here.
    stashes: HashMap<ObjectId, ObjectId>,
    /// The index and untracked-files commits of stashes: never shown.
    stash_parts: HashSet<ObjectId>,
}

impl Graph {
    pub fn open(git: &GitCli, path: &Path, filter: &GraphFilter) -> Result<Self> {
        let repo = discover(path)?;
        let refs = list_refs(git, path)?;

        let mut labels: HashMap<ObjectId, Vec<RefLabel>> = HashMap::new();
        let mut tips = Vec::new();
        let mut add = |oid: &str, label: RefLabel| -> Result<()> {
            let oid = ObjectId::from_hex(oid.as_bytes()).map_err(gix_err)?;
            let (kind, include) = (label.kind, filter.includes(&label.full_name));
            labels.entry(oid).or_default().push(label);
            if kind == RefKind::Head || include {
                tips.push(oid);
            }
            Ok(())
        };

        let head = repo.head().map_err(gix_err)?;
        if head.is_detached()
            && let Some(id) = head.id()
            && filter.solo.is_empty()
        {
            add(&id.to_string(), label("HEAD", "HEAD", RefKind::Head, true))?;
        }
        for b in &refs.local {
            add(
                &b.oid,
                label(&b.name, &b.full_name, RefKind::Local, b.is_head),
            )?;
        }
        for remote in &refs.remotes {
            for b in &remote.branches {
                let name = format!("{}/{}", remote.name, b.name);
                add(
                    &b.oid,
                    RefLabel {
                        remote: Some(remote.name.clone()),
                        ..label(&name, &b.full_name, RefKind::Remote, false)
                    },
                )?;
            }
        }
        for t in &refs.tags {
            add(&t.oid, label(&t.name, &t.full_name, RefKind::Tag, false))?;
        }

        // A stash is drawn as a commit on the one it was made on. Its other
        // parents (the index, and untracked files) are git's bookkeeping.
        let mut stashes = HashMap::new();
        let mut stash_parts = HashSet::new();
        if filter.includes("refs/stash") {
            for s in &refs.stashes {
                let id = ObjectId::from_hex(s.oid.as_bytes()).map_err(gix_err)?;
                let Ok(commit) = repo.find_commit(id) else {
                    continue;
                };
                let mut parents = commit.parent_ids().map(|p| p.detach());
                let Some(base) = parents.next() else { continue };
                stash_parts.extend(parents);
                stashes.insert(id, base);
                let name = format!("stash@{{{}}}", s.index);
                add(&s.oid, label(&name, "refs/stash", RefKind::Stash, false))?;
            }
        }

        // Tags may point at trees or blobs; only commits start a walk.
        let mut seen = HashSet::new();
        tips.retain(|id| seen.insert(*id) && repo.find_commit(*id).is_ok());

        let walk = match repo.commit_graph_if_enabled().ok().flatten() {
            Some(commit_graph) => Walk::Topo(Box::new(
                Builder::from_iters(repo.objects.clone(), tips, None::<Vec<ObjectId>>)
                    .sorting(Sorting::DateOrder)
                    .with_commit_graph(Some(commit_graph))
                    .build()
                    .map_err(gix_err)?,
            )),
            None => Walk::ByTime(Box::new(TieOrdered::new(
                Simple::new(tips, repo.objects.clone())
                    .sorting(SimpleSorting::ByCommitTime(CommitTimeOrder::NewestFirst))
                    .map_err(gix_err)?,
            ))),
        };

        Ok(Self {
            repo,
            walk: Some(walk),
            layout: LaneLayout::new(),
            labels,
            rows: Vec::new(),
            row_of: HashMap::new(),
            stashes,
            stash_parts,
        })
    }

    /// Returns rows `start..end`, computing them if needed.
    pub fn page(&mut self, start: usize, end: usize) -> Result<GraphPage> {
        self.load_until(end)?;
        let end = end.min(self.rows.len());
        let start = start.min(end);
        Ok(GraphPage {
            start: start as u32,
            rows: self.rows[start..end].to_vec(),
            loaded: self.rows.len() as u32,
            complete: self.walk.is_none(),
            max_lanes: self.layout.max_lanes(),
        })
    }

    /// Row index of a commit, if it has been loaded.
    pub fn row_of(&self, oid: &str) -> Option<u32> {
        let id = ObjectId::from_hex(oid.as_bytes()).ok()?;
        self.row_of.get(&id).copied()
    }

    /// Row index of a commit, loading further history until it is found.
    /// `None` if the commit isn't part of this graph.
    pub fn locate(&mut self, oid: &str) -> Result<Option<u32>> {
        let id = ObjectId::from_hex(oid.as_bytes()).map_err(gix_err)?;
        while !self.row_of.contains_key(&id) && self.walk.is_some() {
            let target = self.rows.len() + 1000;
            self.load_until(target)?;
        }
        Ok(self.row_of.get(&id).copied())
    }

    /// Row indices of commits matching `query` in their id prefix, author or message.
    pub fn search(&mut self, query: &str) -> Result<Vec<u32>> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        self.load_until(usize::MAX)?;
        let is_hex = query.len() >= 4 && query.bytes().all(|b| b.is_ascii_hexdigit());
        let mut hits = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            let matched = (is_hex && row.oid.starts_with(&query))
                || row.author_name.to_lowercase().contains(&query)
                || row.author_email.to_lowercase().contains(&query)
                || self.full_message(&row.oid)?.to_lowercase().contains(&query);
            if matched {
                hits.push(i as u32);
            }
        }
        Ok(hits)
    }

    /// Row indices of commits that touch `pathspec`.
    pub fn search_path(&mut self, git: &GitCli, pathspec: &str) -> Result<Vec<u32>> {
        self.load_until(usize::MAX)?;
        let workdir = self
            .repo
            .workdir()
            .unwrap_or(self.repo.git_dir())
            .to_owned();
        let output = git.run(&workdir, ["log", "--all", "--format=%H", "--", pathspec])?;
        let touching: HashSet<&str> = output.lines().collect();
        Ok(self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| touching.contains(row.oid.as_str()))
            .map(|(i, _)| i as u32)
            .collect())
    }

    fn full_message(&self, oid: &str) -> Result<String> {
        let id = ObjectId::from_hex(oid.as_bytes()).map_err(gix_err)?;
        let commit = self.repo.find_commit(id).map_err(gix_err)?;
        Ok(commit.message_raw_sloppy().to_string())
    }

    fn load_until(&mut self, n: usize) -> Result<()> {
        while self.rows.len() < n {
            let Some(walk) = self.walk.as_mut() else {
                break;
            };
            match walk.next() {
                None => self.walk = None,
                Some(info) => {
                    let info = info?;
                    if self.stash_parts.contains(&info.id) {
                        continue;
                    }
                    let row = match self.stashes.get(&info.id) {
                        Some(&base) => self.make_row(info.id, &[base])?,
                        None => self.make_row(info.id, &info.parent_ids)?,
                    };
                    self.row_of.insert(info.id, self.rows.len() as u32);
                    self.rows.push(row);
                }
            }
        }
        Ok(())
    }

    fn make_row(&mut self, id: ObjectId, parents: &[ObjectId]) -> Result<GraphRow> {
        let commit = self.repo.find_commit(id).map_err(gix_err)?;
        let author = commit.author().map_err(gix_err)?.trim();
        let summary = commit.message().map_err(gix_err)?.summary().to_string();
        // A parent already placed (clock skew in a by-time walk) can't be
        // connected downwards; leave the edge out rather than open a lane
        // that never closes.
        let pending: Vec<ObjectId> = parents
            .iter()
            .filter(|p| !self.row_of.contains_key(*p))
            .copied()
            .collect();
        let placement = self.layout.place(&id, &pending);
        Ok(GraphRow {
            oid: id.to_string(),
            parents: parents.iter().map(ToString::to_string).collect(),
            summary,
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
            time: author.seconds(),
            refs: self.labels.remove(&id).unwrap_or_default(),
            lane: placement.lane,
            edges: placement.edges,
        })
    }
}
