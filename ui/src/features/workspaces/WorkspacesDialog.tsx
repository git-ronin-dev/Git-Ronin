import { useQueries, useQueryClient } from "@tanstack/react-query";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { clsx } from "clsx";
import { ArrowDownToLine, FolderOpen, LoaderCircle, Plus, RefreshCw, X } from "lucide-react";
import { useState } from "react";

import { useOverlays } from "../../app/overlays";
import type { Config } from "../../bindings/Config";
import type { Operation } from "../../bindings/Operation";
import type { RepoSummary } from "../../bindings/RepoSummary";
import type { Workspace } from "../../bindings/Workspace";
import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { confirm } from "../../ui/confirm-store";
import { Dialog } from "../../ui/Dialog";
import { TextInput } from "../../ui/Field";
import { toast } from "../../ui/toast-store";
import { Tooltip } from "../../ui/Tooltip";
import { describeHead } from "../repo/head";
import { invalidateRepo, keys, useConfig } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { openWorkspace } from "./store";

const OPERATIONS: Record<Operation, string> = {
  merge: "Merge",
  rebase: "Rebase",
  cherryPick: "Cherry-pick",
  revert: "Revert",
  applyMailbox: "Applying patches",
  bisect: "Bisect",
};

const summaryKey = (path: string) => ["summary", path] as const;
/** Repositories fetched at the same time by "Fetch all". */
const PARALLEL_FETCHES = 3;

export function WorkspacesDialog() {
  const open = useOverlays((s) => s.workspaces);
  return (
    <Dialog
      open={open}
      onOpenChange={(workspaces) => useOverlays.setState({ workspaces })}
      title="Workspaces"
      size="large"
    >
      {open && <Workspaces />}
    </Dialog>
  );
}

function useSaveWorkspaces() {
  const client = useQueryClient();
  return async (workspaces: Workspace[]) => {
    client.setQueryData<Config>(keys.config, (c) =>
      c ? { ...c, local: { ...c.local, workspaces } } : c,
    );
    try {
      await ipc.workspacesSet(workspaces);
    } catch (err) {
      toast.error("Could not save workspaces", String(err));
      void client.invalidateQueries({ queryKey: keys.config });
    }
  };
}

function Workspaces() {
  const workspaces = useConfig().data?.local.workspaces ?? [];
  const save = useSaveWorkspaces();
  const [selected, setSelected] = useState<string | null>(null);
  const [naming, setNaming] = useState("");
  const current = workspaces.find((w) => w.name === selected) ?? workspaces[0];

  const create = (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    if (workspaces.some((w) => w.name === trimmed)) {
      toast.error("A workspace with that name exists");
      return;
    }
    // A new workspace starts with the open tabs.
    const repos = useWorkspace.getState().tabs.map((t) => t.path);
    void save([...workspaces, { name: trimmed, repos }]);
    setSelected(trimmed);
    setNaming("");
  };

  const update = (workspace: Workspace, next: Workspace) => {
    if (next.name !== workspace.name && workspaces.some((w) => w.name === next.name)) {
      toast.error("A workspace with that name exists");
      return;
    }
    void save(workspaces.map((w) => (w.name === workspace.name ? next : w)));
    setSelected(next.name);
  };

  return (
    <div className="flex h-full gap-4">
      <div className="flex w-48 shrink-0 flex-col gap-2">
        <p className="text-xs text-fg-faint">
          Groups of repositories to check, fetch and open together.
        </p>
        <ul aria-label="Workspaces" className="min-h-0 flex-1 space-y-0.5 overflow-y-auto">
          {workspaces.map((w) => (
            <li key={w.name}>
              <button
                type="button"
                aria-current={w.name === current?.name ? "true" : undefined}
                onClick={() => setSelected(w.name)}
                className={clsx(
                  "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left",
                  w.name === current?.name ? "bg-hover text-fg" : "text-fg-muted hover:bg-hover",
                )}
              >
                <span className="min-w-0 flex-1 truncate">{w.name}</span>
                <span className="text-xs text-fg-faint">{w.repos.length}</span>
              </button>
            </li>
          ))}
        </ul>
        <form
          className="flex gap-1"
          onSubmit={(e) => {
            e.preventDefault();
            create(naming);
          }}
        >
          <TextInput
            aria-label="New workspace name"
            placeholder="New workspace…"
            value={naming}
            onChange={(e) => setNaming(e.target.value)}
          />
          <Button type="submit" aria-label="Create workspace" disabled={!naming.trim()}>
            <Plus className="size-3.5" />
          </Button>
        </form>
      </div>
      <div className="min-w-0 flex-1">
        {current ? (
          <WorkspaceView
            key={current.name}
            workspace={current}
            onChange={(next) => update(current, next)}
            onDelete={() => void save(workspaces.filter((w) => w.name !== current.name))}
          />
        ) : (
          <p className="pt-8 text-center text-fg-faint">
            Name a workspace to start one with the open repositories.
          </p>
        )}
      </div>
    </div>
  );
}

function WorkspaceView({
  workspace,
  onChange,
  onDelete,
}: {
  workspace: Workspace;
  onChange: (next: Workspace) => void;
  onDelete: () => void;
}) {
  const client = useQueryClient();
  const [name, setName] = useState(workspace.name);
  const [fetching, setFetching] = useState<Set<string>>(new Set());
  const summaries = useQueries({
    queries: workspace.repos.map((path) => ({
      queryKey: summaryKey(path),
      queryFn: () => ipc.repoSummary(path),
      retry: false,
    })),
  });

  const refresh = () =>
    void client.invalidateQueries({ predicate: (q) => q.queryKey[0] === "summary" });

  const add = async () => {
    const path = await pickFolder({ directory: true, title: "Add repository" });
    if (!path) return;
    try {
      // Resolves a folder inside a repository to its root.
      const summary = await ipc.repoSummary(path);
      if (workspace.repos.includes(summary.info.path)) return;
      client.setQueryData(summaryKey(summary.info.path), summary);
      onChange({ ...workspace, repos: [...workspace.repos, summary.info.path] });
    } catch (err) {
      toast.error("Not a repository", String(err));
    }
  };

  const fetchAll = async () => {
    const queue = [...workspace.repos];
    const failures: string[] = [];
    setFetching(new Set(queue));
    const worker = async () => {
      for (let path = queue.shift(); path; path = queue.shift()) {
        try {
          await ipc.fetchPath(path);
          void invalidateRepo(client, path);
        } catch (err) {
          failures.push(`${path}: ${String(err)}`);
        }
        const done = path;
        setFetching((s) => new Set([...s].filter((p) => p !== done)));
        void client.invalidateQueries({ queryKey: summaryKey(done) });
      }
    };
    await Promise.all(Array.from({ length: PARALLEL_FETCHES }, worker));
    if (failures.length > 0) toast.error("Some fetches failed", failures.join("\n"));
    else toast.success(`Fetched ${workspace.name}`);
  };

  const remove = async () => {
    const ok = await confirm({
      title: "Delete workspace",
      message: `Delete the workspace “${workspace.name}”? The repositories are not touched.`,
      confirmLabel: "Delete",
      danger: true,
    });
    if (ok) onDelete();
  };

  const busy = fetching.size > 0;
  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex items-center gap-2">
        <TextInput
          aria-label="Workspace name"
          className="max-w-64"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() =>
            name.trim() && name.trim() !== workspace.name
              ? onChange({ ...workspace, name: name.trim() })
              : setName(workspace.name)
          }
          onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        />
        <div className="ml-auto flex gap-1">
          <Button variant="primary" onClick={() => void openWorkspace(workspace)}>
            <FolderOpen className="size-3.5" />
            Open all
          </Button>
          <Button disabled={busy || workspace.repos.length === 0} onClick={() => void fetchAll()}>
            {busy ? (
              <LoaderCircle className="size-3.5 animate-spin" />
            ) : (
              <ArrowDownToLine className="size-3.5" />
            )}
            Fetch all
          </Button>
          <Tooltip content="Refresh status">
            <Button aria-label="Refresh status" onClick={refresh}>
              <RefreshCw className="size-3.5" />
            </Button>
          </Tooltip>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto rounded-md border border-line">
        <table className="w-full">
          <thead>
            <tr className="border-b border-line text-left text-xs text-fg-faint">
              <th className="p-2 font-medium">Repository</th>
              <th className="p-2 font-medium">Branch</th>
              <th className="p-2 font-medium">Changes</th>
              <th className="p-2" />
            </tr>
          </thead>
          <tbody>
            {workspace.repos.map((path, i) => {
              const summary = summaries[i];
              return (
                <RepoRow
                  key={path}
                  path={path}
                  summary={summary?.data}
                  error={summary?.isError ? String(summary.error) : null}
                  fetching={fetching.has(path)}
                  onRemove={() =>
                    onChange({ ...workspace, repos: workspace.repos.filter((p) => p !== path) })
                  }
                />
              );
            })}
            {workspace.repos.length === 0 && (
              <tr>
                <td colSpan={4} className="p-4 text-center text-fg-faint">
                  No repositories yet.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="flex gap-2">
        <Button onClick={() => void add()}>
          <Plus className="size-3.5" />
          Add repository…
        </Button>
        <Button
          onClick={() => {
            const tabs = useWorkspace.getState().tabs.map((t) => t.path);
            const repos = [...workspace.repos, ...tabs.filter((p) => !workspace.repos.includes(p))];
            onChange({ ...workspace, repos });
          }}
        >
          Add open tabs
        </Button>
        <Button variant="dangerGhost" className="ml-auto" onClick={() => void remove()}>
          Delete workspace
        </Button>
      </div>
    </div>
  );
}

function RepoRow({
  path,
  summary,
  error,
  fetching,
  onRemove,
}: {
  path: string;
  summary: RepoSummary | undefined;
  error: string | null;
  fetching: boolean;
  onRemove: () => void;
}) {
  const openTab = useWorkspace((s) => s.open);
  const name = summary?.info.name ?? path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
  const changes = summary ? summary.staged + summary.unstaged : 0;
  return (
    <tr className="border-b border-line/50 align-top">
      <td className="max-w-0 p-2">
        <div className="flex items-center gap-1.5">
          {fetching && <LoaderCircle aria-label="Fetching" className="size-3 animate-spin" />}
          <span className="truncate font-medium">{name}</span>
        </div>
        <div className="truncate text-xs text-fg-faint" title={path}>
          {path}
        </div>
        {error && <div className="text-xs text-danger">{error}</div>}
      </td>
      <td className="p-2 whitespace-nowrap">
        {summary && (
          <>
            {describeHead(summary.info.head)}
            {(summary.ahead > 0 || summary.behind > 0) && (
              <span className="text-fg-faint">
                {summary.ahead > 0 && ` ↑${summary.ahead}`}
                {summary.behind > 0 && ` ↓${summary.behind}`}
              </span>
            )}
            {summary.info.operation && (
              <div className="text-xs text-warning">
                {OPERATIONS[summary.info.operation]} in progress
              </div>
            )}
          </>
        )}
      </td>
      <td className="p-2 whitespace-nowrap">
        {summary &&
          (summary.conflicted > 0 ? (
            <span className="text-danger">{summary.conflicted} conflicted</span>
          ) : changes > 0 ? (
            <span className="text-warning">{changes} changed</span>
          ) : (
            <span className="text-fg-faint">clean</span>
          ))}
      </td>
      <td className="p-2 text-right whitespace-nowrap">
        <Button variant="ghost" disabled={!!error} onClick={() => void openTab(path)}>
          Open
        </Button>
        <Tooltip content="Remove from workspace">
          <Button variant="ghost" aria-label={`Remove ${name} from workspace`} onClick={onRemove}>
            <X className="size-3.5" />
          </Button>
        </Tooltip>
      </td>
    </tr>
  );
}
