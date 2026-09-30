import { useQuery, type QueryClient } from "@tanstack/react-query";

import type { AccountView } from "../../bindings/AccountView";
import type { Config } from "../../bindings/Config";
import type { RepoLink } from "../../bindings/RepoLink";
import { ipc } from "../../lib/ipc";
import { activeProfileId } from "../commands/commands";
import { keys, useConfig } from "../workspace/queries";

/** Network data is kept a while and refreshed on demand. */
const STALE = 60_000;

export const hostingKeys = {
  all: ["hosting"] as const,
  accounts: ["hosting", "accounts"] as const,
  launchpad: ["hosting", "launchpad"] as const,
  repos: (account: string) => ["hosting", account, "repos"] as const,
  pullRequests: (account: string, path: string) =>
    ["hosting", account, path, "pullRequests"] as const,
  pullRequest: (account: string, path: string, number: number) =>
    ["hosting", account, path, "pullRequest", number] as const,
  issues: (account: string, path: string) => ["hosting", account, path, "issues"] as const,
  ci: (account: string, path: string, sha: string) =>
    ["hosting", account, path, "ci", sha] as const,
};

/** The profile an account belongs to: its own, or the first if that one is gone. */
export function accountProfile(config: Config | undefined, profile: string): string | undefined {
  const profiles = config?.portable.profiles ?? [];
  return (profiles.find((p) => p.id === profile) ?? profiles[0])?.id;
}

export function useAccounts() {
  return useQuery({ queryKey: hostingKeys.accounts, queryFn: ipc.hostingAccounts });
}

/** Accounts of the profile in use. */
export function useProfileAccounts(): AccountView[] {
  const config = useConfig().data;
  const accounts = useAccounts().data ?? [];
  const active = activeProfileId(config);
  return accounts.filter((a) => accountProfile(config, a.account.profile) === active);
}

/** The query for a repository's links; shared by every user so they share the cache. */
export function linksQuery(repo: string, accounts: AccountView[] | undefined) {
  return {
    queryKey: [...keys.hostingLinks(repo), accounts?.map((a) => a.account.id).join(",")],
    queryFn: () => ipc.hostingLinks(repo),
    enabled: accounts !== undefined,
    staleTime: STALE,
  };
}

/** Remotes of `repo` on the profile's accounts; refreshed with the refs. */
export function useRepoLinks(repo: string) {
  return useQuery(linksQuery(repo, useAccounts().data));
}

/** The first link that can do `what`. */
export function pickLink(
  links: RepoLink[] | undefined,
  what: "pullRequests" | "issues" | "ci",
): RepoLink | undefined {
  return links?.find((l) => l.capabilities[what]);
}

export function usePullRequests(link: RepoLink | undefined) {
  return useQuery({
    queryKey: hostingKeys.pullRequests(link?.account ?? "", link?.path ?? ""),
    queryFn: () => ipc.hostingPullRequests(link!.account, link!.path),
    enabled: !!link,
    staleTime: STALE,
    refetchInterval: 5 * 60_000,
  });
}

export function usePullRequest(link: RepoLink | undefined, number: number) {
  return useQuery({
    queryKey: hostingKeys.pullRequest(link?.account ?? "", link?.path ?? "", number),
    queryFn: () => ipc.hostingPullRequest(link!.account, link!.path, number),
    enabled: !!link,
    staleTime: 15_000,
  });
}

export function useIssues(link: RepoLink | undefined) {
  return useQuery({
    queryKey: hostingKeys.issues(link?.account ?? "", link?.path ?? ""),
    queryFn: () => ipc.hostingIssues(link!.account, link!.path),
    enabled: !!link,
    staleTime: STALE,
    refetchInterval: 5 * 60_000,
  });
}

/** Checks on a commit; polled while some are still running. */
export function useCiStatus(link: RepoLink | undefined, sha: string | null) {
  return useQuery({
    queryKey: hostingKeys.ci(link?.account ?? "", link?.path ?? "", sha ?? ""),
    queryFn: () => ipc.hostingCiStatus(link!.account, link!.path, sha!),
    enabled: !!link && !!sha,
    staleTime: 30_000,
    refetchInterval: (q) => (q.state.data?.state === "pending" ? 30_000 : false),
  });
}

export function useLaunchpad(enabled: boolean) {
  return useQuery({
    queryKey: hostingKeys.launchpad,
    queryFn: ipc.hostingLaunchpad,
    enabled,
    staleTime: STALE,
  });
}

export function useHostedRepos(account: string | null) {
  return useQuery({
    queryKey: hostingKeys.repos(account ?? ""),
    queryFn: () => ipc.hostingRepos(account!),
    enabled: !!account,
    staleTime: 5 * 60_000,
  });
}

/** After signing in or out: everything hosting-related may have changed. */
export function invalidateHosting(client: QueryClient) {
  void client.invalidateQueries({ queryKey: hostingKeys.all });
  void client.invalidateQueries({ predicate: (q) => q.queryKey[1] === "hostingLinks" });
}
