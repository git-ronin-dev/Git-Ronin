import { invoke } from "@tauri-apps/api/core";

import type { GitVersion } from "../bindings/GitVersion";
import type { RepoInfo } from "../bindings/RepoInfo";

/** Typed wrappers for the Rust commands in src-tauri/src/commands.rs. */
export const ipc = {
  gitVersion: () => invoke<GitVersion>("git_version"),
  openRepo: (path: string) => invoke<RepoInfo>("open_repo", { path }),
};
