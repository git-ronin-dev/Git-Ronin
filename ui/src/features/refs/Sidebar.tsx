import { useMutation, useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import {
  Archive,
  Boxes,
  ChevronRight,
  Cloud,
  Eye,
  EyeOff,
  Folder as FolderIcon,
  GitBranch,
  Search,
  Tag,
} from "lucide-react";
import { useState, type ComponentProps, type ReactNode } from "react";

import { copyText } from "../../lib/clipboard";
import { ipc } from "../../lib/ipc";
import { ContextMenu, type MenuItem } from "../../ui/ContextMenu";
import { Section } from "../../ui/Section";
import { toast } from "../../ui/toast-store";
import { revealCommit } from "../graph/reveal";
import { invalidateRepo, keys, useConfig, useRefs } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { updateView } from "../workspace/view";
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
}

export function Sidebar({ repo }: { repo: string }) {
  const refs = useRefs(repo);
  const [filter, setFilter] = useState("");
  const graphFilter = useGraphFilter(repo);

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
    }));
  const tagItems: RefItem[] = tags.filter((t) => match(t.name));

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
        <Section title="Remote" icon={<Cloud className={icon} />} count={remotes.length}>
          {remotes.map((remote) => {
            const items = remote.branches.filter((b) => match(`${remote.name}/${b.name}`));
            return (
              <Folder
                key={remote.name}
                name={remote.name}
                depth={1}
                icon={<Cloud className={icon} />}
              >
                <RefTree repo={repo} items={items} filter={graphFilter} depth={2} />
              </Folder>
            );
          })}
        </Section>
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
            <Leaf
              key={s.oid}
              depth={1}
              title={s.message}
              onClick={() => updateView(repo, { selected: s.oid, openFile: null })}
            >
              <span className="truncate">{s.message}</span>
            </Leaf>
          ))}
        </Section>
        <Section title="Submodules" icon={<Boxes className={icon} />} count={submodules.length}>
          {submodules.map((s) => (
            <Leaf
              key={s.path}
              depth={1}
              title={`Open ${s.path}`}
              onClick={() => void useWorkspace.getState().open(`${repo}/${s.path}`)}
            >
              <span className="truncate">{s.name}</span>
            </Leaf>
          ))}
        </Section>
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
  const menu: MenuItem[] = [
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

const indent = (depth: number) => ({ paddingLeft: 8 + depth * 14 });

function Folder({
  name,
  depth,
  icon: folderIcon,
  children,
}: {
  name: string;
  depth: number;
  icon: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        style={indent(depth - 1)}
        className="flex h-7 w-full items-center gap-1.5 pr-3 text-fg-muted hover:bg-hover hover:text-fg"
      >
        <ChevronRight
          className={clsx("size-3.5 shrink-0 transition-transform", open && "rotate-90")}
        />
        {folderIcon}
        <span className="truncate">{name}</span>
      </button>
      {open && children}
    </div>
  );
}

/** Extra props (and ref) are passed through so it can be a context menu trigger. */
function Leaf({
  depth,
  title,
  onClick,
  dimmed,
  trailing,
  children,
  ...rest
}: {
  depth: number;
  title?: string;
  onClick: () => void;
  dimmed?: boolean;
  trailing?: ReactNode;
  children: ReactNode;
} & Omit<ComponentProps<"div">, "onClick" | "title" | "children">) {
  return (
    <div
      {...rest}
      role="button"
      tabIndex={0}
      title={title}
      onClick={onClick}
      onKeyDown={(e) => e.key === "Enter" && onClick()}
      style={indent(depth)}
      className={clsx(
        "group flex h-7 cursor-default items-center gap-2 pr-3 hover:bg-hover",
        dimmed && "opacity-45",
      )}
    >
      {children}
      <span className="ml-auto" />
      {trailing}
    </div>
  );
}
