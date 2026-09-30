import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { ipc } from "../../lib/ipc";
import { keys, useRefs, useRepoInfo } from "../workspace/queries";
import { pickLink, useRepoLinks } from "../hosting/queries";
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

export function useWorktrees(repo: string) {
  return useQuery({ queryKey: keys.worktrees(repo), queryFn: () => ipc.listWorktrees(repo) });
}

export function useFlowConfig(repo: string) {
  return useQuery({ queryKey: keys.flow(repo), queryFn: () => ipc.flowConfig(repo) });
}

export function useLfsStatus(repo: string) {
  return useQuery({ queryKey: keys.lfs(repo), queryFn: () => ipc.lfsStatus(repo) });
}

/** Locks on the LFS server: asked for only when wanted, since it takes a round trip. */
export function useLfsLocks(repo: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.lfsLocks(repo),
    queryFn: () => ipc.lfsLocks(repo),
    enabled,
    staleTime: 60_000,
    retry: false,
  });
}

/** The bisect in progress, if the repository says one is. */
export function useBisect(repo: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.bisect(repo),
    queryFn: () => ipc.bisectState(repo),
    enabled,
  });
}

export function useRepoContext(repo: string): RepoContext {
  const info = useRepoInfo(repo).data;
  const refs = useRefs(repo).data;
  const pullRequests = !!pickLink(useRepoLinks(repo).data, "pullRequests");
  return useMemo(
    () => repoContext(repo, info, refs, pullRequests),
    [repo, info, refs, pullRequests],
  );
}
