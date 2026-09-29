use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::error::gix_err;
use crate::repo::discover;
use crate::{GitCli, Result};

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Refs {
    pub local: Vec<LocalBranch>,
    pub remotes: Vec<Remote>,
    pub tags: Vec<Tag>,
    pub stashes: Vec<Stash>,
    pub submodules: Vec<Submodule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LocalBranch {
    pub name: String,
    pub full_name: String,
    pub oid: String,
    pub is_head: bool,
    pub upstream: Option<Upstream>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Upstream {
    /// Short name, e.g. `origin/main`.
    pub name: String,
    pub ahead: u32,
    pub behind: u32,
    /// The upstream branch was deleted on the remote.
    pub gone: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Remote {
    pub name: String,
    pub branches: Vec<RemoteBranch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteBranch {
    /// Branch name without the remote prefix.
    pub name: String,
    pub full_name: String,
    pub oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Tag {
    pub name: String,
    pub full_name: String,
    /// The object the tag ultimately points at (annotated tags are peeled).
    pub oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Stash {
    pub index: u32,
    pub oid: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Submodule {
    pub name: String,
    pub path: String,
}

const REF_FORMAT: &str = "--format=%(refname)%00%(objectname)%00%(*objectname)%00\
%(upstream:short)%00%(upstream:track,nobracket)%00%(HEAD)";

/// Lists branches, remotes, tags, stashes and submodules.
pub fn list_refs(git: &GitCli, path: &Path) -> Result<Refs> {
    let repo = discover(path)?;
    let mut refs = Refs::default();

    // Remote names may contain '/', so they come from config rather than ref names.
    let mut remote_names: Vec<String> = repo.remote_names().iter().map(|n| n.to_string()).collect();
    refs.remotes = remote_names
        .iter()
        .map(|name| Remote {
            name: name.clone(),
            branches: Vec::new(),
        })
        .collect();
    // Longest first, so `origin/sub` wins over `origin` for `refs/remotes/origin/sub/x`.
    remote_names.sort_by_key(|n| std::cmp::Reverse(n.len()));

    let output = git.run(
        path,
        [
            "for-each-ref",
            REF_FORMAT,
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    )?;
    for line in output.lines() {
        let fields: Vec<&str> = line.split('\0').collect();
        let [full_name, oid, peeled, upstream, track, head] = fields[..] else {
            continue;
        };
        if let Some(name) = full_name.strip_prefix("refs/heads/") {
            refs.local.push(LocalBranch {
                name: name.to_owned(),
                full_name: full_name.to_owned(),
                oid: oid.to_owned(),
                is_head: head == "*",
                upstream: (!upstream.is_empty()).then(|| parse_upstream(upstream, track)),
            });
        } else if let Some(rest) = full_name.strip_prefix("refs/remotes/") {
            let Some(remote) = remote_names.iter().find(|r| {
                rest.strip_prefix(r.as_str())
                    .is_some_and(|b| b.starts_with('/'))
            }) else {
                continue;
            };
            let name = &rest[remote.len() + 1..];
            // `origin/HEAD` is a pointer to the default branch, not a branch.
            if name == "HEAD" {
                continue;
            }
            if let Some(r) = refs.remotes.iter_mut().find(|r| &r.name == remote) {
                r.branches.push(RemoteBranch {
                    name: name.to_owned(),
                    full_name: full_name.to_owned(),
                    oid: oid.to_owned(),
                });
            }
        } else if let Some(name) = full_name.strip_prefix("refs/tags/") {
            refs.tags.push(Tag {
                name: name.to_owned(),
                full_name: full_name.to_owned(),
                oid: if peeled.is_empty() { oid } else { peeled }.to_owned(),
            });
        }
    }

    refs.stashes = git
        .run(path, ["stash", "list", "--format=%H%x00%gs"])?
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let (oid, message) = line.split_once('\0')?;
            Some(Stash {
                index: index as u32,
                oid: oid.to_owned(),
                message: message.to_owned(),
            })
        })
        .collect();

    if let Some(submodules) = repo.submodules().map_err(gix_err)? {
        for sm in submodules {
            refs.submodules.push(Submodule {
                name: sm.name().to_string(),
                path: sm.path().map_err(gix_err)?.to_string(),
            });
        }
    }

    Ok(refs)
}

/// Parses `%(upstream:track,nobracket)`: "", "gone", "ahead 1", "behind 2" or "ahead 1, behind 2".
fn parse_upstream(name: &str, track: &str) -> Upstream {
    let mut upstream = Upstream {
        name: name.to_owned(),
        ahead: 0,
        behind: 0,
        gone: track == "gone",
    };
    for part in track.split(", ") {
        match part.split_once(' ') {
            Some(("ahead", n)) => upstream.ahead = n.parse().unwrap_or(0),
            Some(("behind", n)) => upstream.behind = n.parse().unwrap_or(0),
            _ => {}
        }
    }
    upstream
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tracking_info() {
        let up = |track| parse_upstream("origin/main", track);
        assert_eq!((up("").ahead, up("").behind, up("").gone), (0, 0, false));
        assert!(up("gone").gone);
        assert_eq!(up("ahead 3").ahead, 3);
        assert_eq!(up("behind 2").behind, 2);
        let both = up("ahead 1, behind 4");
        assert_eq!((both.ahead, both.behind), (1, 4));
    }
}
