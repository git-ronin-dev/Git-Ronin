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
import type { LineSelection } from "../bindings/LineSelection";
import type { PatchTarget } from "../bindings/PatchTarget";
import type { Refs } from "../bindings/Refs";
import type { RepoInfo } from "../bindings/RepoInfo";
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
};

/** Fires with the repository path when its refs change on disk. */
export function onRepoChanged(handler: (repo: string) => void): Promise<UnlistenFn> {
  return listen<string>("repo-changed", (event) => handler(event.payload));
}

/** Fires with the repository path when its index or working tree change on disk. */
export function onWorktreeChanged(handler: (repo: string) => void): Promise<UnlistenFn> {
  return listen<string>("worktree-changed", (event) => handler(event.payload));
}
