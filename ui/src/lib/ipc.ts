import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AccountView } from "../bindings/AccountView";
import type { BisectMark } from "../bindings/BisectMark";
import type { BisectState } from "../bindings/BisectState";
import type { Blame } from "../bindings/Blame";
import type { BlobSource } from "../bindings/BlobSource";
import type { Change } from "../bindings/Change";
import type { CiStatus } from "../bindings/CiStatus";
import type { CommitDetail } from "../bindings/CommitDetail";
import type { CommitOptions } from "../bindings/CommitOptions";
import type { CommitResult } from "../bindings/CommitResult";
import type { Config } from "../bindings/Config";
import type { Conflict } from "../bindings/Conflict";
import type { DeviceStart } from "../bindings/DeviceStart";
import type { DiffOptions } from "../bindings/DiffOptions";
import type { FileCommit } from "../bindings/FileCommit";
import type { FileDiff } from "../bindings/FileDiff";
import type { FlowConfig } from "../bindings/FlowConfig";
import type { FlowKind } from "../bindings/FlowKind";
import type { GitPrefs } from "../bindings/GitPrefs";
import type { GitVersion } from "../bindings/GitVersion";
import type { GraphPage } from "../bindings/GraphPage";
import type { HostedRepo } from "../bindings/HostedRepo";
import type { Hunk } from "../bindings/Hunk";
import type { Identity } from "../bindings/Identity";
import type { IgnoreScope } from "../bindings/IgnoreScope";
import type { ImportMode } from "../bindings/ImportMode";
import type { Issue } from "../bindings/Issue";
import type { JournalState } from "../bindings/JournalState";
import type { Launchpad } from "../bindings/Launchpad";
import type { LfsLock } from "../bindings/LfsLock";
import type { LfsStatus } from "../bindings/LfsStatus";
import type { LineSelection } from "../bindings/LineSelection";
import type { MergeMethod } from "../bindings/MergeMethod";
import type { NewPullRequest } from "../bindings/NewPullRequest";
import type { OperationAction } from "../bindings/OperationAction";
import type { Outcome } from "../bindings/Outcome";
import type { PatchTarget } from "../bindings/PatchTarget";
import type { PrSource } from "../bindings/PrSource";
import type { Profile } from "../bindings/Profile";
import type { ProviderKind } from "../bindings/ProviderKind";
import type { PullMode } from "../bindings/PullMode";
import type { PullRequest } from "../bindings/PullRequest";
import type { PullRequestDetail } from "../bindings/PullRequestDetail";
import type { PushOutcome } from "../bindings/PushOutcome";
import type { PushTarget } from "../bindings/PushTarget";
import type { RangeDiff } from "../bindings/RangeDiff";
import type { RebasePlan } from "../bindings/RebasePlan";
import type { RebaseStep } from "../bindings/RebaseStep";
import type { Refs } from "../bindings/Refs";
import type { RepoInfo } from "../bindings/RepoInfo";
import type { RepoLink } from "../bindings/RepoLink";
import type { RepoSummary } from "../bindings/RepoSummary";
import type { ResetMode } from "../bindings/ResetMode";
import type { Resolution } from "../bindings/Resolution";
import type { SignIn } from "../bindings/SignIn";
import type { SshKey } from "../bindings/SshKey";
import type { StashOptions } from "../bindings/StashOptions";
import type { StatusEntry } from "../bindings/StatusEntry";
import type { SyncStatus } from "../bindings/SyncStatus";
import type { UiPrefs } from "../bindings/UiPrefs";
import type { WorkingStatus } from "../bindings/WorkingStatus";
import type { Workspace } from "../bindings/Workspace";
import type { Worktree } from "../bindings/Worktree";

/** Typed wrappers for the Rust commands in src-tauri/src/commands.rs. */
export const ipc = {
  gitVersion: () => invoke<GitVersion>("git_version"),

  configGet: () => invoke<Config>("config_get"),
  configTakeWarnings: () => invoke<string[]>("config_take_warnings"),
  configSetUi: (ui: UiPrefs) => invoke<void>("config_set_ui", { ui }),
  configSetGit: (git: GitPrefs) => invoke<void>("config_set_git", { git }),
  configSetKeybindings: (keybindings: Record<string, string>) =>
    invoke<void>("config_set_keybindings", { keybindings }),
  /** Resolves to a warning if the active profile could not be applied. */
  configSetProfiles: (profiles: Profile[]) =>
    invoke<string | null>("config_set_profiles", { profiles }),
  configSetTerminalShell: (shell: string) => invoke<void>("config_set_terminal_shell", { shell }),
  /** Closes every repository; restore the new profile's tabs afterwards. */
  profileActivate: (id: string) => invoke<string | null>("profile_activate", { id }),
  gitIdentity: () => invoke<Identity>("git_identity"),

  settingsExport: (path: string) => invoke<void>("settings_export", { path }),
  settingsImportPreview: (path: string, mode: ImportMode) =>
    invoke<Change[]>("settings_import_preview", { path, mode }),
  settingsImport: (path: string, mode: ImportMode) =>
    invoke<string | null>("settings_import", { path, mode }),

  syncStatus: () => invoke<SyncStatus>("sync_status"),
  syncEnable: (remote: string, branch: string, pushMinutes: number) =>
    invoke<SyncStatus>("sync_enable", { remote, branch, pushMinutes }),
  syncDisable: () => invoke<SyncStatus>("sync_disable"),
  syncNow: () => invoke<SyncStatus>("sync_now"),
  /** For each conflict, `true` takes the other machine's value. */
  syncResolve: (takeTheirs: boolean[]) => invoke<SyncStatus>("sync_resolve", { takeTheirs }),

  sshKeys: () => invoke<SshKey[]>("ssh_keys"),
  sshGenerate: (name: string, comment: string, passphrase: string) =>
    invoke<SshKey>("ssh_generate", { name, comment, passphrase }),

  workspacesSet: (workspaces: Workspace[]) => invoke<void>("workspaces_set", { workspaces }),
  repoSummary: (path: string) => invoke<RepoSummary>("repo_summary", { path }),
  fetchPath: (path: string) => invoke<void>("fetch_path", { path }),

  terminalOpen: (cwd: string, cols: number, rows: number, onEvent: (e: TerminalEvent) => void) => {
    const channel = new Channel<TerminalEvent>();
    channel.onmessage = onEvent;
    return invoke<number>("terminal_open", { cwd, cols, rows, onEvent: channel });
  },
  terminalWrite: (id: number, data: string) => invoke<void>("terminal_write", { id, data }),
  terminalResize: (id: number, cols: number, rows: number) =>
    invoke<void>("terminal_resize", { id, cols, rows }),
  terminalClose: (id: number) => invoke<void>("terminal_close", { id }),

  hostingAccounts: () => invoke<AccountView[]>("hosting_accounts"),
  /** The OAuth application built in for a service, if any. */
  hostingClientId: (kind: ProviderKind, url: string) =>
    invoke<string | null>("hosting_client_id", { kind, url }),
  hostingSignIn: (kind: ProviderKind, url: string, login: string, token: string) =>
    invoke<SignIn>("hosting_sign_in", { kind, url, login, token }),
  hostingDeviceStart: (kind: ProviderKind, url: string, clientId: string) =>
    invoke<DeviceStart>("hosting_device_start", { kind, url, clientId }),
  /** Resolves once the user authorized the app in the browser. */
  hostingDeviceWait: (flow: number) => invoke<SignIn>("hosting_device_wait", { flow }),
  hostingDeviceCancel: (flow: number) => invoke<void>("hosting_device_cancel", { flow }),
  hostingSignOut: (id: string) => invoke<void>("hosting_sign_out", { id }),
  hostingMoveAccount: (id: string, profile: string) =>
    invoke<void>("hosting_move_account", { id, profile }),
  hostingRepos: (account: string) => invoke<HostedRepo[]>("hosting_repos", { account }),
  hostingFork: (account: string, path: string) =>
    invoke<HostedRepo>("hosting_fork", { account, path }),
  hostingLinks: (repo: string) => invoke<RepoLink[]>("hosting_links", { repo }),
  hostingPullRequests: (account: string, path: string) =>
    invoke<PullRequest[]>("hosting_pull_requests", { account, path }),
  hostingPullRequest: (account: string, path: string, number: number) =>
    invoke<PullRequestDetail>("hosting_pull_request", { account, path, number }),
  hostingCreatePullRequest: (account: string, path: string, pr: NewPullRequest) =>
    invoke<PullRequest>("hosting_create_pull_request", { account, path, pr }),
  hostingComment: (account: string, path: string, number: number, body: string) =>
    invoke<void>("hosting_comment", { account, path, number, body }),
  hostingApprove: (account: string, path: string, number: number) =>
    invoke<void>("hosting_approve", { account, path, number }),
  hostingMerge: (account: string, path: string, number: number, method: MergeMethod) =>
    invoke<void>("hosting_merge", { account, path, number, method }),
  hostingIssues: (account: string, path: string) =>
    invoke<Issue[]>("hosting_issues", { account, path }),
  hostingCreateIssue: (account: string, path: string, title: string, body: string) =>
    invoke<Issue>("hosting_create_issue", { account, path, title, body }),
  hostingCiStatus: (account: string, path: string, sha: string) =>
    invoke<CiStatus>("hosting_ci_status", { account, path, sha }),
  hostingAddSshKey: (account: string, title: string, key: string) =>
    invoke<void>("hosting_add_ssh_key", { account, title, key }),
  hostingLaunchpad: () => invoke<Launchpad>("hosting_launchpad"),
  prFetch: (args: {
    repo: string;
    remote: string;
    source: PrSource;
    head: string | null;
    base: string | null;
    targetBranch: string;
  }) => invoke<void>("pr_fetch", args),
  rangeFiles: (repo: string, base: string, head: string) =>
    invoke<RangeDiff>("range_files", { repo, base, head }),
  branchIssue: (repo: string, branch: string) =>
    invoke<string | null>("branch_issue", { repo, branch }),
  setBranchIssue: (repo: string, branch: string, issue: string | null) =>
    invoke<void>("set_branch_issue", { repo, branch, issue }),

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
    invoke<CommitResult>("commit", { repo, message, options }),
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

  conflict: (repo: string, path: string) => invoke<Conflict>("conflict", { repo, path }),
  resolveConflict: (repo: string, path: string, resolution: Resolution) =>
    invoke<void>("resolve_conflict", { repo, path, resolution }),

  rebasePlan: (repo: string, base: string | null) =>
    invoke<RebasePlan>("rebase_plan", { repo, base }),
  interactiveRebase: (repo: string, base: string | null, head: string, steps: RebaseStep[]) =>
    invoke<Outcome>("interactive_rebase", { repo, base, head, steps }),

  blame: (repo: string, path: string, rev: string | null, ignoreWhitespace: boolean) =>
    invoke<Blame>("blame", { repo, path, rev, ignoreWhitespace }),
  fileLog: (repo: string, path: string, rev: string | null, skip: number, limit: number) =>
    invoke<FileCommit[]>("file_log", { repo, path, rev, skip, limit }),

  addSubmodule: (repo: string, url: string, path: string) =>
    invoke<void>("add_submodule", { repo, url, path }),
  updateSubmodules: (repo: string, paths: string[]) =>
    invoke<void>("update_submodules", { repo, paths }),

  listWorktrees: (repo: string) => invoke<Worktree[]>("list_worktrees", { repo }),
  addWorktree: (args: {
    repo: string;
    dest: string;
    branch: string;
    create: boolean;
    start: string | null;
  }) => invoke<void>("add_worktree", args),
  removeWorktree: (repo: string, path: string, force: boolean) =>
    invoke<void>("remove_worktree", { repo, path, force }),
  pruneWorktrees: (repo: string) => invoke<void>("prune_worktrees", { repo }),

  lfsStatus: (repo: string) => invoke<LfsStatus>("lfs_status", { repo }),
  lfsInit: (repo: string) => invoke<void>("lfs_init", { repo }),
  lfsTrack: (repo: string, pattern: string) => invoke<void>("lfs_track", { repo, pattern }),
  lfsUntrack: (repo: string, pattern: string, source: string) =>
    invoke<void>("lfs_untrack", { repo, pattern, source }),
  lfsLocks: (repo: string) => invoke<LfsLock[]>("lfs_locks", { repo }),
  lfsLock: (repo: string, path: string) => invoke<void>("lfs_lock", { repo, path }),
  lfsUnlock: (repo: string, id: string, force: boolean) =>
    invoke<void>("lfs_unlock", { repo, id, force }),

  flowConfig: (repo: string) => invoke<FlowConfig | null>("flow_config", { repo }),
  flowInit: (repo: string, config: FlowConfig) => invoke<void>("flow_init", { repo, config }),
  flowStart: (repo: string, kind: FlowKind, name: string) =>
    invoke<void>("flow_start", { repo, kind, name }),
  flowFinish: (args: {
    repo: string;
    kind: FlowKind;
    branch: string;
    tagMessage: string | null;
    keep: boolean;
  }) => invoke<Outcome>("flow_finish", args),

  bisectState: (repo: string) => invoke<BisectState | null>("bisect_state", { repo }),
  bisectMark: (repo: string, mark: BisectMark, rev: string) =>
    invoke<void>("bisect_mark", { repo, mark, rev }),

  credentialRespond: (id: number, answer: string | null) =>
    invoke<void>("credential_respond", { id, answer }),
};

/** Output from a terminal panel's shell, or its exit. */
export type TerminalEvent =
  | { kind: "data"; data: string }
  /** `code` is null when the shell was killed. */
  | { kind: "exit"; code: number | null };

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

/** Fires when the portable settings changed underneath the UI (a sync). */
export function onConfigChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("config-changed", () => handler());
}

/** Fires when the settings sync status changed. */
export function onSyncChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("sync-changed", () => handler());
}
