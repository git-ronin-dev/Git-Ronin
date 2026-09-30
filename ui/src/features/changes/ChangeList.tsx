import { clsx } from "clsx";
import { Check, Minus, Plus, Undo2, type LucideIcon } from "lucide-react";

import type { StatusEntry } from "../../bindings/StatusEntry";
import { copyText } from "../../lib/clipboard";
import { ContextMenu, type MenuItem } from "../../ui/ContextMenu";
import { useGitActions } from "../ops/actions";
import { useLfsStatus } from "../ops/queries";
import { updateView } from "../workspace/view";
import { Badge } from "../commit/FileList";
import { splitPath } from "../commit/lines";
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
  const shown = entries.slice(0, MAX_SHOWN);

  return (
    <ul role="listbox" aria-label={`${side} files`}>
      {shown.map((entry) => {
        const [dir, name] = splitPath(entry.path);
        const buttons: { icon: LucideIcon; label: string; run: () => void }[] =
          side === "unstaged"
            ? [
                { icon: Undo2, label: "Discard changes", run: () => void actions.discard([entry]) },
                { icon: Plus, label: "Stage", run: () => void actions.stage([entry]) },
              ]
            : side === "staged"
              ? [{ icon: Minus, label: "Unstage", run: () => void actions.unstage([entry]) }]
              : [{ icon: Check, label: "Mark resolved", run: () => void actions.stage([entry]) }];

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
                role="option"
                tabIndex={0}
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
                <span className="flex shrink-0 gap-0.5 opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100">
                  {buttons.map(({ icon: Icon, label, run }) => (
                    <button
                      key={label}
                      type="button"
                      aria-label={`${label} ${entry.path}`}
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
