//! Git engine for Git Ronin.
//!
//! Hot read paths (opening, history walks, object access) go through gix.
//! Everything that mutates the repository or talks to the network goes through
//! the system `git` binary, so hooks, credential helpers, signing and user
//! config behave exactly as on the command line. Porcelain-style queries whose
//! output git already computes well (tracking info, diffs honouring
//! `.gitattributes`) also use the CLI.

mod bisect;
mod blame;
mod branch;
mod cli;
mod commit;
mod conflict;
mod detail;
mod diff;
mod error;
mod flow;
mod gitignore;
mod graph;
mod history;
mod interactive;
mod journal;
mod lanes;
mod lfs;
mod refs;
mod remote;
mod repo;
mod stage;
mod stash;
mod status;
mod submodule;
mod tag;
mod worktree;

pub use bisect::{BisectMark, BisectState, bisect_mark, bisect_state};
pub use blame::{Blame, BlameCommit, BlameLine, FileCommit, blame, file_log};
pub use branch::{
    checkout_branch, checkout_detached, checkout_remote_branch, create_branch, delete_branch,
    move_branch, rename_branch, set_upstream,
};
pub use cli::{Finished, GitCli, GitVersion, MIN_GIT_VERSION, Progress};
pub use commit::{CommitOptions, CommitResult, commit, head_message};
pub use conflict::{Conflict, ConflictSide, MergeChunk, Resolution, conflict, resolve_conflict};
pub use detail::{
    BlobSource, CommitDetail, FileChange, FileStatus, Signature, blob_at, commit_detail,
    working_blob,
};
pub use diff::{
    DiffLine, DiffOptions, FileDiff, Hunk, LineKind, file_diff, parse_unified, working_diff,
};
pub use error::{Error, Result};
pub use flow::{FlowConfig, FlowKind, flow_config, flow_finish, flow_init, flow_start};
pub use gitignore::{IgnoreScope, add_to_gitignore, ignore_pattern};
pub use graph::{Graph, GraphFilter, GraphPage, GraphRow, RefKind, RefLabel};
pub use history::{
    OperationAction, Outcome, ResetMode, cherry_pick, merge, pending_message, rebase, reset,
    resolve_operation, revert,
};
pub use interactive::{
    RebaseAction, RebaseCommit, RebasePlan, RebaseStep, interactive_rebase, rebase_plan,
};
pub use journal::{Journal, JournalState, JournalStep, Snapshot, UndoStyle, snapshot};
pub use lanes::{Edge, EdgeKind};
pub use lfs::{
    LfsLock, LfsPattern, LfsStatus, lfs_init, lfs_lock, lfs_locks, lfs_status, lfs_track,
    lfs_unlock, lfs_untrack,
};
pub use refs::{
    LocalBranch, Refs, Remote, RemoteBranch, Stash, Submodule, Tag, Upstream, list_refs,
};
pub use remote::{
    PullMode, PushOutcome, PushTarget, add_remote, clone, clone_name, delete_remote_ref,
    edit_remote, fetch, init, pull, push_branch, push_tag, remove_remote,
};
pub use repo::{HeadState, Operation, RebaseProgress, RepoInfo, git_dirs, open_repo};
pub use stage::{
    LineSelection, PatchTarget, apply_lines, build_patch, discard_files, stage_files, unstage_files,
};
pub use stash::{StashOptions, stash_apply, stash_drop, stash_push};
pub use status::{RepoSummary, StatusEntry, SubmoduleChange, WorkingStatus, status, summary};
pub use submodule::{SubmoduleState, add_submodule, update_submodules};
pub use tag::{create_tag, delete_tag};
pub use worktree::{Worktree, add_worktree, list_worktrees, prune_worktrees, remove_worktree};
