import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { ipc } from "../../lib/ipc";
import { keys, useRefs, useRepoInfo } from "../workspace/queries";
import { repoContext, type RepoContext } from "./menus";

/** The next undo and redo steps. */
export function useJournal(repo: string) {
  return useQuery({ queryKey: keys.journal(repo), queryFn: () => ipc.journalState(repo) });
}

/** The message prepared for concluding a stopped merge, cherry-pick or revert. */
export function usePendingMessage(repo: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.pendingMessage(repo),
    queryFn: () => ipc.pendingMessage(repo),
    enabled,
  });
}

export function useRepoContext(repo: string): RepoContext {
  const info = useRepoInfo(repo).data;
  const refs = useRefs(repo).data;
  return useMemo(() => repoContext(repo, info, refs), [repo, info, refs]);
}
