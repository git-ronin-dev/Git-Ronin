import { forwardRef, type ComponentProps } from "react";

import type { PullMode } from "../bindings/PullMode";
import { countChanges, useWorkingStatus } from "../features/changes/queries";
import { useShortcutLabel } from "../features/commands/useShortcuts";
import { PULL_LABELS, useGitActions } from "../features/ops/actions";
import { openDialog } from "../features/ops/dialogs";
import { useJournal } from "../features/ops/queries";
import { useTask } from "../features/ops/tasks";
import { describeHead } from "../features/repo/head";
import { useStashActions } from "../features/stash/actions";
import { useRefs, useRepoInfo } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { updateView, useRepoView } from "../features/workspace/view";
import { DropdownMenu } from "../ui/DropdownMenu";
import {
  Archive,
  ArchiveRestore,
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronDown,
  GitBranchPlus,
  PanelLeft,
  PanelRight,
  Redo2,
  PaperPlane,
  Search,
  Settings,
  SquareTerminal,
  Undo2,
  type Icon,
} from "../ui/icons";
import { toast } from "../ui/toast-store";
import { Tooltip } from "../ui/Tooltip";
import { openSettings, useOverlays } from "./overlays";

interface ToolbarProps {
  hasRepo: boolean;
  onToggleSidebar: () => void;
  onToggleDetails: () => void;
  onSearch: () => void;
}

export function Toolbar({ hasRepo, onToggleSidebar, onToggleDetails, onSearch }: ToolbarProps) {
  const active = useWorkspace((s) => s.active);

  return (
    // Three columns keep the action group centred.
    <header className="grid h-12 shrink-0 grid-cols-[1fr_auto_1fr] items-center bg-base px-2">
      <div className="flex items-center gap-1">
        <ToolButton
          icon={PanelLeft}
          label="Toggle sidebar"
          shortcut={useShortcutLabel("view.sidebar")}
          onClick={onToggleSidebar}
        />
      </div>

      <div className="flex items-center gap-1">
        {active ? (
          <RepoButtons key={active} repo={active} />
        ) : (
          <>
            <ToolButton icon={Undo2} label="Undo" disabled showLabel />
            <ToolButton icon={Redo2} label="Redo" disabled showLabel />
            <Divider />
            <ToolButton icon={ArrowDownToLine} label="Pull" disabled showLabel />
            <ToolButton icon={ArrowUpFromLine} label="Push" disabled showLabel />
            <Divider />
            <ToolButton icon={GitBranchPlus} label="Branch" disabled showLabel />
            <ToolButton icon={Archive} label="Stash" disabled showLabel />
            <ToolButton icon={ArchiveRestore} label="Pop" disabled showLabel />
          </>
        )}
      </div>

      <div className="flex justify-end gap-1">
        <ToolButton
          icon={Search}
          label="Search commits"
          shortcut={useShortcutLabel("graph.search")}
          onClick={onSearch}
          disabled={!hasRepo}
        />
        {active ? (
          <TerminalButton repo={active} />
        ) : (
          <ToolButton icon={SquareTerminal} label="Toggle terminal" disabled />
        )}
        <ToolButton
          icon={PaperPlane}
          label="Launchpad"
          shortcut={useShortcutLabel("hosting.launchpad")}
          onClick={() => useOverlays.setState({ launchpad: true })}
        />
        <ToolButton
          icon={Settings}
          label="Settings"
          shortcut={useShortcutLabel("settings.open")}
          onClick={() => openSettings()}
        />
        <ToolButton icon={PanelRight} label="Toggle details" onClick={onToggleDetails} />
      </div>
    </header>
  );
}

function TerminalButton({ repo }: { repo: string }) {
  const open = useRepoView(repo).terminal;
  return (
    <ToolButton
      icon={SquareTerminal}
      label={open ? "Hide terminal" : "Show terminal"}
      shortcut={useShortcutLabel("terminal.toggle")}
      onClick={() => updateView(repo, { terminal: !open })}
    />
  );
}

function RepoButtons({ repo }: { repo: string }) {
  const actions = useGitActions(repo);
  const undoKey = useShortcutLabel("repo.undo");
  const redoKey = useShortcutLabel("repo.redo");
  const journal = useJournal(repo).data;
  const info = useRepoInfo(repo).data;
  const refs = useRefs(repo).data;
  const busy = useTask(repo) !== undefined;
  const current = refs?.local.find((b) => b.isHead);
  const upstream = current?.upstream?.name ?? null;
  const hasRemotes = (refs?.remotes.length ?? 0) > 0;
  const unborn = info?.head.kind === "branch" && info.head.unborn;

  const stepTooltip = (verb: string, step = journal?.undo) =>
    step ? (step.blocked ?? `${verb} “${step.label}”`) : `Nothing to ${verb.toLowerCase()}`;
  const pull = (mode: PullMode) => void actions.pull(mode);
  const pullMenu = [
    { label: "Fetch all remotes", disabled: !hasRemotes, onSelect: () => void actions.fetch(null) },
    "separator" as const,
    ...(["fastForwardOnly", "merge", "rebase"] as const).map((mode) => ({
      label: PULL_LABELS[mode],
      disabled: !upstream,
      onSelect: () => pull(mode),
    })),
  ];

  return (
    <>
      <ToolButton
        icon={Undo2}
        label="Undo"
        tooltip={stepTooltip("Undo")}
        shortcut={undoKey}
        // A blocked step stays clickable so it can say why.
        disabled={!journal?.undo}
        onClick={() =>
          journal?.undo?.blocked
            ? toast.info(`Can't undo “${journal.undo.label}”`, journal.undo.blocked)
            : void actions.undo(false)
        }
        showLabel
      />
      <ToolButton
        icon={Redo2}
        label="Redo"
        tooltip={stepTooltip("Redo", journal?.redo)}
        shortcut={redoKey}
        disabled={!journal?.redo}
        onClick={() => void actions.undo(true)}
        showLabel
      />
      <Divider />
      <div className="flex items-center">
        <ToolButton
          icon={ArrowDownToLine}
          label="Pull"
          tooltip={
            upstream
              ? `Pull ${upstream} into ${current!.name}`
              : "The current branch has no upstream"
          }
          disabled={!upstream || busy}
          onClick={() => pull("default")}
          showLabel
        />
        <DropdownMenu items={pullMenu}>
          <MenuCaret aria-label="More pull and fetch options" disabled={busy} />
        </DropdownMenu>
      </div>
      <ToolButton
        icon={ArrowUpFromLine}
        label="Push"
        tooltip={
          !current
            ? "Check out a branch to push it"
            : !hasRemotes
              ? "Add a remote to push to"
              : upstream
                ? `Push ${current.name} to ${upstream}`
                : `Push ${current.name}…`
        }
        disabled={!current || !hasRemotes || busy}
        onClick={() => current && void actions.push(current.name, upstream)}
        showLabel
      />
      <Divider />
      <ToolButton
        icon={GitBranchPlus}
        label="Branch"
        tooltip="Create a branch at the current commit"
        disabled={!info || unborn}
        onClick={() =>
          info &&
          openDialog({
            kind: "createBranch",
            repo,
            start: "HEAD",
            startLabel: describeHead(info.head),
          })
        }
        showLabel
      />
      <StashButtons repo={repo} />
    </>
  );
}

const MenuCaret = forwardRef<HTMLButtonElement, ComponentProps<"button">>(
  function MenuCaret(props, ref) {
    return (
      <button
        ref={ref}
        type="button"
        {...props}
        className="flex h-10 w-4 items-center justify-center rounded-md text-fg-muted hover:bg-hover hover:text-fg disabled:pointer-events-none disabled:opacity-35"
      >
        <ChevronDown className="size-3" />
      </button>
    );
  },
);

function StashButtons({ repo }: { repo: string }) {
  const changes = countChanges(useWorkingStatus(repo).data);
  const latest = useRefs(repo).data?.stashes[0];
  const actions = useStashActions(repo);
  return (
    <>
      <ToolButton
        icon={Archive}
        label="Stash"
        disabled={changes === 0}
        onClick={() => useOverlays.setState({ stash: repo })}
        showLabel
      />
      <ToolButton
        icon={ArchiveRestore}
        label="Pop"
        tooltip={latest ? `Pop “${latest.message}”` : undefined}
        disabled={!latest}
        onClick={() => latest && void actions.apply(latest, true)}
        showLabel
      />
    </>
  );
}

interface ToolButtonProps {
  icon: Icon;
  label: string;
  /** Overrides the label as the tooltip. */
  tooltip?: string;
  shortcut?: string;
  showLabel?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

function ToolButton({
  icon: Icon,
  label,
  tooltip,
  shortcut,
  showLabel,
  disabled,
  onClick,
}: ToolButtonProps) {
  const button = (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className="flex h-10 min-w-9 flex-col items-center justify-center gap-0.5 rounded-md px-2 text-fg-muted transition-colors hover:bg-hover hover:text-fg disabled:pointer-events-none disabled:opacity-35"
    >
      <Icon className="size-4" />
      {showLabel && <span className="text-[10px] leading-none">{label}</span>}
    </button>
  );
  if (showLabel && !shortcut && !tooltip) return button;
  const content = tooltip ?? label;
  return <Tooltip content={shortcut ? `${content} (${shortcut})` : content}>{button}</Tooltip>;
}

function Divider() {
  return <div className="mx-1 h-6 w-px bg-line" />;
}
