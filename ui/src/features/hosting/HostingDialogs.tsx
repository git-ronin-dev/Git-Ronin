import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import type { RepoLink } from "../../bindings/RepoLink";
import { ipc } from "../../lib/ipc";
import { Checkbox, Field, Select, TextInput } from "../../ui/Field";
import { toast } from "../../ui/toast-store";
import { useCommitDetail } from "../commit/queries";
import type { DialogRequest } from "../ops/dialog-store";
import { FormDialog } from "../ops/FormDialog";
import { useRefs } from "../workspace/queries";
import { updateView } from "../workspace/view";
import { closingText } from "./issues";
import { prNoun, prRef } from "./providers";
import { hostingKeys, pickLink, useRepoLinks } from "./queries";

type Of<K extends DialogRequest["kind"]> = Extract<DialogRequest, { kind: K }>;

const textarea =
  "w-full resize-y rounded-md border border-line bg-canvas px-2 py-1 text-fg outline-none focus:border-accent";

/** Branches people usually merge into, most likely first. */
const MAIN_BRANCHES = ["main", "master", "develop", "trunk"];

/** Where a pull request goes by default: the upstream of a fork, else origin. */
function defaultTarget(links: RepoLink[]): RepoLink | undefined {
  const prLinks = links.filter((l) => l.capabilities.pullRequests);
  return prLinks.find((l) => l.remote === "upstream") ?? prLinks[0];
}

export function CreatePullRequest({
  repo,
  branch,
  onClose,
}: Of<"createPullRequest"> & { onClose: () => void }) {
  const client = useQueryClient();
  const links = useRepoLinks(repo).data ?? [];
  const refs = useRefs(repo).data;
  const prLinks = links.filter((l) => l.capabilities.pullRequests);
  const [targetRemote, setTargetRemote] = useState<string | null>(null);
  const target = prLinks.find((l) => l.remote === targetRemote) ?? defaultTarget(links);

  const pushed = refs?.local.filter((b) => b.upstream && !b.upstream.gone) ?? [];
  const [source, setSource] = useState(branch ?? "");
  const local = refs?.local.find((b) => b.name === source);
  // origin/feature → remote "origin", branch "feature".
  const upstream = local?.upstream?.name;
  const upstreamRemote = refs?.remotes.find((r) => upstream?.startsWith(`${r.name}/`));
  const remoteBranch = upstreamRemote ? upstream!.slice(upstreamRemote.name.length + 1) : null;
  const sourceLink = links.find((l) => l.remote === upstreamRemote?.name);

  const targetBranches =
    refs?.remotes.find((r) => r.name === target?.remote)?.branches.map((b) => b.name) ?? [];
  const [into, setInto] = useState<string | null>(null);
  const shownInto =
    into ??
    MAIN_BRANCHES.find((b) => targetBranches.includes(b) && b !== remoteBranch) ??
    targetBranches.find((b) => b !== remoteBranch && b !== "HEAD") ??
    "";

  const tip = useCommitDetail(repo, local?.oid ?? null).data;
  const issue = useQuery({
    queryKey: [repo, "branchIssue", source],
    queryFn: () => ipc.branchIssue(repo, source),
    enabled: !!source,
  }).data;
  const summary = tip?.message.split("\n")[0] ?? "";
  const jiraKey = issue && !issue.startsWith("#") ? `${issue} ` : "";
  const [title, setTitle] = useState<string | null>(null);
  const [body, setBody] = useState<string | null>(null);
  const [draft, setDraft] = useState(false);
  const shownTitle = title ?? (summary ? `${jiraKey}${summary}` : "");
  const shownBody = body ?? (issue ? closingText(target?.kind, issue) : "");

  const noun = prNoun(target?.kind);
  const fork = sourceLink && target && sourceLink.path !== target.path ? sourceLink.path : null;
  const blocker = !target
    ? "No remote is on a service you're signed in to"
    : !source
      ? "Choose a branch"
      : !remoteBranch
        ? `Push ${source} first`
        : !sourceLink
          ? `${upstreamRemote?.name ?? "Its remote"} is not on a signed-in service`
          : !shownInto
            ? "Choose the branch to merge into"
            : !shownTitle.trim()
              ? "Enter a title"
              : null;

  const submit = async () => {
    if (!target || !remoteBranch) return false;
    try {
      const pr = await ipc.hostingCreatePullRequest(target.account, target.path, {
        title: shownTitle.trim(),
        body: shownBody,
        sourceBranch: remoteBranch,
        targetBranch: shownInto,
        sourceRepo: fork,
        draft,
      });
      void client.invalidateQueries({
        queryKey: hostingKeys.pullRequests(target.account, target.path),
      });
      toast.success(`Created ${prRef(target.kind, pr.number)}`, pr.title);
      updateView(repo, {
        pullRequest: {
          account: target.account,
          path: target.path,
          remote: target.remote,
          number: pr.number,
        },
        openFile: null,
      });
      return true;
    } catch (err) {
      toast.error(`Could not create the ${noun}`, String(err));
      return false;
    }
  };

  return (
    <FormDialog
      title={`Create ${noun}`}
      submitLabel="Create"
      blocker={blocker}
      onSubmit={submit}
      onClose={onClose}
    >
      <div className="grid grid-cols-2 gap-3">
        <Field
          label="Branch"
          hint={remoteBranch ? `Pushed as ${upstream}` : source ? "Not pushed yet" : undefined}
        >
          <Select value={source} onChange={(e) => setSource(e.target.value)}>
            <option value="" disabled>
              Choose…
            </option>
            {(refs?.local ?? []).map((b) => (
              <option key={b.name} value={b.name}>
                {b.name}
                {pushed.includes(b) ? "" : " (not pushed)"}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="Into" hint={target ? `${target.path} on ${target.remote}` : undefined}>
          <div className="flex gap-2">
            {prLinks.length > 1 && (
              <div className="w-28 shrink-0">
                <Select
                  aria-label="Repository"
                  value={target?.remote}
                  onChange={(e) => {
                    setTargetRemote(e.target.value);
                    setInto(null);
                  }}
                >
                  {prLinks.map((l) => (
                    <option key={l.remote} value={l.remote}>
                      {l.remote}
                    </option>
                  ))}
                </Select>
              </div>
            )}
            <Select value={shownInto} onChange={(e) => setInto(e.target.value)}>
              {targetBranches
                .filter((b) => b !== "HEAD")
                .map((b) => (
                  <option key={b} value={b}>
                    {b}
                  </option>
                ))}
            </Select>
          </div>
        </Field>
      </div>
      <Field label="Title">
        <TextInput value={shownTitle} onChange={(e) => setTitle(e.target.value)} autoFocus />
      </Field>
      <Field label="Description">
        <textarea
          value={shownBody}
          onChange={(e) => setBody(e.target.value)}
          rows={6}
          className={textarea}
        />
      </Field>
      {target?.capabilities.draft && (
        <Checkbox label="Create as a draft" checked={draft} onChange={setDraft} />
      )}
    </FormDialog>
  );
}

export function CreateIssue({ repo, onClose }: Of<"createIssue"> & { onClose: () => void }) {
  const client = useQueryClient();
  const link = pickLink(useRepoLinks(repo).data, "issues");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");

  const submit = async () => {
    if (!link) return false;
    try {
      const issue = await ipc.hostingCreateIssue(link.account, link.path, title.trim(), body);
      void client.invalidateQueries({ queryKey: hostingKeys.issues(link.account, link.path) });
      toast.success(`Created ${issue.key}`, issue.title);
      return true;
    } catch (err) {
      toast.error("Could not create the issue", String(err));
      return false;
    }
  };

  return (
    <FormDialog
      title={link ? `New issue in ${link.path}` : "New issue"}
      submitLabel="Create"
      blocker={
        !link
          ? "No remote is on a service you're signed in to"
          : title.trim()
            ? null
            : "Enter a title"
      }
      onSubmit={submit}
      onClose={onClose}
    >
      <Field label="Title">
        <TextInput value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
      </Field>
      <Field label="Description">
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          rows={6}
          className={textarea}
        />
      </Field>
    </FormDialog>
  );
}
