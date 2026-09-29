import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useMemo } from "react";

import type { Outcome } from "../../bindings/Outcome";
import type { PullMode } from "../../bindings/PullMode";
import type { PushTarget } from "../../bindings/PushTarget";
import type { Refs } from "../../bindings/Refs";
import type { ResetMode } from "../../bindings/ResetMode";
import { ipc } from "../../lib/ipc";
import { confirm } from "../../ui/confirm-store";
import { toast } from "../../ui/toast-store";
import { invalidateRepo, keys } from "../workspace/queries";
import { WORKING_COPY, updateView } from "../workspace/view";
import { openDialog } from "./dialogs";
import { startTask } from "./tasks";

export type GitActions = ReturnType<typeof gitActions>;

export function useGitActions(repo: string): GitActions {
  const client = useQueryClient();
  return useMemo(() => gitActions(client, repo), [client, repo]);
}

/** A revision for messages: full object ids are abbreviated. */
export function shortRev(rev: string): string {
  return /^[0-9a-f]{40,64}$/.test(rev) ? rev.slice(0, 7) : rev;
}

/** The commit of remote branch `name` (e.g. `origin/main`), as last fetched. */
export function remoteBranchOid(refs: Refs | undefined, name: string): string | null {
  for (const remote of refs?.remotes ?? []) {
    const branch = remote.branches.find((b) => `${remote.name}/${b.name}` === name);
    if (branch) return branch.oid;
  }
  return null;
}

export const PULL_LABELS: Record<PullMode, string> = {
  default: "Pull",
  fastForwardOnly: "Pull (fast-forward only)",
  merge: "Pull (merge)",
  rebase: "Pull (rebase)",
};

/**
 * Git actions on one repository. Each reports its own failure, refreshes
 * the repository afterwards, and resolves to whether it succeeded.
 */
export function gitActions(client: QueryClient, repo: string) {
  const run = async <T>(
    failure: string,
    action: () => Promise<T>,
    task?: string,
  ): Promise<{ ok: true; value: T } | { ok: false }> => {
    const done = task ? startTask(repo, task) : undefined;
    try {
      return { ok: true, value: await action() };
    } catch (err) {
      toast.error(failure, String(err));
      return { ok: false };
    } finally {
      done?.();
      void invalidateRepo(client, repo);
    }
  };
  const simple = async (failure: string, action: () => Promise<unknown>, task?: string) =>
    (await run(failure, action, task)).ok;

  /** Stops on conflicts are not failures, but the user has work to do. */
  const outcome = async (what: string, action: () => Promise<Outcome>, task?: string) => {
    const result = await run(`Could not ${what.toLowerCase()}`, action, task);
    if (result.ok && result.value === "conflicts") {
      toast.info(
        `${what} stopped on conflicts`,
        "Resolve them in the uncommitted changes, then continue or abort.",
      );
      updateView(repo, { selected: WORKING_COPY, openFile: null });
    }
    return result.ok;
  };

  const push = async (name: string, target: PushTarget | null, upstreamName: string) => {
    const result = await run(
      "Could not push",
      () => ipc.pushBranch(repo, name, target, null),
      `Pushing ${name}`,
    );
    if (!result.ok) return false;
    if (result.value === "pushed") {
      toast.success(`Pushed ${name}`);
      return true;
    }
    // The force push may only replace what the user is shown here.
    const seen = remoteBranchOid(client.getQueryData<Refs>(keys.refs(repo)), upstreamName);
    if (!seen) {
      toast.error(
        "Push rejected",
        `${upstreamName} has commits that ${name} doesn't. Fetch, then try again.`,
      );
      return false;
    }
    const force = await confirm({
      title: "Push rejected",
      message:
        `${upstreamName} has commits that ${name} doesn't. Pull them first, or force push ` +
        `to replace ${upstreamName} (at ${shortRev(seen)}) with ${name}.\nIf ${upstreamName} ` +
        "moved on since it was last fetched, the force push fails instead of discarding " +
        "commits you haven't seen.",
      confirmLabel: "Force push",
      danger: true,
    });
    if (!force) return false;
    return simple(
      "Could not force push",
      () => ipc.pushBranch(repo, name, target, seen),
      `Force pushing ${name}`,
    );
  };

  return {
    undo: async (redo: boolean) => {
      const result = await run(redo ? "Could not redo" : "Could not undo", () =>
        ipc.undo(repo, redo),
      );
      if (result.ok) toast.success(`${redo ? "Redid" : "Undid"} “${result.value}”`);
      return result.ok;
    },

    checkout: (name: string) =>
      simple(`Could not check out ${name}`, () => ipc.checkoutBranch(repo, name)),

    /** Checks out a remote branch: its local namesake if there is one, else a new tracking branch. */
    checkoutRemote: (remoteRef: string, branch: string, localNames: string[]) =>
      localNames.includes(branch)
        ? simple(`Could not check out ${branch}`, () => ipc.checkoutBranch(repo, branch))
        : simple(`Could not check out ${branch}`, () =>
            ipc.checkoutRemoteBranch(repo, remoteRef, branch),
          ),

    checkoutDetached: (rev: string) =>
      simple(`Could not check out ${shortRev(rev)}`, () => ipc.checkoutDetached(repo, rev)),

    createBranch: (name: string, start: string, checkout: boolean) =>
      simple("Could not create branch", () => ipc.createBranch(repo, name, start, checkout)),

    renameBranch: (old: string, name: string) =>
      simple("Could not rename branch", () => ipc.renameBranch(repo, old, name)),

    deleteBranch: async (name: string) => {
      const ok = await confirm({
        title: "Delete branch",
        message: `Delete the local branch ${name}?`,
        confirmLabel: "Delete",
        danger: true,
      });
      if (!ok) return false;
      try {
        await ipc.deleteBranch(repo, name, false);
        return true;
      } catch (err) {
        if (!String(err).includes("not fully merged")) {
          toast.error("Could not delete branch", String(err));
          return false;
        }
      } finally {
        void invalidateRepo(client, repo);
      }
      const force = await confirm({
        title: "Branch not merged",
        message: `${name} has commits that no other branch contains. Delete it anyway?`,
        confirmLabel: "Delete anyway",
        danger: true,
      });
      return force && simple("Could not delete branch", () => ipc.deleteBranch(repo, name, true));
    },

    moveBranch: (name: string, rev: string) =>
      simple(`Could not move ${name}`, () => ipc.moveBranch(repo, name, rev)),

    setUpstream: (name: string, upstream: string | null) =>
      simple("Could not set upstream", () => ipc.setUpstream(repo, name, upstream)),

    createTag: (name: string, target: string, message: string | null) =>
      simple("Could not create tag", () => ipc.createTag(repo, name, target, message)),

    deleteTag: async (name: string) => {
      const ok = await confirm({
        title: "Delete tag",
        message: `Delete the local tag ${name}?`,
        confirmLabel: "Delete",
        danger: true,
      });
      return ok && simple("Could not delete tag", () => ipc.deleteTag(repo, name));
    },

    pushTag: async (remote: string, name: string) => {
      const result = await run(
        "Could not push tag",
        () => ipc.pushTag(repo, remote, name),
        `Pushing ${name}`,
      );
      if (!result.ok) return false;
      if (result.value === "rejected") {
        toast.error("Could not push tag", `${remote} already has a different ${name}.`);
        return false;
      }
      toast.success(`Pushed ${name} to ${remote}`);
      return true;
    },

    deleteRemoteRef: async (remote: string, fullRef: string, label: string) => {
      const ok = await confirm({
        title: "Delete on remote",
        message: `Delete ${label} on ${remote}? Anyone using it there loses it too.`,
        confirmLabel: "Delete",
        danger: true,
      });
      return (
        ok &&
        simple(
          `Could not delete ${label}`,
          () => ipc.deleteRemoteRef(repo, remote, fullRef),
          `Deleting ${label}`,
        )
      );
    },

    addRemote: (name: string, url: string) =>
      simple("Could not add remote", () => ipc.addRemote(repo, name, url)),

    editRemote: (name: string, newName: string, url: string, pushUrl: string | null) =>
      simple("Could not update remote", () =>
        ipc.editRemote({ repo, name, newName, url, pushUrl }),
      ),

    removeRemote: async (name: string) => {
      const ok = await confirm({
        title: "Remove remote",
        message: `Remove ${name} and its remote branches from this repository? Nothing changes on ${name} itself.`,
        confirmLabel: "Remove",
        danger: true,
      });
      return ok && simple("Could not remove remote", () => ipc.removeRemote(repo, name));
    },

    fetch: (remote: string | null) =>
      simple(
        remote ? `Could not fetch ${remote}` : "Could not fetch",
        () => ipc.fetch(repo, remote, false),
        remote ? `Fetching ${remote}` : "Fetching",
      ),

    pull: (mode: PullMode) => outcome(PULL_LABELS[mode], () => ipc.pull(repo, mode), "Pulling"),

    /** Pushes to the upstream, or asks where to if there is none. */
    push: (name: string, upstream: string | null) => {
      if (!upstream) {
        openDialog({ kind: "pushNew", repo, branch: name });
        return Promise.resolve(false);
      }
      return push(name, null, upstream);
    },

    pushTo: (name: string, target: PushTarget) =>
      push(name, target, `${target.remote}/${target.branch}`),

    merge: (rev: string, into: string) =>
      outcome(`Merge ${shortRev(rev)} into ${into}`, () => ipc.merge(repo, rev, false)),

    rebase: (onto: string, branch: string) =>
      outcome(`Rebase ${branch} onto ${shortRev(onto)}`, () => ipc.rebase(repo, onto)),

    cherryPick: (oid: string) =>
      outcome(`Cherry-pick ${shortRev(oid)}`, () => ipc.cherryPick(repo, oid)),

    revert: (oid: string) => outcome(`Revert ${shortRev(oid)}`, () => ipc.revert(repo, oid)),

    reset: async (rev: string, mode: ResetMode, branch: string) => {
      if (mode === "hard") {
        const ok = await confirm({
          title: "Hard reset",
          message:
            `Move ${branch} to ${shortRev(rev)} and discard every uncommitted change? ` +
            "Undo brings the commits back, not the discarded changes.",
          confirmLabel: "Reset",
          danger: true,
        });
        if (!ok) return false;
      }
      return simple("Could not reset", () => ipc.reset(repo, rev, mode));
    },

    continueOperation: () => outcome("Continue", () => ipc.resolveOperation(repo, "continue")),
    skipOperation: () => outcome("Skip", () => ipc.resolveOperation(repo, "skip")),
    abortOperation: async () => {
      const ok = await confirm({
        title: "Abort",
        message: "Abort and go back to where it started? Conflict resolutions are lost.",
        confirmLabel: "Abort",
        danger: true,
      });
      return ok && simple("Could not abort", () => ipc.resolveOperation(repo, "abort"));
    },
  };
}
