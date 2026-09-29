import { useQueryClient } from "@tanstack/react-query";

import type { Stash } from "../../bindings/Stash";
import type { StashOptions } from "../../bindings/StashOptions";
import { ipc } from "../../lib/ipc";
import { confirm } from "../../ui/confirm-store";
import { toast } from "../../ui/toast-store";
import { invalidateRepo } from "../workspace/queries";

/** Stash actions. Each reports failures itself and resolves to success. */
export function useStashActions(repo: string) {
  const client = useQueryClient();

  const run = async (title: string, action: () => Promise<unknown>): Promise<boolean> => {
    try {
      await action();
      return true;
    } catch (err) {
      toast.error(title, String(err));
      return false;
    } finally {
      void invalidateRepo(client, repo);
    }
  };

  return {
    push: (options: StashOptions) =>
      run("Could not stash changes", async () => {
        if (!(await ipc.stashPush(repo, options))) toast.info("Nothing to stash");
      }),

    apply: (stash: Stash, pop: boolean) =>
      run(pop ? "Could not pop stash" : "Could not apply stash", () =>
        ipc.stashApply(repo, stash.index, stash.oid, pop),
      ),

    drop: async (stash: Stash) => {
      const ok = await confirm({
        title: "Delete stash",
        message: `Delete “${stash.message}”? This cannot be undone.`,
        confirmLabel: "Delete",
        danger: true,
      });
      return ok && run("Could not delete stash", () => ipc.stashDrop(repo, stash.index, stash.oid));
    },
  };
}
