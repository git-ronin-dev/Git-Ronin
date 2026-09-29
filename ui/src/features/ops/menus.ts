import type { LocalBranch } from "../../bindings/LocalBranch";
import type { Refs } from "../../bindings/Refs";
import type { Remote } from "../../bindings/Remote";
import type { RemoteBranch } from "../../bindings/RemoteBranch";
import type { RepoInfo } from "../../bindings/RepoInfo";
import type { Tag } from "../../bindings/Tag";
import { copyText } from "../../lib/clipboard";
import type { MenuItem } from "../../ui/ContextMenu";
import { shortRev, type GitActions } from "./actions";
import { openDialog } from "./dialogs";

/** What menus need to know about the repository. */
export interface RepoContext {
  repo: string;
  /** The checked-out branch, or null when HEAD is detached (or unknown). */
  head: string | null;
  localNames: string[];
  remoteNames: string[];
  /** A merge, rebase, … is waiting; history actions would fail. */
  busy: boolean;
}

export function repoContext(repo: string, info?: RepoInfo, refs?: Refs): RepoContext {
  return {
    repo,
    head: info?.head.kind === "branch" ? info.head.name : null,
    localNames: refs?.local.map((b) => b.name) ?? [],
    remoteNames: refs?.remotes.map((r) => r.name) ?? [],
    busy: !!info?.operation,
  };
}

/** Name of what HEAD points at, for labels. */
const headLabel = (ctx: RepoContext) => ctx.head ?? "HEAD";

export function localBranchMenu(a: GitActions, ctx: RepoContext, b: LocalBranch): MenuItem[] {
  const current = b.name === ctx.head;
  const head = headLabel(ctx);
  const items: MenuItem[] = [
    { label: `Check out ${b.name}`, disabled: current, onSelect: () => void a.checkout(b.name) },
  ];
  if (!current) {
    items.push(
      {
        label: `Merge ${b.name} into ${head}`,
        disabled: ctx.busy,
        onSelect: () => void a.merge(b.name, head),
      },
      {
        label: `Rebase ${head} onto ${b.name}`,
        disabled: ctx.busy || !ctx.head,
        onSelect: () => void a.rebase(b.name, head),
      },
    );
  }
  items.push(
    "separator",
    ...(current && b.upstream
      ? [{ label: "Pull", disabled: ctx.busy, onSelect: () => void a.pull("default") }]
      : []),
    {
      label: b.upstream ? `Push to ${b.upstream.name}` : "Push…",
      disabled: ctx.remoteNames.length === 0,
      onSelect: () => void a.push(b.name, b.upstream?.name ?? null),
    },
    {
      label: "Set upstream…",
      disabled: ctx.remoteNames.length === 0,
      onSelect: () =>
        openDialog({
          kind: "upstream",
          repo: ctx.repo,
          branch: b.name,
          upstream: b.upstream?.name ?? null,
        }),
    },
    "separator",
    ...createHere(ctx, b.name, b.name),
    {
      label: "Rename…",
      onSelect: () => openDialog({ kind: "renameBranch", repo: ctx.repo, name: b.name }),
    },
    { label: "Delete…", disabled: current, onSelect: () => void a.deleteBranch(b.name) },
  );
  return items;
}

export function remoteBranchMenu(
  a: GitActions,
  ctx: RepoContext,
  remote: string,
  b: RemoteBranch,
): MenuItem[] {
  const name = `${remote}/${b.name}`;
  const head = headLabel(ctx);
  return [
    {
      label: ctx.localNames.includes(b.name) ? `Check out ${b.name}` : `Check out as ${b.name}`,
      disabled: b.name === ctx.head,
      onSelect: () => void a.checkoutRemote(b.fullName, b.name, ctx.localNames),
    },
    {
      label: `Merge ${name} into ${head}`,
      disabled: ctx.busy,
      onSelect: () => void a.merge(name, head),
    },
    {
      label: `Rebase ${head} onto ${name}`,
      disabled: ctx.busy || !ctx.head,
      onSelect: () => void a.rebase(name, head),
    },
    "separator",
    ...createHere(ctx, b.fullName, name),
    {
      label: `Delete from ${remote}…`,
      onSelect: () => void a.deleteRemoteRef(remote, `refs/heads/${b.name}`, name),
    },
  ];
}

export function tagMenu(a: GitActions, ctx: RepoContext, tag: Tag): MenuItem[] {
  return [
    {
      label: `Check out ${tag.name}`,
      onSelect: () => void a.checkoutDetached(tag.fullName),
    },
    "separator",
    ...ctx.remoteNames.map((remote) => ({
      label: `Push to ${remote}`,
      onSelect: () => void a.pushTag(remote, tag.name),
    })),
    ...createHere(ctx, tag.fullName, tag.name).slice(0, 1),
    { label: "Delete…", onSelect: () => void a.deleteTag(tag.name) },
    ...ctx.remoteNames.map((remote) => ({
      label: `Delete from ${remote}…`,
      onSelect: () => void a.deleteRemoteRef(remote, tag.fullName, tag.name),
    })),
  ];
}

export function remoteMenu(a: GitActions, ctx: RepoContext, remote: Remote): MenuItem[] {
  return [
    { label: `Fetch ${remote.name}`, onSelect: () => void a.fetch(remote.name) },
    "separator",
    { label: "Edit…", onSelect: () => openDialog({ kind: "remote", repo: ctx.repo, remote }) },
    { label: "Remove…", onSelect: () => void a.removeRemote(remote.name) },
    ...(remote.url
      ? ([
          "separator",
          { label: "Copy URL", onSelect: () => void copyText(remote.url!, "Copied URL") },
        ] as MenuItem[])
      : []),
  ];
}

export function commitMenu(a: GitActions, ctx: RepoContext, oid: string): MenuItem[] {
  const head = headLabel(ctx);
  const reset = (mode: "soft" | "mixed" | "hard", label: string): MenuItem => ({
    label: `Reset ${head} here: ${label}`,
    disabled: ctx.busy,
    onSelect: () => void a.reset(oid, mode, head),
  });
  return [
    { label: "Check out this commit", onSelect: () => void a.checkoutDetached(oid) },
    ...createHere(ctx, oid, shortRev(oid)),
    "separator",
    {
      label: `Cherry-pick onto ${head}`,
      disabled: ctx.busy,
      onSelect: () => void a.cherryPick(oid),
    },
    { label: "Revert", disabled: ctx.busy, onSelect: () => void a.revert(oid) },
    { label: `Merge into ${head}`, disabled: ctx.busy, onSelect: () => void a.merge(oid, head) },
    "separator",
    reset("soft", "keep changes staged"),
    reset("mixed", "keep changes unstaged"),
    reset("hard", "discard changes…"),
  ];
}

/** "Create branch here…" and "Create tag here…" at `rev`. */
function createHere(ctx: RepoContext, rev: string, label: string): MenuItem[] {
  return [
    {
      label: "Create branch here…",
      onSelect: () =>
        openDialog({ kind: "createBranch", repo: ctx.repo, start: rev, startLabel: label }),
    },
    {
      label: "Create tag here…",
      onSelect: () =>
        openDialog({ kind: "createTag", repo: ctx.repo, target: rev, targetLabel: label }),
    },
  ];
}
