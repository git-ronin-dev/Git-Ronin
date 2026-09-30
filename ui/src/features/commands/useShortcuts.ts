import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";

import { useConfig } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { effectiveKeys, shortcutMap } from "./commands";
import { eventToShortcut, formatShortcut, reachesAppFromTerminal } from "./keys";

/** Each command's shortcuts, with the user's overrides applied. */
export function useBindings(): Map<string, string[]> {
  const overrides = useConfig().data?.portable.keybindings;
  return useMemo(() => effectiveKeys(overrides), [overrides]);
}

/** The first shortcut of a command, formatted for display. */
export function useShortcutLabel(id: string): string | undefined {
  const shortcut = useBindings().get(id)?.[0];
  return shortcut && formatShortcut(shortcut);
}

/** Runs the command bound to each key press, app-wide. */
export function useShortcuts() {
  const client = useQueryClient();
  const bindings = useBindings();
  const byShortcut = useMemo(() => shortcutMap(bindings), [bindings]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const shortcut = eventToShortcut(e);
      const command = shortcut && byShortcut.get(shortcut);
      if (!command) return;
      if (inTerminal(e.target) && !reachesAppFromTerminal(shortcut)) return;
      // Text fields keep their own keys (undo, …) unless the command says otherwise.
      if (!command.inInput && isEditing(e.target)) return;
      if (!command.inDialog && dialogOpen()) return;
      e.preventDefault();
      const ctx = { client, repo: useWorkspace.getState().active };
      if (command.enabled?.(ctx) === false) return;
      void command.run(ctx);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [client, byShortcut]);
}

export function isEditing(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && !!target.closest("input, textarea, [contenteditable]");
}

function inTerminal(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && !!target.closest("[data-terminal]");
}

function dialogOpen(): boolean {
  return !!document.querySelector(
    '[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"]',
  );
}
