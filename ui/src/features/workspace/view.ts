import { create } from "zustand";

import type { FileChange } from "../../bindings/FileChange";

export interface Search {
  query: string;
  byPath: boolean;
}

/** `selected` value for the uncommitted-changes row. */
export const WORKING_COPY = "working-copy";

/** A file whose diff replaces the graph. */
export type OpenFile =
  | { kind: "commit"; oid: string; file: FileChange }
  | { kind: "working"; path: string; staged: boolean };

/** What the user is looking at in one repository tab. */
export interface RepoView {
  /** A commit id, or `WORKING_COPY`. */
  selected: string | null;
  openFile: OpenFile | null;
  search: Search | null;
}

const empty: RepoView = { selected: null, openFile: null, search: null };

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
