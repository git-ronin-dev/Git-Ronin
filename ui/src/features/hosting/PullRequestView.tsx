import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useCallback, useState } from "react";

import type { MergeMethod } from "../../bindings/MergeMethod";
import type { PullRequestDetail } from "../../bindings/PullRequestDetail";
import type { RepoLink } from "../../bindings/RepoLink";
import type { ReviewState } from "../../bindings/ReviewState";
import { ipc } from "../../lib/ipc";
import { openUrl } from "../../lib/open";
import { formatDate, relativeTime } from "../../lib/time";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { confirm } from "../../ui/confirm-store";
import { DropdownMenu } from "../../ui/DropdownMenu";
import { ArrowLeft, ExternalLink, GitPullRequest, RefreshCw } from "../../ui/icons";
import { toast } from "../../ui/toast-store";
import { FileList } from "../commit/FileList";
import { useEscape } from "../commit/useEscape";
import { useGitActions } from "../ops/actions";
import { startTask, useTask } from "../ops/tasks";
import { useRefs, useUiPrefs } from "../workspace/queries";
import { updateView, useRepoView, type PrRef } from "../workspace/view";
import { Checks } from "./Checks";
import { prNoun, prRef } from "./providers";
import { hostingKeys, useAccounts, useCiStatus, usePullRequest, useRepoLinks } from "./queries";

const METHODS: Record<MergeMethod, string> = {
  merge: "Create a merge commit",
  squash: "Squash and merge",
  rebase: "Rebase and merge",
};

const REVIEW: Record<ReviewState, [string, string]> = {
  approved: ["approved", "text-success"],
  changesRequested: ["requested changes", "text-danger"],
  commented: ["commented", "text-fg-muted"],
  pending: ["review requested", "text-fg-faint"],
};

/** The link a pull request was opened through. */
function useLink(repo: string, pr: PrRef): RepoLink | undefined {
  const links = useRepoLinks(repo).data;
  return links?.find((l) => l.account === pr.account && l.path === pr.path);
}

/** Replaces the graph while a pull request is open. */
export function PullRequestView({ repo, pr }: { repo: string; pr: PrRef }) {
  const link = useLink(repo, pr);
  const detail = usePullRequest(link, pr.number);
  const fileOpen = useRepoView(repo).openFile !== null;
  const [tab, setTab] = useState<"conversation" | "files">("conversation");
  const close = useCallback(() => updateView(repo, { pullRequest: null }), [repo]);
  const noop = useCallback(() => {}, []);
  useEscape(fileOpen ? noop : close);

  const noun = prNoun(link?.kind);
  const d = detail.data;
  return (
    <div className="flex h-full flex-col">
      <header className="flex h-10 shrink-0 items-center gap-2 border-b border-line px-3">
        <Button variant="ghost" className="h-7 px-2" aria-label="Back to the graph" onClick={close}>
          <ArrowLeft className="size-4" />
        </Button>
        <GitPullRequest className="size-4 text-fg-muted" />
        <span className="truncate font-semibold">
          {d
            ? d.pr.title
            : `${noun[0]!.toUpperCase()}${noun.slice(1)} ${prRef(link?.kind, pr.number)}`}
        </span>
        <span className="shrink-0 text-fg-faint">{prRef(link?.kind, pr.number)}</span>
        <span className="ml-auto" />
        <Button
          variant="ghost"
          className="h-7 px-2"
          aria-label="Refresh"
          onClick={() => void detail.refetch()}
        >
          <RefreshCw className={clsx("size-3.5", detail.isFetching && "animate-spin")} />
        </Button>
        {d && (
          <Button variant="ghost" className="h-7" onClick={() => void openUrl(d.pr.webUrl)}>
            <ExternalLink className="size-3.5" />
            Open in browser
          </Button>
        )}
      </header>
      {detail.isError ? (
        <p className="p-8 text-center text-danger select-text">{String(detail.error)}</p>
      ) : !d || !link ? (
        <p className="p-8 text-center text-fg-faint">Loading…</p>
      ) : (
        <>
          <Summary repo={repo} link={link} pr={pr} detail={d} />
          <div role="tablist" className="flex shrink-0 gap-1 border-b border-line px-3">
            {(["conversation", "files"] as const).map((t) => (
              <button
                key={t}
                type="button"
                role="tab"
                aria-selected={tab === t}
                onClick={() => setTab(t)}
                className={clsx(
                  "-mb-px h-8 border-b-2 px-2",
                  tab === t
                    ? "border-accent text-fg"
                    : "border-transparent text-fg-muted hover:text-fg",
                )}
              >
                {t === "files" ? "Files changed" : "Conversation"}
              </button>
            ))}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {tab === "conversation" ? (
              <Conversation link={link} pr={pr} detail={d} />
            ) : (
              <Files repo={repo} pr={pr} detail={d} />
            )}
          </div>
        </>
      )}
    </div>
  );
}

function StateBadge({ detail }: { detail: PullRequestDetail }) {
  const { state, draft } = detail.pr;
  const [label, className] =
    state === "merged"
      ? ["Merged", "bg-accent/20 text-accent"]
      : state === "closed"
        ? ["Closed", "bg-danger/20 text-danger"]
        : draft
          ? ["Draft", "bg-raised text-fg-muted"]
          : ["Open", "bg-success/20 text-success"];
  return (
    <span className={clsx("rounded-full px-2 py-0.5 text-xs font-medium", className)}>{label}</span>
  );
}

function Summary({
  repo,
  link,
  pr,
  detail,
}: {
  repo: string;
  link: RepoLink;
  pr: PrRef;
  detail: PullRequestDetail;
}) {
  const client = useQueryClient();
  const actions = useGitActions(repo);
  const refs = useRefs(repo).data;
  const showAvatars = useUiPrefs()?.showAvatars ?? false;
  const task = useTask(repo);
  const noun = prNoun(link.kind);
  const open = detail.pr.state === "open";
  const me = useAccounts().data?.find((a) => a.account.id === pr.account)?.account.username;
  const approved = detail.reviews.some((r) => r.author.username === me && r.state === "approved");

  const refresh = () => {
    void client.invalidateQueries({
      queryKey: hostingKeys.pullRequest(pr.account, pr.path, pr.number),
    });
    void client.invalidateQueries({ queryKey: hostingKeys.pullRequests(pr.account, pr.path) });
  };
  const approve = useMutation({
    mutationFn: () => ipc.hostingApprove(pr.account, pr.path, pr.number),
    onSuccess: () => {
      toast.success(`Approved ${prRef(link.kind, pr.number)}`);
      refresh();
    },
    onError: (err) => toast.error("Could not approve", String(err)),
  });
  const merge = async (method: MergeMethod) => {
    const ok = await confirm({
      title: `Merge ${noun}`,
      message: `${METHODS[method]}: ${detail.pr.sourceBranch} into ${detail.pr.targetBranch}?`,
      confirmLabel: "Merge",
    });
    if (!ok) return;
    try {
      await ipc.hostingMerge(pr.account, pr.path, pr.number, method);
      toast.success(`Merged ${prRef(link.kind, pr.number)}`);
      refresh();
      void actions.fetch(pr.remote);
    } catch (err) {
      toast.error("Could not merge", String(err));
    }
  };

  const checkout = async () => {
    const source = detail.pr.sourceBranch;
    const remote = refs?.remotes.find((r) => r.name === pr.remote);
    const tracking = remote?.branches.find((b) => b.name === source);
    const localNames = refs?.local.map((b) => b.name) ?? [];
    // The branch is on our remote: track it, like any remote branch.
    if (tracking && !detail.pr.sourceRepo) {
      await actions.checkoutRemote(tracking.fullName, source, localNames);
      return;
    }
    const done = startTask(repo, `Fetching ${prRef(link.kind, pr.number)}`);
    try {
      await ipc.prFetch({
        repo,
        remote: pr.remote,
        source: detail.source,
        head: detail.headSha,
        base: null,
        targetBranch: detail.pr.targetBranch,
      });
    } catch (err) {
      toast.error("Could not fetch", String(err));
      return;
    } finally {
      done();
    }
    let name = localNames.includes(source) ? `pr-${pr.number}-${source}` : source;
    if (localNames.includes(name)) name = `pr-${pr.number}`;
    if (detail.headSha) await actions.createBranch(name, detail.headSha, true);
  };

  return (
    <div className="shrink-0 space-y-3 border-b border-line p-4">
      <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
        <StateBadge detail={detail} />
        <Avatar
          name={detail.pr.author.name}
          email={detail.pr.author.username}
          size={18}
          remote={showAvatars}
          url={detail.pr.author.avatarUrl}
        />
        <span className="text-fg">{detail.pr.author.name}</span>
        <span>
          {detail.pr.state === "merged"
            ? "merged"
            : detail.pr.state === "closed"
              ? "wanted to merge"
              : "wants to merge"}
        </span>
        <code className="rounded bg-raised px-1 text-fg select-text">
          {detail.pr.sourceRepo ? `${detail.pr.sourceRepo}:` : ""}
          {detail.pr.sourceBranch}
        </code>
        <span>into</span>
        <code className="rounded bg-raised px-1 text-fg select-text">{detail.pr.targetBranch}</code>
        <span title={formatDate(Number(detail.pr.updated))}>
          · updated {relativeTime(Number(detail.pr.updated))}
        </span>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button disabled={!!task} onClick={() => void checkout()}>
          Check out
        </Button>
        {link.capabilities.approve && open && !approved && (
          <Button disabled={approve.isPending} onClick={() => approve.mutate()}>
            Approve
          </Button>
        )}
        {approved && <span className="text-xs text-success">You approved it.</span>}
        {open && (
          <DropdownMenu
            items={detail.mergeMethods.map((m) => ({
              label: METHODS[m],
              onSelect: () => void merge(m),
            }))}
          >
            <Button variant="primary" disabled={detail.mergeable === false || detail.pr.draft}>
              Merge…
            </Button>
          </DropdownMenu>
        )}
        {open && detail.pr.draft && (
          <span className="text-xs text-fg-muted">
            A draft can’t be merged; mark it ready for review on the website first.
          </span>
        )}
        {open && !detail.pr.draft && detail.mergeable === false && (
          <span className="text-xs text-warning">
            Can’t be merged yet: conflicts, failing requirements or missing approvals.
          </span>
        )}
      </div>
    </div>
  );
}

function Conversation({
  link,
  pr,
  detail,
}: {
  link: RepoLink;
  pr: PrRef;
  detail: PullRequestDetail;
}) {
  const client = useQueryClient();
  const showAvatars = useUiPrefs()?.showAvatars ?? false;
  const ci = useCiStatus(link.capabilities.ci ? link : undefined, detail.headSha);
  const [draft, setDraft] = useState("");
  const comment = useMutation({
    mutationFn: (body: string) => ipc.hostingComment(pr.account, pr.path, pr.number, body),
    onSuccess: () => {
      setDraft("");
      void client.invalidateQueries({
        queryKey: hostingKeys.pullRequest(pr.account, pr.path, pr.number),
      });
    },
    onError: (err) => toast.error("Could not comment", String(err)),
  });

  return (
    <div className="mx-auto max-w-3xl space-y-5 p-4">
      <section className="rounded-md border border-line p-3 select-text">
        {detail.body.trim() ? (
          <p className="break-words whitespace-pre-wrap">{detail.body}</p>
        ) : (
          <p className="text-fg-faint italic">No description.</p>
        )}
      </section>

      {link.capabilities.ci && (
        <section className="space-y-2">
          <h3 className="text-xs font-semibold tracking-wide text-fg-muted uppercase">Checks</h3>
          {ci.isError ? (
            <p className="text-xs text-danger">{String(ci.error)}</p>
          ) : ci.data ? (
            <Checks status={ci.data} />
          ) : (
            <p className="text-xs text-fg-faint">Loading…</p>
          )}
        </section>
      )}

      {detail.reviews.length > 0 && (
        <section className="space-y-2">
          <h3 className="text-xs font-semibold tracking-wide text-fg-muted uppercase">Reviews</h3>
          <ul className="space-y-1">
            {detail.reviews.map((r) => {
              const [label, className] = REVIEW[r.state];
              return (
                <li key={r.author.username} className="flex items-center gap-2 text-xs">
                  <Avatar
                    name={r.author.name}
                    email={r.author.username}
                    size={18}
                    remote={showAvatars}
                    url={r.author.avatarUrl}
                  />
                  <span>{r.author.name}</span>
                  <span className={className}>{label}</span>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      <section className="space-y-3">
        <h3 className="text-xs font-semibold tracking-wide text-fg-muted uppercase">
          Comments ({detail.comments.length})
        </h3>
        {detail.comments.map((c, i) => (
          <article key={i} className="rounded-md border border-line">
            <header className="flex items-center gap-2 border-b border-line bg-raised/50 px-3 py-1.5 text-xs">
              <Avatar
                name={c.author.name}
                email={c.author.username}
                size={18}
                remote={showAvatars}
                url={c.author.avatarUrl}
              />
              <span className="font-medium">{c.author.name}</span>
              <span className="text-fg-faint" title={formatDate(Number(c.created))}>
                {relativeTime(Number(c.created))}
              </span>
            </header>
            <p className="p-3 break-words whitespace-pre-wrap select-text">{c.body}</p>
          </article>
        ))}
        <form
          className="space-y-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (draft.trim()) comment.mutate(draft.trim());
          }}
        >
          <textarea
            aria-label="New comment"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && draft.trim()) {
                e.preventDefault();
                comment.mutate(draft.trim());
              }
            }}
            rows={4}
            placeholder="Leave a comment (Ctrl+Enter sends it)"
            className="w-full resize-y rounded-md border border-line bg-canvas px-2 py-1.5 outline-none focus:border-accent"
          />
          <Button type="submit" variant="primary" disabled={!draft.trim() || comment.isPending}>
            Comment
          </Button>
        </form>
      </section>
    </div>
  );
}

function Files({ repo, pr, detail }: { repo: string; pr: PrRef; detail: PullRequestDetail }) {
  const { openFile } = useRepoView(repo);
  const head = detail.headSha;
  const files = useQuery({
    queryKey: [repo, "prFiles", pr.account, pr.path, pr.number, head],
    queryFn: async () => {
      const done = startTask(repo, "Fetching the changes");
      try {
        await ipc.prFetch({
          repo,
          remote: pr.remote,
          source: detail.source,
          head,
          base: detail.baseSha,
          targetBranch: detail.pr.targetBranch,
        });
      } finally {
        done();
      }
      return ipc.rangeFiles(
        repo,
        detail.baseSha ?? `refs/remotes/${pr.remote}/${detail.pr.targetBranch}`,
        head ?? "FETCH_HEAD",
      );
    },
    staleTime: Infinity,
    retry: false,
  });

  if (files.isError)
    return <p className="p-8 text-center text-danger select-text">{String(files.error)}</p>;
  if (!files.data) return <p className="p-8 text-center text-fg-faint">Fetching the changes…</p>;
  const range = files.data;
  return (
    <div className="py-2">
      <p className="px-3 pb-2 text-xs text-fg-muted">
        {range.files.length} {range.files.length === 1 ? "file" : "files"} changed since{" "}
        <span className="font-mono">{range.base.slice(0, 7)}</span>
      </p>
      <FileList
        files={range.files}
        activePath={
          openFile?.kind === "commit" && openFile.oid === range.head ? openFile.file.path : null
        }
        onOpen={(file) =>
          updateView(repo, {
            openFile: { kind: "commit", oid: range.head, file, base: range.base },
          })
        }
      />
    </div>
  );
}
