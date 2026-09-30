import { clsx } from "clsx";
import type { ReactNode } from "react";

import { ChevronRight, Folder, List, ListTree } from "../../ui/icons";
import { Tooltip } from "../../ui/Tooltip";
import { useSetUiPrefs, useUiPrefs } from "../workspace/queries";
import { indent, type TreeRow } from "./fileTree";

export function FolderRow<T>({
  row,
  onToggle,
  actions,
}: {
  row: Extract<TreeRow<T>, { kind: "dir" }>;
  onToggle: () => void;
  /** Buttons shown on hover, e.g. "stage folder". */
  actions?: ReactNode;
}) {
  return (
    <div
      role="treeitem"
      aria-expanded={!row.collapsed}
      aria-selected={false}
      tabIndex={0}
      title={row.path}
      onClick={onToggle}
      onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onToggle()}
      style={{ paddingLeft: indent(row.depth) - 12 }}
      className="group flex h-7 cursor-default items-center gap-1 pr-3 text-fg-muted hover:bg-hover"
    >
      <ChevronRight
        className={clsx("size-3.5 shrink-0 transition-transform", !row.collapsed && "rotate-90")}
      />
      <Folder className="size-3.5 shrink-0" />
      <span className="min-w-0 flex-1 truncate">{row.name}</span>
      {actions}
      <span className="shrink-0 text-xs text-fg-faint">{row.items.length}</span>
    </div>
  );
}

/** Switches changed-file lists between a flat list and a folder tree. */
export function FileLayoutToggle() {
  const prefs = useUiPrefs();
  const save = useSetUiPrefs();
  if (!prefs) return null;
  const tree = prefs.fileTree;
  const label = tree ? "Show files as a list" : "Show files as a folder tree";
  const Icon = tree ? List : ListTree;
  return (
    <Tooltip content={label}>
      <button
        type="button"
        aria-label={label}
        onClick={() => save.mutate({ ...prefs, fileTree: !tree })}
        className="flex size-6 items-center justify-center rounded-sm text-fg-muted hover:bg-hover hover:text-fg"
      >
        <Icon className="size-3.5" />
      </button>
    </Tooltip>
  );
}
