//! Git engine for Git Ronin.
//!
//! Hot read paths (opening, history walks, object access) go through gix.
//! Everything that mutates the repository or talks to the network goes through
//! the system `git` binary, so hooks, credential helpers, signing and user
//! config behave exactly as on the command line. Porcelain-style queries whose
//! output git already computes well (tracking info, diffs honouring
//! `.gitattributes`) also use the CLI.

mod cli;
mod commit;
mod detail;
mod diff;
mod error;
mod gitignore;
mod graph;
mod lanes;
mod refs;
mod repo;
mod stage;
mod stash;
mod status;

pub use cli::{GitCli, GitVersion, MIN_GIT_VERSION};
pub use commit::{CommitOptions, commit, head_message};
pub use detail::{
    BlobSource, CommitDetail, FileChange, FileStatus, Signature, blob_at, commit_detail,
    working_blob,
};
pub use diff::{
    DiffLine, DiffOptions, FileDiff, Hunk, LineKind, file_diff, parse_unified, working_diff,
};
pub use error::{Error, Result};
pub use gitignore::{IgnoreScope, add_to_gitignore, ignore_pattern};
pub use graph::{Graph, GraphFilter, GraphPage, GraphRow, RefKind, RefLabel};
pub use lanes::{Edge, EdgeKind};
pub use refs::{
    LocalBranch, Refs, Remote, RemoteBranch, Stash, Submodule, Tag, Upstream, list_refs,
};
pub use repo::{HeadState, RepoInfo, git_dirs, open_repo};
pub use stage::{
    LineSelection, PatchTarget, apply_lines, build_patch, discard_files, stage_files, unstage_files,
};
pub use stash::{StashOptions, stash_apply, stash_drop, stash_push};
pub use status::{StatusEntry, SubmoduleChange, WorkingStatus, status};
