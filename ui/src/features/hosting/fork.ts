import { useQueryClient } from "@tanstack/react-query";

import type { RepoLink } from "../../bindings/RepoLink";
import { ipc } from "../../lib/ipc";
import { toast } from "../../ui/toast-store";
import { gitActions } from "../ops/actions";
import { invalidateHosting, useAccounts } from "./queries";

/**
 * Forks the repository a remote points at to the user's account, and adds
 * the fork as a remote named after them (same protocol as the original).
 */
export function useFork(repo: string, remoteNames: string[]) {
  const client = useQueryClient();
  const accounts = useAccounts().data ?? [];
  return async (link: RepoLink, remoteUrl: string | null) => {
    const account = accounts.find((a) => a.account.id === link.account)?.account;
    toast.info(`Forking ${link.path}…`);
    try {
      const fork = await ipc.hostingFork(link.account, link.path);
      const ssh = !!remoteUrl && !/^https?:\/\//.test(remoteUrl);
      const url = ssh && fork.cloneSsh ? fork.cloneSsh : fork.cloneHttps;
      let name = account?.username.replace(/[^\w.-]+/g, "-") || "fork";
      if (remoteNames.includes(name)) name = `${name}-fork`;
      const actions = gitActions(client, repo);
      if (!(await actions.addRemote(name, url))) return;
      toast.success(`Forked to ${fork.path}`, `Added as remote “${name}”.`);
      invalidateHosting(client);
      // Services create forks in the background; give it a moment.
      setTimeout(() => void actions.fetch(name), 3000);
    } catch (err) {
      toast.error("Could not fork", String(err));
    }
  };
}
