import * as RadixDialog from "@radix-ui/react-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import { Search } from "lucide-react";
import { useMemo, useRef, useState, type KeyboardEvent } from "react";

import { useOverlays } from "../../app/overlays";
import { useWorkspace } from "../workspace/store";
import { COMMANDS, dynamicCommands, type Command } from "./commands";
import { fuzzyFilter } from "./fuzzy";
import { formatShortcut } from "./keys";
import { useBindings } from "./useShortcuts";

/** Ctrl+P: every command, searchable. */
export function CommandPalette() {
  const open = useOverlays((s) => s.palette);
  return (
    <RadixDialog.Root open={open} onOpenChange={(palette) => useOverlays.setState({ palette })}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="fixed inset-0 z-40 bg-canvas/50" />
        <RadixDialog.Content
          aria-describedby={undefined}
          className="fixed top-[12vh] left-1/2 z-50 flex max-h-[70vh] w-[min(600px,92vw)] -translate-x-1/2 flex-col overflow-hidden rounded-lg border border-line bg-surface shadow-2xl outline-none"
        >
          <RadixDialog.Title className="sr-only">Commands</RadixDialog.Title>
          {open && <Palette />}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}

function Palette() {
  const client = useQueryClient();
  const repo = useWorkspace((s) => s.active);
  const bindings = useBindings();
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const list = useRef<HTMLUListElement>(null);

  // Built once per opening: the palette shows what was true when it opened.
  const commands = useMemo(() => {
    const ctx = { client, repo };
    return [...COMMANDS, ...dynamicCommands(ctx)].filter((c) => c.enabled?.(ctx) ?? true);
  }, [client, repo]);
  const matches = useMemo(
    () => fuzzyFilter(commands, query, (c) => `${c.category}: ${c.title}`),
    [commands, query],
  );
  const selected = Math.min(index, Math.max(matches.length - 1, 0));

  const run = (command: Command) => {
    useOverlays.setState({ palette: false });
    // After the dialog has closed and given focus back.
    setTimeout(() => void command.run({ client, repo: useWorkspace.getState().active }), 0);
  };

  const move = (to: number) => {
    const next = (to + matches.length) % Math.max(matches.length, 1);
    setIndex(next);
    list.current?.children[next]?.scrollIntoView({ block: "nearest" });
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown") move(selected + 1);
    else if (e.key === "ArrowUp") move(selected - 1);
    else if (e.key === "PageDown") move(Math.min(selected + 8, matches.length - 1));
    else if (e.key === "PageUp") move(Math.max(selected - 8, 0));
    else if (e.key === "Enter" && matches[selected]) run(matches[selected]);
    else return;
    e.preventDefault();
  };

  return (
    <>
      <div className="flex items-center gap-2 border-b border-line px-3">
        <Search className="size-4 shrink-0 text-fg-faint" />
        <input
          autoFocus
          role="combobox"
          aria-expanded
          aria-controls="palette-list"
          aria-activedescendant={matches[selected] ? `palette-${selected}` : undefined}
          aria-label="Search commands"
          placeholder="Type a command, branch, repository or profile…"
          spellCheck={false}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={onKeyDown}
          className="h-11 min-w-0 flex-1 bg-transparent text-fg outline-none placeholder:text-fg-faint"
        />
      </div>
      <ul
        id="palette-list"
        ref={list}
        role="listbox"
        className="min-h-0 flex-1 overflow-y-auto p-1"
      >
        {matches.map((command, i) => {
          const shortcut = command.dynamic ? undefined : bindings.get(command.id)?.[0];
          return (
            <li
              key={command.id}
              id={`palette-${i}`}
              role="option"
              aria-selected={i === selected}
              onMouseMove={() => i !== selected && setIndex(i)}
              onClick={() => run(command)}
              className={clsx(
                "flex h-8 cursor-default items-center gap-2 rounded-md px-2",
                i === selected && "bg-accent text-accent-fg",
              )}
            >
              <span className={clsx("shrink-0", i === selected ? "opacity-80" : "text-fg-faint")}>
                {command.category}:
              </span>
              <span className="min-w-0 flex-1 truncate">{command.title}</span>
              {shortcut && (
                <kbd
                  className={clsx(
                    "shrink-0 font-sans text-xs",
                    i === selected ? "opacity-80" : "text-fg-faint",
                  )}
                >
                  {formatShortcut(shortcut)}
                </kbd>
              )}
            </li>
          );
        })}
        {matches.length === 0 && (
          <li className="px-2 py-3 text-center text-fg-faint">No matching commands</li>
        )}
      </ul>
    </>
  );
}
