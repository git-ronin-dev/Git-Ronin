import { clsx } from "clsx";

import type { StatusEntry } from "../../bindings/StatusEntry";
import { copyText } from "../../lib/clipboard";
import { ContextMenu, type MenuItem } from "../../ui/ContextMenu";
import { Check, Minus, Plus, Undo2, type Icon } from "../../ui/icons";
import { useGitActions } from "../ops/actions";
import { useLfsStatus } from "../ops/queries";
import { updateView } from "../workspace/view";
import { Badge } from "../commit/FileList";
import { indent, treeRows, useCollapsed, type TreeRow } from "../commit/fileTree";
import { FolderRow } from "../commit/FileTreeParts";
import { splitPath } from "../commit/lines";
import { useUiPrefs } from "../workspace/queries";
import { ignoreChoices } from "./ignore";
import { useWorkingActions } from "./queries";

export type Side = "unstaged" | "staged" | "conflicted";

/** Long lists (a forgotten build directory) would stall the panel. */
const MAX_SHOWN = 1000;

interface ChangeListProps {
  repo: string;
  side: Side;
  entries: StatusEntry[];
  activePath: string | null;
  onOpen: (entry: StatusEntry) => void;
}

export function ChangeList({ repo, side, entries, activePath, onOpen }: ChangeListProps) {
  const actions = useWorkingActions(repo);
  const git = useGitActions(repo);
  const lfs = useLfsStatus(repo).data;
  const tree = useUiPrefs()?.fileTree ?? false;
  const [collapsed, toggle] = useCollapsed();
  const shown = entries.slice(0, MAX_SHOWN);
  const rows: TreeRow<StatusEntry>[] = tree
    ? treeRows(shown, (e) => e.path, collapsed)
    : shown.map((item) => ({ kind: "file", item, name: item.path, depth: 0 }));

  /** The row buttons, for one file or everything in a folder. */
  const buttonsFor = (targets: StatusEntry[]): { icon: Icon; label: string; run: () => void }[] =>
    side === "unstaged"
      ? [
          { icon: Undo2, label: "Discard changes", run: () => void actions.discard(targets) },
          { icon: Plus, label: "Stage", run: () => void actions.stage(targets) },
        ]
      : side === "staged"
        ? [{ icon: Minus, label: "Unstage", run: () => void actions.unstage(targets) }]
        : [{ icon: Check, label: "Mark resolved", run: () => void actions.stage(targets) }];

  return (
    <ul role={tree ? "tree" : "listbox"} aria-label={`${side} files`}>
      {rows.map((row) => {
        if (row.kind === "dir")
          return (
            <li key={`dir:${row.path}`}>
              <FolderRow
                row={row}
                onToggle={() => toggle(row.path)}
                actions={<RowButtons buttons={buttonsFor(row.items)} what={`${row.path}/`} />}
              />
            </li>
          );
        const entry = row.item;
        const [dir, name] = tree ? ["", row.name] : splitPath(entry.path);
        const buttons = buttonsFor([entry]);

        const tracked = entry.status !== "untracked" && entry.status !== "added";
        const menu: MenuItem[] = [
          ...(side === "conflicted"
            ? [{ label: "Resolve in the merge editor", onSelect: () => onOpen(entry) }]
            : []),
          ...buttons.map((b) => ({ label: b.label, onSelect: b.run })),
          ...(tracked && side !== "conflicted"
            ? [
                "separator" as const,
                {
                  label: "Blame",
                  onSelect: () =>
                    updateView(repo, { openFile: { kind: "blame", path: entry.path, rev: null } }),
                },
                {
                  label: "File history",
                  onSelect: () =>
                    updateView(repo, { openFile: { kind: "history", path: entry.path } }),
                },
              ]
            : []),
          ...(lfs?.initialized && entry.status !== "untracked" && entry.submodule === null
            ? [
                "separator" as const,
                { label: "Lock (LFS)", onSelect: () => void git.lfsLock(entry.path) },
              ]
            : []),
          ...(entry.status === "untracked"
            ? [
                "separator" as const,
                ...ignoreChoices(entry.path).map((c) => ({
                  label: c.label,
                  onSelect: () => void actions.ignore(entry.path, c.scope),
                })),
              ]
            : []),
          "separator",
          { label: "Copy path", onSelect: () => void copyText(entry.path, "Copied path") },
        ];

        return (
          <li key={entry.path}>
            <ContextMenu items={menu}>
              <div
                role={tree ? "treeitem" : "option"}
                tabIndex={0}
                style={tree ? { paddingLeft: indent(row.depth) + 6 } : undefined}
                aria-selected={entry.path === activePath}
                title={entry.oldPath ? `${entry.oldPath} → ${entry.path}` : entry.path}
                onClick={() => onOpen(entry)}
                onKeyDown={(e) => e.key === "Enter" && onOpen(entry)}
                className={clsx(
                  "group flex h-7 cursor-default items-center gap-2 px-3",
                  entry.path === activePath ? "bg-accent/20" : "hover:bg-hover",
                )}
              >
                <Badge status={entry.status} />
                <span className="min-w-0 flex-1 truncate">
                  <span className="text-fg-faint">{dir}</span>
                  {name}
                </span>
                <RowButtons buttons={buttons} what={entry.path} />
              </div>
            </ContextMenu>
          </li>
        );
      })}
      {entries.length > shown.length && (
        <li className="px-3 py-1 text-xs text-fg-faint">
          and {(entries.length - shown.length).toLocaleString()} more
        </li>
      )}
    </ul>
  );
}

/** Hover buttons on a file or folder row. */
function RowButtons({
  buttons,
  what,
}: {
  buttons: { icon: Icon; label: string; run: () => void }[];
  what: string;
}) {
  return (
    <span className="flex shrink-0 gap-0.5 opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100">
      {buttons.map(({ icon: Icon, label, run }) => (
        <button
          key={label}
          type="button"
          aria-label={`${label} ${what}`}
          title={label}
          onClick={(e) => {
            e.stopPropagation();
            run();
          }}
          className="flex size-5 items-center justify-center rounded-sm text-fg-muted hover:bg-raised hover:text-fg"
        >
          <Icon className="size-3.5" />
        </button>
      ))}
    </span>
  );
}
