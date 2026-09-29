import {
  Archive,
  ArchiveRestore,
  ArrowDownToLine,
  ArrowUpFromLine,
  FolderOpen,
  GitBranchPlus,
  PanelLeft,
  PanelRight,
  Redo2,
  Undo2,
  type LucideIcon,
} from "lucide-react";

import { useRepoStore } from "../features/repo/store";
import { Tooltip } from "../ui/Tooltip";

interface ToolbarProps {
  onToggleSidebar: () => void;
  onToggleDetails: () => void;
}

export function Toolbar({ onToggleSidebar, onToggleDetails }: ToolbarProps) {
  const repo = useRepoStore((s) => s.repo);
  const pickAndOpen = useRepoStore((s) => s.pickAndOpen);
  // Repository actions are wired up in Phases 2 and 3.
  const actionsDisabled = true;

  return (
    // Three columns keep the action group centred whatever the repo name's width.
    <header className="grid h-12 shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-line bg-surface px-2">
      <div className="flex min-w-0 items-center gap-1">
        <ToolButton icon={PanelLeft} label="Toggle sidebar" onClick={onToggleSidebar} />
        <ToolButton
          icon={FolderOpen}
          label="Open repository"
          shortcut="Ctrl+O"
          onClick={pickAndOpen}
        />
        <span className="ml-1 truncate font-semibold">{repo?.name}</span>
      </div>

      <div className="flex items-center gap-1">
        <ToolButton icon={Undo2} label="Undo" disabled={actionsDisabled} showLabel />
        <ToolButton icon={Redo2} label="Redo" disabled={actionsDisabled} showLabel />
        <Divider />
        <ToolButton icon={ArrowDownToLine} label="Pull" disabled={actionsDisabled} showLabel />
        <ToolButton icon={ArrowUpFromLine} label="Push" disabled={actionsDisabled} showLabel />
        <Divider />
        <ToolButton icon={GitBranchPlus} label="Branch" disabled={actionsDisabled} showLabel />
        <ToolButton icon={Archive} label="Stash" disabled={actionsDisabled} showLabel />
        <ToolButton icon={ArchiveRestore} label="Pop" disabled={actionsDisabled} showLabel />
      </div>

      <div className="flex justify-end">
        <ToolButton icon={PanelRight} label="Toggle details" onClick={onToggleDetails} />
      </div>
    </header>
  );
}

interface ToolButtonProps {
  icon: LucideIcon;
  label: string;
  shortcut?: string;
  showLabel?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

function ToolButton({
  icon: Icon,
  label,
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
  if (showLabel && !shortcut) return button;
  return <Tooltip content={shortcut ? `${label} (${shortcut})` : label}>{button}</Tooltip>;
}

function Divider() {
  return <div className="mx-1 h-6 w-px bg-line" />;
}
