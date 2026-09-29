//! Git engine for Git Ronin.
//!
//! Hot read paths (opening, history walks, object access) go through gix.
//! Everything that mutates the repository or talks to the network goes through
//! the system `git` binary, so hooks, credential helpers, signing and user
//! config behave exactly as on the command line. Porcelain-style queries whose
//! output git already computes well (tracking info, diffs honouring
//! `.gitattributes`) also use the CLI.

mod cli;
mod detail;
mod diff;
mod error;
mod graph;
mod lanes;
mod refs;
mod repo;

pub use cli::{GitCli, GitVersion, MIN_GIT_VERSION};
pub use detail::{CommitDetail, FileChange, FileStatus, Signature, blob_at, commit_detail};
pub use diff::{DiffLine, DiffOptions, FileDiff, Hunk, LineKind, file_diff, parse_unified};
pub use error::{Error, Result};
pub use graph::{Graph, GraphFilter, GraphPage, GraphRow, RefKind, RefLabel};
pub use lanes::{Edge, EdgeKind};
pub use refs::{
    LocalBranch, Refs, Remote, RemoteBranch, Stash, Submodule, Tag, Upstream, list_refs,
};
pub use repo::{HeadState, RepoInfo, open_repo};
