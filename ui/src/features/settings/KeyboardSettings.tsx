import { clsx } from "clsx";
import { useEffect, useMemo, useState } from "react";

import { Button } from "../../ui/Button";
import { TextInput } from "../../ui/Field";
import { toast } from "../../ui/toast-store";
import { COMMANDS, conflictsWith, type Command } from "../commands/commands";
import { fuzzyFilter } from "../commands/fuzzy";
import { eventToShortcut, formatShortcut } from "../commands/keys";
import { useBindings } from "../commands/useShortcuts";
import { useConfig } from "../workspace/queries";
import { useSetKeybindings } from "./queries";
import { SettingsPage } from "./SettingsPage";

/** Keys that make a shortcut on their own; anything else needs Ctrl/Cmd or Alt. */
const STANDALONE = /^F\d+$/;

export function KeyboardSettings() {
  const overrides = useConfig().data?.portable.keybindings ?? {};
  const bindings = useBindings();
  const save = useSetKeybindings();
  const [filter, setFilter] = useState("");
  const [recording, setRecording] = useState<string | null>(null);

  const commands = useMemo(
    () => fuzzyFilter(COMMANDS, filter, (c) => `${c.category}: ${c.title}`),
    [filter],
  );

  const assign = (command: Command, shortcut: string) => {
    const next = { ...overrides, [command.id]: shortcut };
    for (const other of conflictsWith(bindings, command.id, shortcut)) {
      next[other.id] = (bindings.get(other.id) ?? []).filter((k) => k !== shortcut).join(" ");
      toast.info(`${formatShortcut(shortcut)} no longer runs “${other.title}”`);
    }
    save.mutate(next);
  };

  const reset = (command: Command) => {
    const next = { ...overrides };
    delete next[command.id];
    save.mutate(next);
  };

  return (
    <SettingsPage
      title="Keyboard"
      description="Every command is also in the command palette. Shortcuts are synced with your other settings."
    >
      <TextInput
        aria-label="Filter commands"
        placeholder="Filter commands…"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
      />
      <table className="w-full table-fixed border-collapse">
        <colgroup>
          <col />
          <col className="w-48" />
          <col className="w-44" />
        </colgroup>
        <thead className="sr-only">
          <tr>
            <th>Command</th>
            <th>Shortcut</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {commands.map((command) => (
            <Row
              key={command.id}
              command={command}
              keys={bindings.get(command.id) ?? []}
              overridden={command.id in overrides}
              recording={recording === command.id}
              onRecord={() => setRecording(command.id)}
              onRecorded={(shortcut) => {
                setRecording(null);
                if (shortcut) assign(command, shortcut);
              }}
              onRemove={() => save.mutate({ ...overrides, [command.id]: "" })}
              onReset={() => reset(command)}
            />
          ))}
        </tbody>
      </table>
    </SettingsPage>
  );
}

function Row({
  command,
  keys,
  overridden,
  recording,
  onRecord,
  onRecorded,
  onRemove,
  onReset,
}: {
  command: Command;
  keys: string[];
  overridden: boolean;
  recording: boolean;
  onRecord: () => void;
  /** With the new shortcut, or null if recording was cancelled. */
  onRecorded: (shortcut: string | null) => void;
  onRemove: () => void;
  onReset: () => void;
}) {
  const [hint, setHint] = useState<string | null>(null);

  useEffect(() => {
    if (!recording) return;
    // Capturing on window runs before the app's shortcuts and the dialog's Escape.
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (e.key === "Escape" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
        onRecorded(null);
        return;
      }
      const shortcut = eventToShortcut(e);
      if (!shortcut) return;
      const key = shortcut.split("+").at(-1)!;
      const modified = /(^|\+)(Mod|Ctrl|Alt)\+/.test(shortcut);
      if (!modified && !STANDALONE.test(key)) {
        setHint("Add Ctrl, Alt or Cmd, so typing isn't taken over");
        return;
      }
      setHint(null);
      onRecorded(shortcut);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onRecorded]);

  return (
    <tr className="border-b border-line/60">
      <td className="truncate py-1.5 pr-2">
        <span className="text-fg-faint">{command.category}: </span>
        {command.title}
      </td>
      <td className="py-1.5 pr-2">
        {recording ? (
          <span role="status" className="text-accent">
            {hint ?? "Press a shortcut… (Escape cancels)"}
          </span>
        ) : keys.length > 0 ? (
          <span className="flex flex-wrap gap-1">
            {keys.map((k) => (
              <kbd
                key={k}
                className={clsx(
                  "rounded-sm border border-line bg-raised px-1.5 font-sans text-xs",
                  overridden && "border-accent/60",
                )}
              >
                {formatShortcut(k)}
              </kbd>
            ))}
          </span>
        ) : (
          <span className="text-fg-faint">—</span>
        )}
      </td>
      <td className="py-1.5 text-right whitespace-nowrap">
        <Button
          variant="ghost"
          aria-label={`Change shortcut for ${command.title}`}
          onClick={recording ? () => onRecorded(null) : onRecord}
        >
          {recording ? "Cancel" : "Change"}
        </Button>
        {keys.length > 0 && !recording && (
          <Button
            variant="ghost"
            aria-label={`Remove shortcut for ${command.title}`}
            onClick={onRemove}
          >
            Remove
          </Button>
        )}
        {overridden && !recording && (
          <Button
            variant="ghost"
            aria-label={`Reset shortcut for ${command.title}`}
            onClick={onReset}
          >
            Reset
          </Button>
        )}
      </td>
    </tr>
  );
}
