import { useQueries, type QueryClient } from "@tanstack/react-query";

import type { GraphPage } from "../../bindings/GraphPage";
import type { GraphRow } from "../../bindings/GraphRow";
import { ipc } from "../../lib/ipc";
import { keys } from "../workspace/queries";

export const PAGE_SIZE = 250;

export interface GraphRows {
  /** Rows to give the list: all known rows, plus a page of placeholders while more may exist. */
  count: number;
  complete: boolean;
  maxLanes: number;
  row: (index: number) => GraphRow | undefined;
}

/** Loads the pages covering rows `first..=last` on demand. */
export function useGraphRows(repo: string, first: number, last: number): GraphRows {
  const firstPage = Math.floor(first / PAGE_SIZE);
  const lastPage = Math.max(firstPage, Math.floor(last / PAGE_SIZE));
  const pages = Array.from({ length: lastPage - firstPage + 1 }, (_, i) => firstPage + i);

  const results = useQueries({
    queries: pages.map((page) => ({
      queryKey: keys.graph(repo, page),
      queryFn: () => ipc.graphPage(repo, page * PAGE_SIZE, (page + 1) * PAGE_SIZE),
    })),
  });

  // The most recently fetched page knows the most about the whole graph.
  let latest: GraphPage | undefined;
  let latestAt = -1;
  for (const r of results) {
    if (r.data && r.dataUpdatedAt > latestAt) {
      latest = r.data;
      latestAt = r.dataUpdatedAt;
    }
  }

  const loaded = latest?.loaded ?? 0;
  const complete = latest?.complete ?? false;
  return {
    count: complete ? loaded : loaded + PAGE_SIZE,
    complete,
    maxLanes: latest?.maxLanes ?? 1,
    row: (index) => {
      const page = Math.floor(index / PAGE_SIZE);
      return results[page - firstPage]?.data?.rows[index - page * PAGE_SIZE];
    },
  };
}

/** Index of a commit among the pages already in the cache. */
export function findLoadedIndex(client: QueryClient, repo: string, oid: string): number | null {
  for (const [, page] of client.getQueriesData<GraphPage>({ queryKey: [repo, "graph"] })) {
    const i = page?.rows.findIndex((r) => r.oid === oid) ?? -1;
    if (i >= 0 && page) return page.start + i;
  }
  return null;
}

/** Row at `index` if its page is in the cache. */
export function loadedRow(client: QueryClient, repo: string, index: number): GraphRow | undefined {
  const page = Math.floor(index / PAGE_SIZE);
  return client.getQueryData<GraphPage>(keys.graph(repo, page))?.rows[index - page * PAGE_SIZE];
}
