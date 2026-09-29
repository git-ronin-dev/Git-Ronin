import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";

import type { Config } from "../../bindings/Config";
import type { UiPrefs } from "../../bindings/UiPrefs";
import { ipc } from "../../lib/ipc";
import { toast } from "../../ui/toast-store";

/**
 * Query keys start with the repository path so a whole repository can be
 * invalidated at once. Commit and diff data is keyed by immutable ids and
 * never goes stale.
 */
export const keys = {
  config: ["config"] as const,
  info: (repo: string) => [repo, "info"] as const,
  refs: (repo: string) => [repo, "refs"] as const,
  graph: (repo: string, page: number) => [repo, "graph", page] as const,
  search: (repo: string, query: string, byPath: boolean) =>
    [repo, "search", query, byPath] as const,
  commit: (repo: string, oid: string) => [repo, "commit", oid] as const,
};

const LIVE = new Set(["info", "refs", "graph", "search"]);

/** Refetches everything that depends on a repository's refs. */
export function invalidateRepo(client: QueryClient, repo: string) {
  return client.invalidateQueries({
    predicate: (q) => q.queryKey[0] === repo && LIVE.has(q.queryKey[1] as string),
  });
}

export function useRepoInfo(repo: string) {
  return useQuery({ queryKey: keys.info(repo), queryFn: () => ipc.repoInfo(repo) });
}

export function useRefs(repo: string) {
  return useQuery({ queryKey: keys.refs(repo), queryFn: () => ipc.listRefs(repo) });
}

export function useConfig() {
  return useQuery({ queryKey: keys.config, queryFn: ipc.configGet, staleTime: Infinity });
}

export function useUiPrefs(): UiPrefs | undefined {
  return useConfig().data?.portable.ui;
}

/** Saves UI preferences, applying them immediately. */
export function useSetUiPrefs() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ipc.configSetUi,
    onMutate: (ui) => {
      client.setQueryData<Config>(keys.config, (c) =>
        c ? { ...c, portable: { ...c.portable, ui } } : c,
      );
    },
    onError: (err) => {
      toast.error("Could not save settings", String(err));
      void client.invalidateQueries({ queryKey: keys.config });
    },
  });
}
