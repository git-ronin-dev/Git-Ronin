import {
  Archive,
  ArchiveRestore,
  ArrowDownToLine,
  ArrowUpFromLine,
  GitBranchPlus,
  PanelLeft,
  PanelRight,
  Redo2,
  Search,
  Undo2,
  type LucideIcon,
} from "lucide-react";
import { useState } from "react";

import { countChanges, useWorkingStatus } from "../features/changes/queries";
import { useStashActions } from "../features/stash/actions";
import { StashDialog } from "../features/stash/StashDialog";
import { useRefs } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { Tooltip } from "../ui/Tooltip";

interface ToolbarProps {
  hasRepo: boolean;
  onToggleSidebar: () => void;
  onToggleDetails: () => void;
  onSearch: () => void;
}

export function Toolbar({ hasRepo, onToggleSidebar, onToggleDetails, onSearch }: ToolbarProps) {
  // Undo, pull, push and branch arrive in Phase 3.
  const actionsDisabled = true;
  const active = useWorkspace((s) => s.active);

  return (
    // Three columns keep the action group centred.
    <header className="grid h-12 shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-line bg-surface px-2">
      <div className="flex items-center gap-1">
        <ToolButton icon={PanelLeft} label="Toggle sidebar" onClick={onToggleSidebar} />
      </div>

      <div className="flex items-center gap-1">
        <ToolButton icon={Undo2} label="Undo" disabled={actionsDisabled} showLabel />
        <ToolButton icon={Redo2} label="Redo" disabled={actionsDisabled} showLabel />
        <Divider />
        <ToolButton icon={ArrowDownToLine} label="Pull" disabled={actionsDisabled} showLabel />
        <ToolButton icon={ArrowUpFromLine} label="Push" disabled={actionsDisabled} showLabel />
        <Divider />
        <ToolButton icon={GitBranchPlus} label="Branch" disabled={actionsDisabled} showLabel />
        {active ? (
          <StashButtons key={active} repo={active} />
        ) : (
          <>
            <ToolButton icon={Archive} label="Stash" disabled showLabel />
            <ToolButton icon={ArchiveRestore} label="Pop" disabled showLabel />
          </>
        )}
      </div>

      <div className="flex justify-end gap-1">
        <ToolButton
          icon={Search}
          label="Search commits"
          shortcut="Ctrl+F"
          onClick={onSearch}
          disabled={!hasRepo}
        />
        <ToolButton icon={PanelRight} label="Toggle details" onClick={onToggleDetails} />
      </div>
    </header>
  );
}

function StashButtons({ repo }: { repo: string }) {
  const [open, setOpen] = useState(false);
  const changes = countChanges(useWorkingStatus(repo).data);
  const latest = useRefs(repo).data?.stashes[0];
  const actions = useStashActions(repo);
  return (
    <>
      <ToolButton
        icon={Archive}
        label="Stash"
        disabled={changes === 0}
        onClick={() => setOpen(true)}
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
      <StashDialog repo={repo} open={open} onOpenChange={setOpen} />
    </>
  );
}

interface ToolButtonProps {
  icon: LucideIcon;
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
