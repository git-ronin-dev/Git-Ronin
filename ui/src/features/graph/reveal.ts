import type { QueryClient } from "@tanstack/react-query";

import { ipc } from "../../lib/ipc";
import { keys } from "../workspace/queries";
import { PAGE_SIZE, loadedRow } from "./useGraphRows";
import { toast } from "../../ui/toast-store";
import { updateView } from "../workspace/view";

type Scroller = (index: number) => void;

/** Each mounted graph registers how to scroll itself to a row. */
const scrollers = new Map<string, Scroller>();

export function registerScroller(repo: string, scroll: Scroller) {
  scrollers.set(repo, scroll);
  return () => {
    if (scrollers.get(repo) === scroll) scrollers.delete(repo);
  };
}

export function scrollToRow(repo: string, index: number) {
  scrollers.get(repo)?.(index);
}

/** Selects a commit and scrolls the graph to it, loading history as needed. */
export async function revealCommit(repo: string, oid: string) {
  try {
    const index = await ipc.graphLocate(repo, oid);
    if (index === null) {
      toast.info("Commit is not in the graph", "It may be hidden by a branch filter.");
      return;
    }
    updateView(repo, { selected: oid, openFile: null });
    scrollToRow(repo, index);
  } catch (err) {
    toast.error("Could not find commit", String(err));
  }
}

/** Selects the row at `index`, fetching its page if it isn't loaded yet. */
export async function selectRow(client: QueryClient, repo: string, index: number) {
  scrollToRow(repo, index);
  const page = Math.floor(index / PAGE_SIZE);
  const row =
    loadedRow(client, repo, index) ??
    (
      await client.fetchQuery({
        queryKey: keys.graph(repo, page),
        queryFn: () => ipc.graphPage(repo, page * PAGE_SIZE, (page + 1) * PAGE_SIZE),
      })
    ).rows[index - page * PAGE_SIZE];
  if (row) updateView(repo, { selected: row.oid, openFile: null });
}
