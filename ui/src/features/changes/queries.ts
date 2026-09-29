import { useQuery, useQueryClient } from "@tanstack/react-query";

import type { Hunk } from "../../bindings/Hunk";
import type { IgnoreScope } from "../../bindings/IgnoreScope";
import type { LineSelection } from "../../bindings/LineSelection";
import type { PatchTarget } from "../../bindings/PatchTarget";
import type { StatusEntry } from "../../bindings/StatusEntry";
import type { WorkingStatus } from "../../bindings/WorkingStatus";
import { ipc } from "../../lib/ipc";
import { confirm } from "../../ui/confirm-store";
import { toast } from "../../ui/toast-store";
import { invalidateWorktree, keys } from "../workspace/queries";

export function useWorkingStatus(repo: string, enabled = true) {
  return useQuery({
    queryKey: keys.status(repo),
    queryFn: () => ipc.workingStatus(repo),
    enabled,
  });
}

/** Number of distinct paths with uncommitted changes. */
export function countChanges(status: WorkingStatus | undefined): number {
  if (!status) return 0;
  const all = [...status.staged, ...status.unstaged, ...status.conflicted];
  return new Set(all.map((e) => e.path)).size;
}

/** Paths to unstage: both sides of a staged rename. */
export function unstagePaths(entries: StatusEntry[]): string[] {
  return entries.flatMap((e) => (e.oldPath ? [e.oldPath, e.path] : [e.path]));
}

export function discardMessage(entries: StatusEntry[]): string {
  const untracked = entries.filter((e) => e.status === "untracked").length;
  const what = entries.length === 1 ? entries[0]!.path : `${entries.length.toLocaleString()} files`;
  let message = `Discard all changes to ${what}? This cannot be undone.`;
  if (untracked === entries.length) {
    message = `Delete ${entries.length === 1 ? "the untracked file" : "the untracked files"} ${
      entries.length === 1 ? entries[0]!.path : `(${untracked.toLocaleString()})`
    }? This cannot be undone.`;
  } else if (untracked > 0) {
    message += `\n${untracked.toLocaleString()} untracked ${untracked === 1 ? "file is" : "files are"} deleted.`;
  }
  return message;
}

/** Actions on the working copy. Each reports failures itself and resolves to success. */
export function useWorkingActions(repo: string) {
  const client = useQueryClient();

  const run = async (title: string, action: () => Promise<unknown>): Promise<boolean> => {
    try {
      await action();
      return true;
    } catch (err) {
      toast.error(title, String(err));
      return false;
    } finally {
      void invalidateWorktree(client, repo);
    }
  };

  return {
    stage: (entries: StatusEntry[]) =>
      run("Could not stage", () =>
        ipc.stageFiles(
          repo,
          entries.map((e) => e.path),
        ),
      ),

    unstage: (entries: StatusEntry[]) =>
      run("Could not unstage", () => ipc.unstageFiles(repo, unstagePaths(entries))),

    discard: async (entries: StatusEntry[]) => {
      if (entries.length === 0) return false;
      const ok = await confirm({
        title: "Discard changes",
        message: discardMessage(entries),
        confirmLabel: "Discard",
        danger: true,
      });
      return ok && run("Could not discard changes", () => ipc.discardFiles(repo, entries));
    },

    applyLines: async (
      path: string,
      hunks: Hunk[],
      selection: LineSelection[],
      target: PatchTarget,
    ) => {
      if (target === "discard") {
        const ok = await confirm({
          title: "Discard changes",
          message: `Discard the selected changes to ${path}? This cannot be undone.`,
          confirmLabel: "Discard",
          danger: true,
        });
        if (!ok) return false;
      }
      const titles = {
        stage: "Could not stage lines",
        unstage: "Could not unstage lines",
        discard: "Could not discard lines",
      };
      return run(titles[target], () => ipc.applyLines({ repo, path, hunks, selection, target }));
    },

    ignore: (path: string, scope: IgnoreScope) =>
      run("Could not update .gitignore", async () => {
        const pattern = await ipc.addToGitignore(repo, path, scope);
        toast.success("Added to .gitignore", pattern);
      }),
  };
}
