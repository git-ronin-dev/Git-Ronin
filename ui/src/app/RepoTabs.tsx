import { clsx } from "clsx";
import { Plus, X } from "lucide-react";

import { useWorkspace } from "../features/workspace/store";
import { Tooltip } from "../ui/Tooltip";

export function RepoTabs() {
  const { tabs, active, activate, close, pickAndOpen } = useWorkspace();
  return (
    <div
      role="tablist"
      aria-label="Open repositories"
      className="flex h-9 shrink-0 items-end gap-0.5 overflow-x-auto bg-canvas px-2 pt-1"
    >
      {tabs.map((tab) => (
        <div
          key={tab.path}
          role="tab"
          aria-selected={tab.path === active}
          title={tab.path}
          onClick={() => activate(tab.path)}
          // Middle click closes, as in browsers.
          onAuxClick={(e) => e.button === 1 && void close(tab.path)}
          className={clsx(
            "group flex h-8 max-w-56 min-w-28 cursor-default items-center gap-2 rounded-t-md border border-b-0 px-3",
            tab.path === active
              ? "border-line bg-surface text-fg"
              : "border-transparent text-fg-muted hover:bg-hover hover:text-fg",
          )}
        >
          <span className="min-w-0 flex-1 truncate">{tab.name}</span>
          <button
            type="button"
            aria-label={`Close ${tab.name}`}
            onClick={(e) => {
              e.stopPropagation();
              void close(tab.path);
            }}
            className={clsx(
              "rounded-sm text-fg-faint hover:bg-hover hover:text-fg",
              tab.path === active ? "opacity-100" : "opacity-0 group-hover:opacity-100",
            )}
          >
            <X className="size-3.5" />
          </button>
        </div>
      ))}
      <Tooltip content="Open repository (Ctrl+O)">
        <button
          type="button"
          aria-label="Open repository"
          onClick={() => void pickAndOpen()}
          className="mb-1 flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-hover hover:text-fg"
        >
          <Plus className="size-4" />
        </button>
      </Tooltip>
    </div>
  );
}
