import { clsx } from "clsx";

import type { FlowConfig } from "../../bindings/FlowConfig";
import type { FlowKind } from "../../bindings/FlowKind";
import type { Submodule } from "../../bindings/Submodule";
import type { SubmoduleState } from "../../bindings/SubmoduleState";
import { copyText } from "../../lib/clipboard";
import { ContextMenu, type MenuItem } from "../../ui/ContextMenu";
import { DropdownMenu } from "../../ui/DropdownMenu";
import {
  Boxes,
  FolderGit2,
  GitMerge,
  HardDriveDownload,
  Lock,
  Plus,
  RefreshCw,
  Workflow,
} from "../../ui/icons";
import { Section } from "../../ui/Section";
import { Tooltip } from "../../ui/Tooltip";
import { revealCommit } from "../graph/reveal";
import { useGitActions } from "../ops/actions";
import { openDialog } from "../ops/dialog-store";
import { useFlowConfig, useLfsLocks, useLfsStatus, useWorktrees } from "../ops/queries";
import { useRefs, useRepoInfo } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { Folder, Leaf } from "./SidebarTree";

const icon = "size-3.5";

/** The last segment of a path. */
function baseName(path: string): string {
  return (
    path
      .replace(/[\\/]+$/, "")
      .split(/[\\/]/)
      .pop() ?? path
  );
}

function HeaderButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick?: () => void;
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

/** "Add…" menu next to the sidebar filter. */
export function AddMenu({ repo }: { repo: string }) {
  const lfs = useLfsStatus(repo).data;
  const flow = useFlowConfig(repo).data;
  const actions = useGitActions(repo);
  const items: MenuItem[] = [
    { label: "Add remote…", onSelect: () => openDialog({ kind: "remote", repo, remote: null }) },
    { label: "Add submodule…", onSelect: () => openDialog({ kind: "addSubmodule", repo }) },
    {
      label: "Add working tree…",
      onSelect: () => openDialog({ kind: "addWorktree", repo, branch: null }),
    },
    "separator",
    ...(lfs?.installed
      ? [
          {
            label: "Track files with Git LFS…",
            onSelect: () => openDialog({ kind: "lfsTrack", repo, pattern: "" }),
          },
          ...(lfs.initialized
            ? []
            : [{ label: "Set up Git LFS here", onSelect: () => void actions.lfsInit() }]),
        ]
      : [{ label: "Git LFS is not installed", disabled: true, onSelect: () => {} }]),
    flow
      ? { label: "Git Flow is set up", disabled: true, onSelect: () => {} }
      : { label: "Set up Git Flow…", onSelect: () => openDialog({ kind: "flowInit", repo }) },
  ];
  return (
    <DropdownMenu items={items} align="end">
      <button
        type="button"
        aria-label="Add remote, submodule, working tree…"
        className="flex size-6 items-center justify-center rounded-sm text-fg-faint hover:bg-hover hover:text-fg"
      >
        <Plus className={icon} />
      </button>
    </DropdownMenu>
  );
}

const SUBMODULE_STATES: Record<SubmoduleState, { label: string; className: string } | null> = {
  current: null,
  uninitialized: { label: "not cloned", className: "text-fg-faint" },
  moved: { label: "moved", className: "text-warning" },
  conflicted: { label: "conflict", className: "text-danger" },
};

export function SubmoduleSection({ repo, submodules }: { repo: string; submodules: Submodule[] }) {
  const actions = useGitActions(repo);
  const stale = submodules.filter((s) => s.state !== "current").map((s) => s.path);
  return (
    <Section
      title="Submodules"
      icon={<Boxes className={icon} />}
      count={submodules.length}
      action={
        <HeaderButton
          label={stale.length > 0 ? "Update submodules" : "Add submodule"}
          onClick={() =>
            stale.length > 0
              ? void actions.updateSubmodules([])
              : openDialog({ kind: "addSubmodule", repo })
          }
        >
          {stale.length > 0 ? <RefreshCw className={icon} /> : <Plus className={icon} />}
        </HeaderButton>
      }
    >
      {submodules.map((s) => {
        const state = SUBMODULE_STATES[s.state];
        const cloned = s.state !== "uninitialized";
        const open = () => void useWorkspace.getState().open(`${repo}/${s.path}`);
        return (
          <ContextMenu
            key={s.path}
            items={[
              { label: "Open", disabled: !cloned, onSelect: open },
              {
                label: cloned ? "Update to the recorded commit" : "Clone",
                onSelect: () => void actions.updateSubmodules([s.path]),
              },
              "separator",
              { label: "Copy path", onSelect: () => void copyText(s.path, "Copied path") },
            ]}
          >
            <Leaf
              depth={1}
              title={cloned ? `Open ${s.path}` : `${s.path} is not cloned yet`}
              onClick={() => (cloned ? open() : void actions.updateSubmodules([s.path]))}
              dimmed={!cloned}
            >
              <span className="truncate">{s.name}</span>
              {state && (
                <span className={clsx("shrink-0 text-[10px]", state.className)}>{state.label}</span>
              )}
            </Leaf>
          </ContextMenu>
        );
      })}
    </Section>
  );
}

/** Working trees, Git Flow and Git LFS: shown when the repository uses them. */
export function RepoSections({ repo }: { repo: string }) {
  return (
    <>
      <WorktreeSection repo={repo} />
      <FlowSection repo={repo} />
      <LfsSection repo={repo} />
    </>
  );
}

function WorktreeSection({ repo }: { repo: string }) {
  const worktrees = useWorktrees(repo).data ?? [];
  const actions = useGitActions(repo);
  if (worktrees.length < 2) return null;
  const prunable = worktrees.some((w) => w.prunable);
  return (
    <Section
      title="Working trees"
      icon={<FolderGit2 className={icon} />}
      count={worktrees.length}
      action={
        <HeaderButton
          label="Add working tree"
          onClick={() => openDialog({ kind: "addWorktree", repo, branch: null })}
        >
          <Plus className={icon} />
        </HeaderButton>
      }
    >
      {worktrees.map((w) => {
        const open = () => void useWorkspace.getState().open(w.path);
        const where = w.branch ?? (w.head ? `detached at ${w.head.slice(0, 7)}` : "");
        return (
          <ContextMenu
            key={w.path}
            items={[
              { label: "Open", disabled: w.isCurrent || w.prunable, onSelect: open },
              {
                label: "Remove…",
                disabled: w.isMain || w.isCurrent,
                onSelect: () => void actions.removeWorktree(w.path),
              },
              ...(prunable
                ? [
                    {
                      label: "Forget missing working trees",
                      onSelect: () => void actions.pruneWorktrees(),
                    },
                  ]
                : []),
              "separator",
              { label: "Copy path", onSelect: () => void copyText(w.path, "Copied path") },
            ]}
          >
            <Leaf
              depth={1}
              title={`${w.path}${w.locked !== null ? `\nLocked${w.locked ? `: ${w.locked}` : ""}` : ""}${w.prunable ? "\nIts folder is gone" : ""}`}
              onClick={() => !w.isCurrent && !w.prunable && open()}
              dimmed={w.prunable}
            >
              {w.isCurrent && (
                <span
                  className="size-1.5 shrink-0 rounded-full bg-accent"
                  aria-label="this working tree"
                />
              )}
              <span className={clsx("truncate", w.isCurrent && "font-semibold")}>
                {w.isMain ? `${baseName(w.path)} (main)` : baseName(w.path)}
              </span>
              <span className="min-w-0 shrink truncate text-[10px] text-fg-faint">{where}</span>
              {w.locked !== null && <Lock className="size-3 shrink-0 text-fg-faint" />}
            </Leaf>
          </ContextMenu>
        );
      })}
    </Section>
  );
}

const FLOW_KINDS: { kind: FlowKind; title: string }[] = [
  { kind: "feature", title: "Features" },
  { kind: "release", title: "Releases" },
  { kind: "hotfix", title: "Hotfixes" },
];

function flowPrefix(config: FlowConfig, kind: FlowKind): string {
  return {
    feature: config.featurePrefix,
    release: config.releasePrefix,
    hotfix: config.hotfixPrefix,
  }[kind];
}

function FlowSection({ repo }: { repo: string }) {
  const config = useFlowConfig(repo).data;
  const refs = useRefs(repo).data;
  const busy = !!useRepoInfo(repo).data?.operation;
  const actions = useGitActions(repo);
  if (!config) return null;
  const start: MenuItem[] = FLOW_KINDS.map(({ kind }) => ({
    label: `Start ${kind}…`,
    onSelect: () => openDialog({ kind: "flowStart", repo, flow: kind }),
  }));
  const groups = FLOW_KINDS.map(({ kind, title }) => ({
    kind,
    title,
    branches: (refs?.local ?? []).filter((b) => b.name.startsWith(flowPrefix(config, kind))),
  }));
  return (
    <Section
      title="Git Flow"
      icon={<Workflow className={icon} />}
      count={groups.reduce((n, g) => n + g.branches.length, 0)}
      action={
        <DropdownMenu items={start} align="end">
          <button
            type="button"
            aria-label="Start a feature, release or hotfix"
            className="rounded-sm text-fg-faint opacity-0 group-hover:opacity-100 hover:text-fg focus-visible:opacity-100"
          >
            <Plus className={icon} />
          </button>
        </DropdownMenu>
      }
    >
      <p className="px-4 pb-1 text-[10px] text-fg-faint">
        {config.develop} ← features · {config.master} ← releases, hotfixes
      </p>
      {groups
        .filter((g) => g.branches.length > 0)
        .map((g) => (
          <Folder key={g.kind} name={g.title} depth={1} icon={<GitMerge className={icon} />}>
            {g.branches.map((b) => (
              <ContextMenu
                key={b.name}
                items={[
                  {
                    label: `Check out ${b.name}`,
                    disabled: b.isHead,
                    onSelect: () => void actions.checkout(b.name),
                  },
                  {
                    label: `Finish ${g.kind}…`,
                    disabled: busy,
                    onSelect: () =>
                      openDialog({ kind: "flowFinish", repo, flow: g.kind, branch: b.name }),
                  },
                ]}
              >
                <Leaf
                  depth={2}
                  onClick={() => void revealCommit(repo, b.oid)}
                  onDoubleClick={b.isHead ? undefined : () => void actions.checkout(b.name)}
                >
                  <span className={clsx("truncate", b.isHead && "font-semibold")}>
                    {b.name.slice(flowPrefix(config, g.kind).length)}
                  </span>
                </Leaf>
              </ContextMenu>
            ))}
          </Folder>
        ))}
    </Section>
  );
}

function LfsSection({ repo }: { repo: string }) {
  const status = useLfsStatus(repo).data;
  const actions = useGitActions(repo);
  if (!status?.installed || (!status.initialized && status.patterns.length === 0)) return null;
  return (
    <Section
      title="Git LFS"
      icon={<HardDriveDownload className={icon} />}
      count={status.patterns.length}
      defaultOpen={false}
      action={
        <HeaderButton
          label="Track files with LFS"
          onClick={() => openDialog({ kind: "lfsTrack", repo, pattern: "" })}
        >
          <Plus className={icon} />
        </HeaderButton>
      }
    >
      {!status.initialized && (
        <div className="flex items-center gap-2 px-4 py-1 text-xs text-warning">
          <span className="min-w-0 flex-1">LFS isn't set up in this clone.</span>
          <button
            type="button"
            className="text-accent hover:underline"
            onClick={() => void actions.lfsInit()}
          >
            Set up
          </button>
        </div>
      )}
      {status.patterns.map((p) => (
        <ContextMenu
          key={`${p.source}:${p.pattern}`}
          items={[
            {
              label: "Stop tracking",
              onSelect: () => void actions.lfsUntrack(p.pattern, p.source),
            },
          ]}
        >
          <Leaf depth={1} title={`From ${p.source}`} onClick={() => {}}>
            <span className="truncate font-mono text-xs">{p.pattern}</span>
          </Leaf>
        </ContextMenu>
      ))}
      <Folder name="Locks" depth={1} icon={<Lock className={icon} />} defaultOpen={false}>
        <LfsLocks repo={repo} />
      </Folder>
    </Section>
  );
}

/** Mounted only while the Locks folder is open, so the server is asked then. */
function LfsLocks({ repo }: { repo: string }) {
  const locks = useLfsLocks(repo, true);
  const actions = useGitActions(repo);
  if (locks.isPending) return <p className="px-6 py-1 text-xs text-fg-faint">Asking the server…</p>;
  if (locks.isError) {
    return (
      <p className="px-6 py-1 text-xs break-words text-danger select-text">{String(locks.error)}</p>
    );
  }
  if (locks.data.length === 0) return <p className="px-6 py-1 text-xs text-fg-faint">No locks</p>;
  return locks.data.map((lock) => (
    <ContextMenu
      key={lock.id}
      items={[
        {
          label: lock.ours === false ? "Force unlock…" : "Unlock",
          onSelect: () => void actions.lfsUnlock(lock.id, lock.path, lock.ours),
        },
        "separator",
        { label: "Copy path", onSelect: () => void copyText(lock.path, "Copied path") },
      ]}
    >
      <Leaf
        depth={2}
        title={`${lock.path}\nLocked by ${lock.owner} ${lock.lockedAt}`}
        onClick={() => {}}
      >
        <span className="truncate">{baseName(lock.path)}</span>
        <span className={clsx("shrink-0 text-[10px]", lock.ours ? "text-accent" : "text-fg-faint")}>
          {lock.ours ? "you" : lock.owner}
        </span>
      </Leaf>
    </ContextMenu>
  ));
}
