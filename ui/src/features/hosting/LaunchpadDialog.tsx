import { useQueries, useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useState } from "react";

import { openSettings, useOverlays } from "../../app/overlays";
import type { Involvement } from "../../bindings/Involvement";
import type { LaunchpadIssue } from "../../bindings/LaunchpadIssue";
import type { LaunchpadPr } from "../../bindings/LaunchpadPr";
import type { RepoLink } from "../../bindings/RepoLink";
import { copyText } from "../../lib/clipboard";
import { openUrl } from "../../lib/open";
import { relativeTime } from "../../lib/time";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { ContextMenu } from "../../ui/ContextMenu";
import { Dialog } from "../../ui/Dialog";
import { EmptyNote } from "../../ui/EmptyNote";
import { CircleDot, GitPullRequest, RefreshCw } from "../../ui/icons";
import { useUiPrefs } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { updateView } from "../workspace/view";
import { issueBranchName, useStartIssue } from "./issues";
import { PROVIDERS, prRef } from "./providers";
import { hostingKeys, linksQuery, useAccounts, useLaunchpad, useProfileAccounts } from "./queries";

type Filter = "all" | Involvement;

const FILTERS: [Filter, string][] = [
  ["all", "All"],
  ["reviewer", "Review requested"],
  ["author", "Mine"],
  ["assignee", "Assigned"],
];

const ROLE: Record<Involvement, string> = {
  author: "yours",
  reviewer: "review requested",
  assignee: "assigned",
};

/** Open tabs whose remotes are on the profile's accounts, with their links. */
function useOpenLinks(): { repo: string; link: RepoLink }[] {
  const tabs = useWorkspace((s) => s.tabs);
  const accounts = useAccounts().data;
  const results = useQueries({ queries: tabs.map((t) => linksQuery(t.path, accounts)) });
  return tabs.flatMap((t, i) => (results[i]?.data ?? []).map((link) => ({ repo: t.path, link })));
}

export function LaunchpadDialog() {
  const open = useOverlays((s) => s.launchpad);
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && useOverlays.setState({ launchpad: false })}
      title="Launchpad"
      size="large"
    >
      {open && <Launchpad />}
    </Dialog>
  );
}

function Launchpad() {
  const client = useQueryClient();
  const accounts = useProfileAccounts();
  const launchpad = useLaunchpad(accounts.length > 0);
  const [tab, setTab] = useState<"prs" | "issues">("prs");
  const [filter, setFilter] = useState<Filter>("all");
  const openLinks = useOpenLinks();
  const accountName = (id: string) => {
    const a = accounts.find((v) => v.account.id === id)?.account;
    return a ? `${a.username} on ${PROVIDERS[a.kind].label}` : id;
  };

  if (accounts.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-fg-muted">
        <p>Sign in to a hosting service to see your pull requests and issues here.</p>
        <Button variant="primary" onClick={() => openSettings("accounts")}>
          Add an account…
        </Button>
      </div>
    );
  }

  const data = launchpad.data;
  const prs = (data?.pullRequests ?? []).filter(
    (p) => filter === "all" || p.involvement.includes(filter),
  );
  const issues = data?.issues ?? [];
  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex items-center gap-2">
        <div role="tablist" className="flex gap-1">
          <TabButton selected={tab === "prs"} onClick={() => setTab("prs")}>
            <GitPullRequest className="size-3.5" />
            Pull requests {data && `(${data.pullRequests.length})`}
          </TabButton>
          <TabButton selected={tab === "issues"} onClick={() => setTab("issues")}>
            <CircleDot className="size-3.5" />
            Issues {data && `(${issues.length})`}
          </TabButton>
        </div>
        {tab === "prs" && (
          <div className="ml-4 flex gap-1">
            {FILTERS.map(([id, label]) => (
              <button
                key={id}
                type="button"
                aria-pressed={filter === id}
                onClick={() => setFilter(id)}
                className={clsx(
                  "h-6 rounded-full border px-2 text-xs",
                  filter === id
                    ? "border-accent bg-accent/15 text-fg"
                    : "border-line text-fg-muted hover:bg-hover",
                )}
              >
                {label}
              </button>
            ))}
          </div>
        )}
        <Button
          variant="ghost"
          className="ml-auto"
          onClick={() => void client.invalidateQueries({ queryKey: hostingKeys.launchpad })}
        >
          <RefreshCw className={clsx("size-3.5", launchpad.isFetching && "animate-spin")} />
          Refresh
        </Button>
      </div>
      {launchpad.isError && <p className="text-danger">{String(launchpad.error)}</p>}
      {data?.errors.map((e) => (
        <p key={e.account} className="text-xs break-words text-danger">
          {accountName(e.account)}: {e.message}
        </p>
      ))}
      <div className="min-h-0 flex-1 overflow-y-auto rounded-md border border-line">
        {!data ? (
          <p className="p-6 text-center text-fg-faint">Loading…</p>
        ) : tab === "prs" ? (
          prs.length === 0 ? (
            <EmptyNote title="Nothing waiting for you">
              Pull requests you wrote, review or are assigned to show up here.
            </EmptyNote>
          ) : (
            <ul>
              {prs.map((p) => (
                <PrRow key={`${p.account}:${p.pr.webUrl}`} item={p} openLinks={openLinks} />
              ))}
            </ul>
          )
        ) : issues.length === 0 ? (
          <EmptyNote title="No open issues assigned to you" />
        ) : (
          <ul>
            {issues.map((i) => (
              <IssueRow key={`${i.account}:${i.issue.webUrl}`} item={i} openLinks={openLinks} />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function TabButton({
  selected,
  onClick,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={selected}
      onClick={onClick}
      className={clsx(
        "flex h-7 items-center gap-1.5 rounded-md px-2",
        selected ? "bg-hover text-fg" : "text-fg-muted hover:bg-hover hover:text-fg",
      )}
    >
      {children}
    </button>
  );
}

function PrRow({
  item,
  openLinks,
}: {
  item: LaunchpadPr;
  openLinks: { repo: string; link: RepoLink }[];
}) {
  const { pr } = item;
  const showAvatars = useUiPrefs()?.showAvatars ?? false;
  const local = openLinks.find((o) => o.link.account === item.account && o.link.path === pr.repo);
  const kind = useAccounts().data?.find((a) => a.account.id === item.account)?.account.kind;
  const show = () => {
    if (!local) {
      void openUrl(pr.webUrl);
      return;
    }
    useWorkspace.getState().activate(local.repo);
    updateView(local.repo, {
      pullRequest: {
        account: item.account,
        path: pr.repo,
        remote: local.link.remote,
        number: pr.number,
      },
      openFile: null,
    });
    useOverlays.setState({ launchpad: false });
  };
  return (
    <ContextMenu
      items={[
        { label: local ? "Open in its tab" : "Open in browser", onSelect: show },
        ...(local ? [{ label: "Open in browser", onSelect: () => void openUrl(pr.webUrl) }] : []),
        "separator",
        { label: "Copy link", onSelect: () => void copyText(pr.webUrl, "Copied link") },
      ]}
    >
      <li>
        <button
          type="button"
          onClick={show}
          title={local ? `Open in ${local.repo}` : "Open in browser"}
          className="flex w-full items-center gap-3 border-b border-line px-3 py-2 text-left hover:bg-hover"
        >
          <Avatar
            name={pr.author.name}
            email={pr.author.username}
            size={24}
            remote={showAvatars}
            url={pr.author.avatarUrl}
          />
          <span className="min-w-0 flex-1">
            <span className="flex items-baseline gap-2">
              <span className="truncate font-medium">{pr.title}</span>
              {pr.draft && <span className="shrink-0 text-xs text-fg-faint">draft</span>}
            </span>
            <span className="block truncate text-xs text-fg-muted">
              {pr.repo} {prRef(kind, pr.number)} · {pr.author.name} · updated{" "}
              {relativeTime(Number(pr.updated))}
            </span>
          </span>
          <span className="flex shrink-0 gap-1">
            {item.involvement.map((r) => (
              <span
                key={r}
                className="rounded-full bg-raised px-2 py-0.5 text-[10px] text-fg-muted"
              >
                {ROLE[r]}
              </span>
            ))}
          </span>
        </button>
      </li>
    </ContextMenu>
  );
}

function IssueRow({
  item,
  openLinks,
}: {
  item: LaunchpadIssue;
  openLinks: { repo: string; link: RepoLink }[];
}) {
  const { issue } = item;
  const active = useWorkspace((s) => s.active);
  // Its repository's tab, else (Jira issues have no repository) the active one.
  const local =
    openLinks.find((o) => o.link.account === item.account && o.link.path === issue.repo)?.repo ??
    (issue.key.startsWith("#") ? null : active);
  const start = useStartIssue(local ?? "");
  return (
    <ContextMenu
      items={[
        { label: "Open in browser", onSelect: () => void openUrl(issue.webUrl) },
        ...(local
          ? [
              {
                label: `Start work in ${local.split(/[\\/]/).pop()} (branch ${issueBranchName(issue)})`,
                onSelect: () => {
                  useWorkspace.getState().activate(local);
                  void start(issue);
                  useOverlays.setState({ launchpad: false });
                },
              },
            ]
          : []),
        "separator",
        { label: "Copy link", onSelect: () => void copyText(issue.webUrl, "Copied link") },
      ]}
    >
      <li>
        <button
          type="button"
          onClick={() => void openUrl(issue.webUrl)}
          className="flex w-full items-center gap-3 border-b border-line px-3 py-2 text-left hover:bg-hover"
        >
          <CircleDot className="size-4 shrink-0 text-success" />
          <span className="min-w-0 flex-1">
            <span className="block truncate font-medium">{issue.title}</span>
            <span className="block truncate text-xs text-fg-muted">
              {issue.repo} {issue.key} · updated {relativeTime(Number(issue.updated))}
            </span>
          </span>
          <span className="flex shrink-0 gap-1">
            {issue.labels.slice(0, 3).map((l) => (
              <span
                key={l}
                className="rounded-full bg-raised px-2 py-0.5 text-[10px] text-fg-muted"
              >
                {l}
              </span>
            ))}
          </span>
        </button>
      </li>
    </ContextMenu>
  );
}
