import { useMutation, useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useState } from "react";

import { copyText } from "../../lib/clipboard";
import { ipc } from "../../lib/ipc";
import { ContextMenu, type MenuItem } from "../../ui/ContextMenu";
import {
  Archive,
  Cloud,
  Eye,
  EyeOff,
  Folder as FolderIcon,
  GitBranch,
  Plus,
  Search,
  Tag,
} from "../../ui/icons";
import { Section } from "../../ui/Section";
import { toast } from "../../ui/toast-store";
import { Tooltip } from "../../ui/Tooltip";
import { revealCommit } from "../graph/reveal";
import { useFork } from "../hosting/fork";
import { HostingSections } from "../hosting/HostingSections";
import { useRepoLinks } from "../hosting/queries";
import { useGitActions } from "../ops/actions";
import { openDialog } from "../ops/dialog-store";
import { beginDrag, dropProps, useDrag, type DragRef } from "../ops/drag";
import { localBranchMenu, remoteBranchMenu, remoteMenu, tagMenu } from "../ops/menus";
import { useRepoContext } from "../ops/queries";
import { useStashActions } from "../stash/actions";
import { invalidateRepo, keys, useConfig, useRefs } from "../workspace/queries";
import { updateView } from "../workspace/view";
import { AddMenu, RepoSections, SubmoduleSection } from "./RepoSections";
import { Folder, Leaf } from "./SidebarTree";
import { buildTree, type TreeNode } from "./tree";

const icon = "size-3.5";

interface RefItem {
  name: string;
  fullName: string;
  oid: string;
  current?: boolean;
  ahead?: number;
  behind?: number;
  gone?: boolean;
  /** Git actions, shown above the graph filter items. */
  menu: MenuItem[];
  /** Branches can be dragged onto each other. */
  drag?: DragRef;
  onDoubleClick?: () => void;
}

export function Sidebar({ repo }: { repo: string }) {
  const refs = useRefs(repo);
  const [filter, setFilter] = useState("");
  const graphFilter = useGraphFilter(repo);
  const stashActions = useStashActions(repo);
  const actions = useGitActions(repo);
  const ctx = useRepoContext(repo);
  const links = useRepoLinks(repo).data ?? [];
  const fork = useFork(repo, ctx.remoteNames);

  if (refs.isError) return <p className="p-3 text-danger">{String(refs.error)}</p>;
  if (!refs.data) return null;
  const { local, remotes, tags, stashes, submodules } = refs.data;
  const match = (name: string) => name.toLowerCase().includes(filter.trim().toLowerCase());

  const localItems: RefItem[] = local
    .filter((b) => match(b.name))
    .map((b) => ({
      name: b.name,
      fullName: b.fullName,
      oid: b.oid,
      current: b.isHead,
      ahead: b.upstream?.ahead,
      behind: b.upstream?.behind,
      gone: b.upstream?.gone,
      menu: localBranchMenu(actions, ctx, b),
      drag: { kind: "local", name: b.name, fullName: b.fullName, oid: b.oid },
      onDoubleClick: b.isHead ? undefined : () => void actions.checkout(b.name),
    }));
  const tagItems: RefItem[] = tags
    .filter((t) => match(t.name))
    .map((t) => ({ ...t, menu: tagMenu(actions, ctx, t) }));

  return (
    <nav aria-label="Repository" className="flex h-full flex-col">
      <div className="flex h-9 shrink-0 items-center gap-2 border-b border-line px-3">
        <Search className="size-3.5 text-fg-faint" />
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter"
          aria-label="Filter branches and tags"
          className="h-7 min-w-0 flex-1 bg-transparent text-xs outline-none placeholder:text-fg-faint"
        />
        <AddMenu repo={repo} />
      </div>
      {graphFilter.solo.size > 0 && (
        <div className="flex items-center justify-between border-b border-line bg-accent/10 px-3 py-1.5 text-xs">
          <span>Showing only {graphFilter.solo.size} ref(s)</span>
          <button
            type="button"
            className="text-accent hover:underline"
            onClick={graphFilter.clearSolo}
          >
            Show all
          </button>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto py-2">
        <Section title="Local" icon={<GitBranch className={icon} />} count={local.length}>
          <RefTree repo={repo} items={localItems} filter={graphFilter} depth={1} />
        </Section>
        <Section
          title="Remote"
          icon={<Cloud className={icon} />}
          count={remotes.length}
          action={
            <Tooltip content="Add remote">
              <button
                type="button"
                aria-label="Add remote"
                onClick={() => openDialog({ kind: "remote", repo, remote: null })}
                className="rounded-sm text-fg-faint opacity-0 group-hover:opacity-100 hover:text-fg focus-visible:opacity-100"
              >
                <Plus className={icon} />
              </button>
            </Tooltip>
          }
        >
          {remotes.map((remote) => {
            const items: RefItem[] = remote.branches
              .filter((b) => match(`${remote.name}/${b.name}`))
              .map((b) => ({
                ...b,
                menu: remoteBranchMenu(actions, ctx, remote.name, b),
                drag: {
                  kind: "remote",
                  name: `${remote.name}/${b.name}`,
                  fullName: b.fullName,
                  oid: b.oid,
                  remote: remote.name,
                  branch: b.name,
                },
                onDoubleClick: () =>
                  void actions.checkoutRemote(b.fullName, b.name, ctx.localNames),
              }));
            return (
              <Folder
                key={remote.name}
                name={remote.name}
                depth={1}
                icon={<Cloud className={icon} />}
                title={remote.url ?? undefined}
                menu={[
                  ...remoteMenu(actions, ctx, remote),
                  ...links
                    .filter((l) => l.remote === remote.name && l.capabilities.fork)
                    .flatMap((l) => [
                      "separator" as const,
                      {
                        label: `Fork ${l.path} to your account`,
                        onSelect: () => void fork(l, remote.url),
                      },
                    ]),
                ]}
              >
                <RefTree repo={repo} items={items} filter={graphFilter} depth={2} />
              </Folder>
            );
          })}
        </Section>
        <HostingSections repo={repo} />
        <Section
          title="Tags"
          icon={<Tag className={icon} />}
          count={tags.length}
          defaultOpen={false}
        >
          <RefTree repo={repo} items={tagItems} filter={graphFilter} depth={1} />
        </Section>
        <Section title="Stashes" icon={<Archive className={icon} />} count={stashes.length}>
          {stashes.map((s) => (
            <ContextMenu
              key={s.oid}
              items={[
                { label: "Apply", onSelect: () => void stashActions.apply(s, false) },
                { label: "Pop", onSelect: () => void stashActions.apply(s, true) },
                { label: "Delete…", onSelect: () => void stashActions.drop(s) },
                "separator",
                {
                  label: "Copy message",
                  onSelect: () => void copyText(s.message, "Copied message"),
                },
              ]}
            >
              <Leaf
                depth={1}
                title={s.message}
                onClick={() => updateView(repo, { selected: s.oid, openFile: null })}
              >
                <span className="truncate">{s.message}</span>
              </Leaf>
            </ContextMenu>
          ))}
        </Section>
        <SubmoduleSection repo={repo} submodules={submodules} />
        <RepoSections repo={repo} />
      </div>
    </nav>
  );
}

interface GraphFilterApi {
  hidden: Set<string>;
  solo: Set<string>;
  toggleHidden: (fullName: string) => void;
  toggleSolo: (fullName: string) => void;
  clearSolo: () => void;
}

function useGraphFilter(repo: string): GraphFilterApi {
  const client = useQueryClient();
  const state = useConfig().data?.local.repos[repo];
  const hidden = new Set(state?.hiddenRefs ?? []);
  const solo = new Set(state?.soloRefs ?? []);
  const save = useMutation({
    mutationFn: (next: { hidden: Set<string>; solo: Set<string> }) =>
      ipc.setGraphFilter(repo, [...next.hidden], [...next.solo]),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: keys.config });
      void invalidateRepo(client, repo);
    },
    onError: (err) => toast.error("Could not update graph filter", String(err)),
  });
  const toggle = (set: Set<string>, item: string) => {
    const next = new Set(set);
    if (!next.delete(item)) next.add(item);
    return next;
  };
  return {
    hidden,
    solo,
    toggleHidden: (name) => save.mutate({ hidden: toggle(hidden, name), solo }),
    toggleSolo: (name) => save.mutate({ hidden, solo: toggle(solo, name) }),
    clearSolo: () => save.mutate({ hidden, solo: new Set() }),
  };
}

function RefTree(props: { repo: string; items: RefItem[]; filter: GraphFilterApi; depth: number }) {
  return <Nodes {...props} nodes={buildTree(props.items, (i) => i.name)} />;
}

function Nodes({
  repo,
  nodes,
  filter,
  depth,
}: {
  repo: string;
  nodes: TreeNode<RefItem>[];
  filter: GraphFilterApi;
  depth: number;
}) {
  return nodes.map((node) =>
    node.kind === "folder" ? (
      <Folder key={node.path} name={node.name} depth={depth} icon={<FolderIcon className={icon} />}>
        <Nodes repo={repo} nodes={node.children} filter={filter} depth={depth + 1} />
      </Folder>
    ) : (
      <RefLeaf
        key={node.item.fullName}
        repo={repo}
        item={node.item}
        label={node.name}
        filter={filter}
        depth={depth}
      />
    ),
  );
}

function RefLeaf({
  repo,
  item,
  label,
  filter,
  depth,
}: {
  repo: string;
  item: RefItem;
  label: string;
  filter: GraphFilterApi;
  depth: number;
}) {
  const hidden = filter.hidden.has(item.fullName);
  const solo = filter.solo.has(item.fullName);
  const over = useDrag((s) => s.over !== null && s.over === item.fullName);
  const menu: MenuItem[] = [
    ...item.menu,
    ...(item.menu.length > 0 ? (["separator"] as const) : []),
    {
      label: hidden ? "Show in graph" : "Hide in graph",
      onSelect: () => filter.toggleHidden(item.fullName),
    },
    { label: solo ? "Remove from solo" : "Solo", onSelect: () => filter.toggleSolo(item.fullName) },
    "separator",
    { label: "Copy name", onSelect: () => void copyText(item.name, "Copied name") },
  ];
  return (
    <ContextMenu items={menu}>
      <Leaf
        depth={depth}
        onClick={() => void revealCommit(repo, item.oid)}
        onDoubleClick={item.onDoubleClick}
        onPointerDown={item.drag ? (e) => beginDrag(repo, item.drag!, e) : undefined}
        {...(item.drag ? dropProps(item.drag) : {})}
        highlighted={over}
        dimmed={hidden || (filter.solo.size > 0 && !solo)}
        trailing={
          <button
            type="button"
            aria-label={hidden ? `Show ${item.name}` : `Hide ${item.name}`}
            onClick={(e) => {
              e.stopPropagation();
              filter.toggleHidden(item.fullName);
            }}
            className={clsx(
              "shrink-0 text-fg-faint hover:text-fg",
              hidden ? "opacity-100" : "opacity-0 group-hover:opacity-100",
            )}
          >
            {hidden ? <EyeOff className={icon} /> : <Eye className={icon} />}
          </button>
        }
      >
        {item.current && (
          <span className="size-1.5 shrink-0 rounded-full bg-accent" aria-label="current branch" />
        )}
        <span className={clsx("truncate", item.current && "font-semibold")}>{label}</span>
        <Tracking ahead={item.ahead} behind={item.behind} gone={item.gone} />
      </Leaf>
    </ContextMenu>
  );
}

function Tracking({
  ahead = 0,
  behind = 0,
  gone,
}: {
  ahead?: number;
  behind?: number;
  gone?: boolean;
}) {
  if (gone)
    return (
      <span className="shrink-0 text-[10px] text-warning" title="Upstream branch was deleted">
        gone
      </span>
    );
  if (!ahead && !behind) return null;
  return (
    <span
      className="flex shrink-0 gap-1 text-[10px] text-fg-muted"
      title={`${ahead} ahead, ${behind} behind upstream`}
    >
      {ahead > 0 && <span>↑{ahead}</span>}
      {behind > 0 && <span>↓{behind}</span>}
    </span>
  );
}
