import { create } from "zustand";

import type { FileChange } from "../../bindings/FileChange";

export interface Search {
  query: string;
  byPath: boolean;
}

/** What the user is looking at in one repository tab. */
export interface RepoView {
  selected: string | null;
  /** File whose diff replaces the graph, with the commit it belongs to. */
  openFile: { oid: string; file: FileChange } | null;
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
