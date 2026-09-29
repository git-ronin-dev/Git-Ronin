use std::collections::{HashMap, HashSet};
use std::path::Path;

use gix::ObjectId;
use gix::traverse::commit::Topo;
use gix::traverse::commit::topo::{Builder, Sorting};
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
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

type Walk = Topo<gix::OdbHandle, fn(&gix::hash::oid) -> bool>;

/// An incrementally computed commit graph. Rows are produced on demand, so
/// opening a huge repository costs only the rows actually looked at.
pub struct Graph {
    repo: gix::Repository,
    walk: Option<Walk>,
    layout: LaneLayout<ObjectId>,
    labels: HashMap<ObjectId, Vec<RefLabel>>,
    rows: Vec<GraphRow>,
}

impl Graph {
    pub fn open(git: &GitCli, path: &Path, filter: &GraphFilter) -> Result<Self> {
        let repo = discover(path)?;
        let refs = list_refs(git, path)?;

        let mut labels: HashMap<ObjectId, Vec<RefLabel>> = HashMap::new();
        let mut tips = Vec::new();
        let mut add = |oid: &str, name: &str, full_name: &str, kind, current| -> Result<()> {
            let oid = ObjectId::from_hex(oid.as_bytes()).map_err(gix_err)?;
            labels.entry(oid).or_default().push(RefLabel {
                name: name.to_owned(),
                full_name: full_name.to_owned(),
                kind,
                current,
            });
            if kind == RefKind::Head || filter.includes(full_name) {
                tips.push(oid);
            }
            Ok(())
        };

        let head = repo.head().map_err(gix_err)?;
        if head.is_detached()
            && let Some(id) = head.id()
            && filter.solo.is_empty()
        {
            add(&id.to_string(), "HEAD", "HEAD", RefKind::Head, true)?;
        }
        for b in &refs.local {
            add(&b.oid, &b.name, &b.full_name, RefKind::Local, b.is_head)?;
        }
        for remote in &refs.remotes {
            for b in &remote.branches {
                let name = format!("{}/{}", remote.name, b.name);
                add(&b.oid, &name, &b.full_name, RefKind::Remote, false)?;
            }
        }
        for t in &refs.tags {
            add(&t.oid, &t.name, &t.full_name, RefKind::Tag, false)?;
        }

        // Tags may point at trees or blobs; only commits start a walk.
        let mut seen = HashSet::new();
        tips.retain(|id| seen.insert(*id) && repo.find_commit(*id).is_ok());

        let walk = Builder::from_iters(repo.objects.clone(), tips, None::<Vec<ObjectId>>)
            .sorting(Sorting::DateOrder)
            .with_commit_graph(repo.commit_graph_if_enabled().ok().flatten())
            .build()
            .map_err(gix_err)?;

        Ok(Self {
            repo,
            walk: Some(walk),
            layout: LaneLayout::new(),
            labels,
            rows: Vec::new(),
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
                    let info = info.map_err(gix_err)?;
                    let row = self.make_row(info.id, &info.parent_ids)?;
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
        let placement = self.layout.place(&id, parents);
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
