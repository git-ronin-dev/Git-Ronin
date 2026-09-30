import { useQueryClient } from "@tanstack/react-query";
import { CircleDot, GitPullRequest, LogIn, Plus, RefreshCw } from "lucide-react";

import { openSettings } from "../../app/overlays";
import type { Issue } from "../../bindings/Issue";
import type { PullRequest } from "../../bindings/PullRequest";
import type { RepoLink } from "../../bindings/RepoLink";
import { copyText } from "../../lib/clipboard";
import { openUrl } from "../../lib/open";
import { ContextMenu } from "../../ui/ContextMenu";
import { Section } from "../../ui/Section";
import { Tooltip } from "../../ui/Tooltip";
import { openDialog } from "../ops/dialogs";
import { Leaf } from "../refs/SidebarTree";
import { useRefs } from "../workspace/queries";
import { updateView, useRepoView } from "../workspace/view";
import { issueBranchName, useStartIssue } from "./issues";
import { PROVIDERS, prNoun, prRef } from "./providers";
import { hostingKeys, pickLink, useIssues, usePullRequests, useRepoLinks } from "./queries";

const icon = "size-3.5";

/** Public services a remote may be on, to suggest signing in. */
const PUBLIC_HOSTS: [string, string][] = [
  ["github.com", PROVIDERS.github.label],
  ["gitlab.com", PROVIDERS.gitlab.label],
  ["bitbucket.org", PROVIDERS.bitbucket.label],
  ["dev.azure.com", PROVIDERS.azureDevops.label],
  ["visualstudio.com", PROVIDERS.azureDevops.label],
];

function HeaderButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip content={label}>
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        className="rounded-sm text-fg-faint opacity-0 group-hover:opacity-100 hover:text-fg focus-visible:opacity-100"
      >
        {children}
      </button>
    </Tooltip>
  );
}

/** Pull requests and issues of the repository's remotes, when an account covers them. */
export function HostingSections({ repo }: { repo: string }) {
  const links = useRepoLinks(repo).data;
  const remotes = useRefs(repo).data?.remotes ?? [];
  if (!links) return null;
  if (links.length === 0) {
    const service = PUBLIC_HOSTS.find(([host]) => remotes.some((r) => r.url?.includes(host)));
    if (!service) return null;
    return (
      <Section title="Pull requests" icon={<GitPullRequest className={icon} />}>
        <Leaf depth={1} onClick={() => openSettings("accounts")}>
          <LogIn className={icon} />
          <span className="truncate text-fg-muted">Sign in to {service[1]}…</span>
        </Leaf>
      </Section>
    );
  }
  const prs = pickLink(links, "pullRequests");
  const issues = pickLink(links, "issues");
  return (
    <>
      {prs && <PullRequestSection repo={repo} link={prs} />}
      {issues && <IssueSection repo={repo} link={issues} />}
    </>
  );
}

function PullRequestSection({ repo, link }: { repo: string; link: RepoLink }) {
  const client = useQueryClient();
  const prs = usePullRequests(link);
  const open = useRepoView(repo).pullRequest;
  const head = useRefs(repo).data?.local.find((b) => b.isHead);
  const noun = prNoun(link.kind);
  return (
    <Section
      title={link.kind === "gitlab" ? "Merge requests" : "Pull requests"}
      icon={<GitPullRequest className={icon} />}
      count={prs.data?.length}
      action={
        <span className="flex gap-1">
          <HeaderButton
            label="Refresh"
            onClick={() =>
              void client.invalidateQueries({
                queryKey: hostingKeys.pullRequests(link.account, link.path),
              })
            }
          >
            <RefreshCw className={icon} />
          </HeaderButton>
          <HeaderButton
            label={`Create ${noun}`}
            onClick={() =>
              openDialog({ kind: "createPullRequest", repo, branch: head?.name ?? null })
            }
          >
            <Plus className={icon} />
          </HeaderButton>
        </span>
      }
    >
      {prs.isError && <p className="px-3 text-xs break-words text-danger">{String(prs.error)}</p>}
      {prs.data?.length === 0 && (
        <p className="px-3 py-1 text-xs text-fg-faint">No open {noun}s.</p>
      )}
      {prs.data?.map((pr) => (
        <PullRequestLeaf
          key={pr.number}
          repo={repo}
          link={link}
          pr={pr}
          active={open?.number === pr.number && open.path === link.path}
        />
      ))}
    </Section>
  );
}

function PullRequestLeaf({
  repo,
  link,
  pr,
  active,
}: {
  repo: string;
  link: RepoLink;
  pr: PullRequest;
  active: boolean;
}) {
  const show = () =>
    updateView(repo, {
      pullRequest: {
        account: link.account,
        path: link.path,
        remote: link.remote,
        number: pr.number,
      },
      openFile: null,
    });
  return (
    <ContextMenu
      items={[
        { label: "Open", onSelect: show },
        { label: "Open in browser", onSelect: () => void openUrl(pr.webUrl) },
        "separator",
        { label: "Copy link", onSelect: () => void copyText(pr.webUrl, "Copied link") },
      ]}
    >
      <Leaf
        depth={1}
        title={`${pr.title}\n${pr.sourceBranch} → ${pr.targetBranch} · ${pr.author.name}`}
        onClick={show}
        highlighted={active}
      >
        <span className="shrink-0 text-xs text-fg-faint">{prRef(link.kind, pr.number)}</span>
        <span className="truncate">{pr.title}</span>
        {pr.draft && <span className="shrink-0 text-[10px] text-fg-faint">draft</span>}
      </Leaf>
    </ContextMenu>
  );
}

function IssueSection({ repo, link }: { repo: string; link: RepoLink }) {
  const client = useQueryClient();
  const issues = useIssues(link);
  return (
    <Section
      title="Issues"
      icon={<CircleDot className={icon} />}
      count={issues.data?.length}
      defaultOpen={false}
      action={
        <span className="flex gap-1">
          <HeaderButton
            label="Refresh"
            onClick={() =>
              void client.invalidateQueries({
                queryKey: hostingKeys.issues(link.account, link.path),
              })
            }
          >
            <RefreshCw className={icon} />
          </HeaderButton>
          <HeaderButton label="New issue" onClick={() => openDialog({ kind: "createIssue", repo })}>
            <Plus className={icon} />
          </HeaderButton>
        </span>
      }
    >
      {issues.isError && (
        <p className="px-3 text-xs break-words text-danger">{String(issues.error)}</p>
      )}
      {issues.data?.length === 0 && (
        <p className="px-3 py-1 text-xs text-fg-faint">No open issues.</p>
      )}
      {issues.data?.map((issue) => (
        <IssueLeaf key={issue.key} repo={repo} issue={issue} />
      ))}
    </Section>
  );
}

function IssueLeaf({ repo, issue }: { repo: string; issue: Issue }) {
  const start = useStartIssue(repo);
  return (
    <ContextMenu
      items={[
        {
          label: `Start work on it (branch ${issueBranchName(issue)})`,
          onSelect: () => void start(issue),
        },
        { label: "Open in browser", onSelect: () => void openUrl(issue.webUrl) },
        "separator",
        { label: "Copy link", onSelect: () => void copyText(issue.webUrl, "Copied link") },
      ]}
    >
      <Leaf
        depth={1}
        title={`${issue.title}${issue.labels.length ? `\n${issue.labels.join(", ")}` : ""}`}
        onClick={() => void openUrl(issue.webUrl)}
      >
        <span className="shrink-0 text-xs text-fg-faint">{issue.key}</span>
        <span className="truncate">{issue.title}</span>
      </Leaf>
    </ContextMenu>
  );
}
