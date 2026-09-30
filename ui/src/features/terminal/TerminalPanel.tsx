import "@xterm/xterm/css/xterm.css";

import { clsx } from "clsx";
import { ChevronDown, ChevronUp, Plus, Search, X } from "lucide-react";
import { useRef, useState } from "react";

import { Tooltip } from "../../ui/Tooltip";
import { useUiPrefs } from "../workspace/queries";
import { updateView } from "../workspace/view";
import { Shell, type ShellHandle } from "./Shell";

interface ShellTab {
  id: number;
  /** Set when the shell exited: its code, or null if it was killed. */
  exited?: number | null;
}

/**
 * Shells in the repository folder, in tabs. They keep running while the
 * panel is hidden and end when the repository tab closes.
 */
export function TerminalPanel({ repo, visible }: { repo: string; visible: boolean }) {
  const fontSize = useUiPrefs()?.terminalFontSize ?? 13;
  const nextId = useRef(1);
  const [shells, setShells] = useState<ShellTab[]>([{ id: 0 }]);
  const [active, setActive] = useState(0);
  const handles = useRef(new Map<number, ShellHandle>());
  const [search, setSearch] = useState<string | null>(null);
  const searchInput = useRef<HTMLInputElement>(null);

  const add = () => {
    const id = nextId.current++;
    setShells((s) => [...s, { id }]);
    setActive(id);
  };

  const close = (id: number) => {
    const remaining = shells.filter((s) => s.id !== id);
    if (remaining.length === 0) {
      // The last one: start afresh and hide the panel.
      const fresh = nextId.current++;
      setShells([{ id: fresh }]);
      setActive(fresh);
      updateView(repo, { terminal: false });
      return;
    }
    setShells(remaining);
    if (id === active) setActive(remaining.at(-1)!.id);
  };

  const find = (backwards: boolean) => {
    const handle = handles.current.get(active);
    if (handle && search) handle.find(search, backwards);
  };

  const openSearch = () => {
    setSearch((s) => s ?? "");
    setTimeout(() => searchInput.current?.select(), 0);
  };

  const closeSearch = () => {
    setSearch(null);
    handles.current.get(active)?.clearSearch();
    handles.current.get(active)?.focus();
  };

  return (
    <section aria-label="Terminal" className="flex h-full flex-col bg-surface">
      <header className="flex h-7 shrink-0 items-center gap-1 border-b border-line px-2 text-xs text-fg-muted">
        <span className="mr-1 font-semibold tracking-wide uppercase">Terminal</span>
        <div role="tablist" aria-label="Shells" className="flex min-w-0 items-center gap-0.5">
          {shells.map((shell, i) => (
            <div
              key={shell.id}
              role="tab"
              aria-selected={shell.id === active}
              onClick={() => setActive(shell.id)}
              className={clsx(
                "group flex h-6 cursor-default items-center gap-1 rounded-sm pr-1 pl-2",
                shell.id === active ? "bg-hover text-fg" : "hover:bg-hover",
              )}
            >
              <span>Shell {i + 1}</span>
              {shell.exited !== undefined && (
                <span className="text-fg-faint">
                  ({shell.exited === null ? "stopped" : `exit ${shell.exited}`})
                </span>
              )}
              <button
                type="button"
                aria-label={`Close shell ${i + 1}`}
                onClick={(e) => {
                  e.stopPropagation();
                  close(shell.id);
                }}
                className="rounded-sm p-0.5 opacity-0 group-hover:opacity-100 group-aria-selected:opacity-100 hover:text-fg"
              >
                <X className="size-3" />
              </button>
            </div>
          ))}
          <IconButton label="New shell" onClick={add}>
            <Plus className="size-3.5" />
          </IconButton>
        </div>
        <div className="ml-auto flex items-center gap-1">
          {search !== null ? (
            <form
              className="flex items-center gap-0.5"
              onSubmit={(e) => {
                e.preventDefault();
                find(false);
              }}
            >
              <input
                ref={searchInput}
                autoFocus
                aria-label="Find in terminal"
                placeholder="Find…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") {
                    e.preventDefault();
                    e.stopPropagation();
                    closeSearch();
                  } else if (e.key === "Enter" && e.shiftKey) {
                    e.preventDefault();
                    find(true);
                  }
                }}
                className="h-5 w-40 rounded-sm border border-line bg-canvas px-1.5 text-fg outline-none focus:border-accent"
              />
              <IconButton label="Previous match" onClick={() => find(true)}>
                <ChevronUp className="size-3.5" />
              </IconButton>
              <IconButton label="Next match" onClick={() => find(false)}>
                <ChevronDown className="size-3.5" />
              </IconButton>
              <IconButton label="Close search" onClick={closeSearch}>
                <X className="size-3.5" />
              </IconButton>
            </form>
          ) : (
            <IconButton label="Find (Ctrl+Shift+F)" onClick={openSearch}>
              <Search className="size-3.5" />
            </IconButton>
          )}
          <IconButton label="Hide terminal" onClick={() => updateView(repo, { terminal: false })}>
            <ChevronDown className="size-3.5" />
          </IconButton>
        </div>
      </header>
      <div className="relative min-h-0 flex-1">
        {shells.map((shell) => (
          <Shell
            key={shell.id}
            repo={repo}
            fontSize={fontSize}
            visible={visible && shell.id === active}
            onReady={(handle) => {
              if (handle) handles.current.set(shell.id, handle);
              else handles.current.delete(shell.id);
            }}
            onSearch={openSearch}
            onExit={(code) =>
              setShells((s) => s.map((t) => (t.id === shell.id ? { ...t, exited: code } : t)))
            }
          />
        ))}
      </div>
    </section>
  );
}

function IconButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip content={label}>
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        className="rounded-sm p-0.5 hover:bg-hover hover:text-fg"
      >
        {children}
      </button>
    </Tooltip>
  );
}
