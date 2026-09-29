import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { CommitDetail } from "../bindings/CommitDetail";
import type { Config } from "../bindings/Config";
import type { DiffOptions } from "../bindings/DiffOptions";
import type { FileDiff } from "../bindings/FileDiff";
import type { GitVersion } from "../bindings/GitVersion";
import type { GraphPage } from "../bindings/GraphPage";
import type { Refs } from "../bindings/Refs";
import type { RepoInfo } from "../bindings/RepoInfo";
import type { UiPrefs } from "../bindings/UiPrefs";

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
};

/** Fires with the repository path when its refs change on disk. */
export function onRepoChanged(handler: (repo: string) => void): Promise<UnlistenFn> {
  return listen<string>("repo-changed", (event) => handler(event.payload));
}
