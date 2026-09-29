import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { BlobSource } from "../bindings/BlobSource";
import type { CommitDetail } from "../bindings/CommitDetail";
import type { CommitOptions } from "../bindings/CommitOptions";
import type { Config } from "../bindings/Config";
import type { DiffOptions } from "../bindings/DiffOptions";
import type { FileDiff } from "../bindings/FileDiff";
import type { GitVersion } from "../bindings/GitVersion";
import type { GraphPage } from "../bindings/GraphPage";
import type { Hunk } from "../bindings/Hunk";
import type { IgnoreScope } from "../bindings/IgnoreScope";
import type { JournalState } from "../bindings/JournalState";
import type { LineSelection } from "../bindings/LineSelection";
import type { OperationAction } from "../bindings/OperationAction";
import type { Outcome } from "../bindings/Outcome";
import type { PatchTarget } from "../bindings/PatchTarget";
import type { PullMode } from "../bindings/PullMode";
import type { PushOutcome } from "../bindings/PushOutcome";
import type { PushTarget } from "../bindings/PushTarget";
import type { Refs } from "../bindings/Refs";
import type { RepoInfo } from "../bindings/RepoInfo";
import type { ResetMode } from "../bindings/ResetMode";
import type { StashOptions } from "../bindings/StashOptions";
import type { StatusEntry } from "../bindings/StatusEntry";
import type { UiPrefs } from "../bindings/UiPrefs";
import type { WorkingStatus } from "../bindings/WorkingStatus";

/** Typed wrappers for the Rust commands in src-tauri/src/commands.rs. */
export const ipc = {
  gitVersion: () => invoke<GitVersion>("git_version"),

  configGet: () => invoke<Config>("config_get"),
  configTakeWarnings: () => invoke<string[]>("config_take_warnings"),
  configSetUi: (ui: UiPrefs) => invoke<void>("config_set_ui", { ui }),

  openRepo: (path: string) => invoke<RepoInfo>("open_repo", { path }),
  restoreTabs: () => invoke<RepoInfo[]>("restore_tabs"),
  closeRepo: (repo: string) => invoke<void>("close_repo", { repo }),
  setActiveTab: (repo: string | null) => invoke<void>("set_active_tab", { repo }),

  repoInfo: (repo: string) => invoke<RepoInfo>("repo_info", { repo }),
  listRefs: (repo: string) => invoke<Refs>("list_refs", { repo }),
  graphPage: (repo: string, start: number, end: number) =>
    invoke<GraphPage>("graph_page", { repo, start, end }),
  graphSearch: (repo: string, query: string, byPath: boolean) =>
    invoke<number[]>("graph_search", { repo, query, byPath }),
  graphLocate: (repo: string, oid: string) => invoke<number | null>("graph_locate", { repo, oid }),
  setGraphFilter: (repo: string, hidden: string[], solo: string[]) =>
    invoke<void>("set_graph_filter", { repo, hidden, solo }),

  commitDetail: (repo: string, oid: string) => invoke<CommitDetail>("commit_detail", { repo, oid }),
  fileDiff: (args: {
    repo: string;
    base: string | null;
    target: string;
    path: string;
    oldPath: string | null;
    options: DiffOptions;
  }) => invoke<FileDiff>("file_diff", args),
  blob: (repo: string, oid: string, path: string) =>
    invoke<ArrayBuffer>("blob", { repo, oid, path }),

  workingStatus: (repo: string) => invoke<WorkingStatus>("working_status", { repo }),
  workingDiff: (repo: string, entry: StatusEntry, staged: boolean, options: DiffOptions) =>
    invoke<FileDiff>("working_diff", { repo, entry, staged, options }),
  workingBlob: (repo: string, path: string, source: BlobSource) =>
    invoke<ArrayBuffer>("working_blob", { repo, path, source }),
  stageFiles: (repo: string, paths: string[]) => invoke<void>("stage_files", { repo, paths }),
  unstageFiles: (repo: string, paths: string[]) => invoke<void>("unstage_files", { repo, paths }),
  discardFiles: (repo: string, entries: StatusEntry[]) =>
    invoke<void>("discard_files", { repo, entries }),
  applyLines: (args: {
    repo: string;
    path: string;
    hunks: Hunk[];
    selection: LineSelection[];
    target: PatchTarget;
  }) => invoke<void>("apply_lines", args),
  commit: (repo: string, message: string, options: CommitOptions) =>
    invoke<string>("commit", { repo, message, options }),
  headMessage: (repo: string) => invoke<string | null>("head_message", { repo }),
  stashPush: (repo: string, options: StashOptions) =>
    invoke<boolean>("stash_push", { repo, options }),
  stashApply: (repo: string, index: number, oid: string, pop: boolean) =>
    invoke<void>("stash_apply", { repo, index, oid, pop }),
  stashDrop: (repo: string, index: number, oid: string) =>
    invoke<void>("stash_drop", { repo, index, oid }),
  addToGitignore: (repo: string, path: string, scope: IgnoreScope) =>
    invoke<string>("add_to_gitignore", { repo, path, scope }),

  cloneRepo: (url: string, parent: string, name: string) =>
    invoke<RepoInfo>("clone_repo", { url, parent, name }),
  cloneName: (url: string) => invoke<string | null>("clone_name", { url }),
  initRepo: (path: string) => invoke<RepoInfo>("init_repo", { path }),

  journalState: (repo: string) => invoke<JournalState>("journal_state", { repo }),
  undo: (repo: string, redo: boolean) => invoke<string>("undo", { repo, redo }),

  createBranch: (repo: string, name: string, start: string, checkout: boolean) =>
    invoke<void>("create_branch", { repo, name, start, checkout }),
  checkoutBranch: (repo: string, name: string) => invoke<void>("checkout_branch", { repo, name }),
  checkoutRemoteBranch: (repo: string, remoteRef: string, name: string) =>
    invoke<void>("checkout_remote_branch", { repo, remoteRef, name }),
  checkoutDetached: (repo: string, rev: string) => invoke<void>("checkout_detached", { repo, rev }),
  renameBranch: (repo: string, old: string, name: string) =>
    invoke<void>("rename_branch", { repo, old, new: name }),
  deleteBranch: (repo: string, name: string, force: boolean) =>
    invoke<void>("delete_branch", { repo, name, force }),
  moveBranch: (repo: string, name: string, rev: string) =>
    invoke<void>("move_branch", { repo, name, rev }),
  setUpstream: (repo: string, name: string, upstream: string | null) =>
    invoke<void>("set_upstream", { repo, name, upstream }),

  createTag: (repo: string, name: string, target: string, message: string | null) =>
    invoke<void>("create_tag", { repo, name, target, message }),
  deleteTag: (repo: string, name: string) => invoke<void>("delete_tag", { repo, name }),

  addRemote: (repo: string, name: string, url: string) =>
    invoke<void>("add_remote", { repo, name, url }),
  editRemote: (args: {
    repo: string;
    name: string;
    newName: string;
    url: string;
    pushUrl: string | null;
  }) => invoke<void>("edit_remote", args),
  removeRemote: (repo: string, name: string) => invoke<void>("remove_remote", { repo, name }),

  fetch: (repo: string, remote: string | null, background: boolean) =>
    invoke<void>("fetch", { repo, remote, background }),
  pull: (repo: string, mode: PullMode) => invoke<Outcome>("pull", { repo, mode }),
  /** `forceOver`: force the push while the remote branch is still at this commit. */
  pushBranch: (repo: string, name: string, target: PushTarget | null, forceOver: string | null) =>
    invoke<PushOutcome>("push_branch", { repo, name, target, forceOver }),
  pushTag: (repo: string, remote: string, name: string) =>
    invoke<PushOutcome>("push_tag", { repo, remote, name }),
  deleteRemoteRef: (repo: string, remote: string, fullRef: string) =>
    invoke<void>("delete_remote_ref", { repo, remote, fullRef }),

  merge: (repo: string, rev: string, noFf: boolean) =>
    invoke<Outcome>("merge", { repo, rev, noFf }),
  rebase: (repo: string, onto: string) => invoke<Outcome>("rebase", { repo, onto }),
  cherryPick: (repo: string, oid: string) => invoke<Outcome>("cherry_pick", { repo, oid }),
  revert: (repo: string, oid: string) => invoke<Outcome>("revert", { repo, oid }),
  reset: (repo: string, rev: string, mode: ResetMode) => invoke<void>("reset", { repo, rev, mode }),
  resolveOperation: (repo: string, action: OperationAction) =>
    invoke<Outcome>("resolve_operation", { repo, action }),
  pendingMessage: (repo: string) => invoke<string | null>("pending_message", { repo }),

  credentialRespond: (id: number, answer: string | null) =>
    invoke<void>("credential_respond", { id, answer }),
};

/** A progress update from a long-running command. */
export interface ProgressEvent {
  /** The repository path, or the destination of a clone. */
  key: string;
  message: string;
  percent: number | null;
}

/** git or ssh asking for a credential; answer with `ipc.credentialRespond`. */
export interface CredentialRequest {
  id: number;
  prompt: string;
  secret: boolean;
}

export function onProgress(handler: (event: ProgressEvent) => void): Promise<UnlistenFn> {
  return listen<ProgressEvent>("progress", (event) => handler(event.payload));
}

export function onCredentialRequest(
  handler: (request: CredentialRequest) => void,
): Promise<UnlistenFn> {
  return listen<CredentialRequest>("credential-request", (event) => handler(event.payload));
}

/** Fires with the repository path when its refs change on disk. */
export function onRepoChanged(handler: (repo: string) => void): Promise<UnlistenFn> {
  return listen<string>("repo-changed", (event) => handler(event.payload));
}

/** Fires with the repository path when its index or working tree change on disk. */
export function onWorktreeChanged(handler: (repo: string) => void): Promise<UnlistenFn> {
  return listen<string>("worktree-changed", (event) => handler(event.payload));
}
