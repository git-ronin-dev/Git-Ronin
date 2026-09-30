import { create } from "zustand";

import type { FileChange } from "../../bindings/FileChange";

export interface Search {
  query: string;
  byPath: boolean;
}

/** `selected` value for the uncommitted-changes row. */
export const WORKING_COPY = "working-copy";

/** A file whose diff (or conflict, blame, history) replaces the graph. */
export type OpenFile =
  | { kind: "commit"; oid: string; file: FileChange }
  | { kind: "working"; path: string; staged: boolean }
  | { kind: "conflict"; path: string }
  /** `rev` null blames the working tree. */
  | { kind: "blame"; path: string; rev: string | null }
  | { kind: "history"; path: string };

/** What the user is looking at in one repository tab. */
export interface RepoView {
  /** A commit id, or `WORKING_COPY`. */
  selected: string | null;
  openFile: OpenFile | null;
  search: Search | null;
  /** An interactive rebase being planned, onto `base` (the root when null). */
  rebase: { base: string | null } | null;
  /** The terminal panel is open. */
  terminal: boolean;
}

const empty: RepoView = {
  selected: null,
  openFile: null,
  search: null,
  rebase: null,
  terminal: false,
};

interface ViewState {
  views: Record<string, RepoView>;
  update: (repo: string, patch: Partial<RepoView>) => void;
}

export const useViews = create<ViewState>()((set) => ({
  views: {},
  update: (repo, patch) =>
    set((s) => ({ views: { ...s.views, [repo]: { ...(s.views[repo] ?? empty), ...patch } } })),
}));

export function useRepoView(repo: string): RepoView {
  return useViews((s) => s.views[repo] ?? empty);
}

export function updateView(repo: string, patch: Partial<RepoView>) {
  useViews.getState().update(repo, patch);
}
