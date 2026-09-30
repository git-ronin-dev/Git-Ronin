import { useState } from "react";

import type { Theme } from "../../bindings/Theme";
import type { UiPrefs } from "../../bindings/UiPrefs";
import { Button } from "../../ui/Button";
import { Checkbox, Field, Select, TextInput } from "../../ui/Field";
import { useConfig, useSetUiPrefs, useUiPrefs } from "../workspace/queries";
import { useSetGitPrefs, useSetTerminalShell } from "./queries";
import { SettingsPage } from "./SettingsPage";

function useUi(): [UiPrefs | undefined, (patch: Partial<UiPrefs>) => void] {
  const prefs = useUiPrefs();
  const save = useSetUiPrefs();
  return [prefs, (patch) => prefs && save.mutate({ ...prefs, ...patch })];
}

export function GeneralSettings() {
  const [prefs, set] = useUi();
  const shell = useConfig().data?.local.terminalShell ?? "";
  const saveShell = useSetTerminalShell();
  if (!prefs) return null;
  return (
    <SettingsPage title="General">
      <Field label="Theme">
        <Select value={prefs.theme} onChange={(e) => set({ theme: e.target.value as Theme })}>
          <option value="system">Same as the system</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </Select>
      </Field>
      <div className="space-y-1">
        <Checkbox
          label="Show author avatars"
          checked={prefs.showAvatars}
          onChange={(showAvatars) => set({ showAvatars })}
        />
        <p className="pl-6 text-xs text-fg-faint">
          Pictures come from Gravatar, which is sent a hash of each author's email address.
          Otherwise authors show as initials.
        </p>
      </div>
      <Field label="Graph column" hint="Or drag the edge of the graph's column header.">
        <div className="flex items-center gap-2">
          <span className="text-fg">
            {prefs.graphWidth > 0 ? `${prefs.graphWidth} px wide` : "Fits the branch lanes"}
          </span>
          {prefs.graphWidth > 0 && (
            <Button variant="ghost" onClick={() => set({ graphWidth: 0 })}>
              Fit the lanes
            </Button>
          )}
        </div>
      </Field>
      <Field label="Terminal font size">
        <CommitInput
          key={prefs.terminalFontSize}
          value={String(prefs.terminalFontSize)}
          inputMode="numeric"
          className="w-24"
          validate={(v) => /^\d{1,2}$/.test(v.trim()) && Number(v) >= 8 && Number(v) <= 32}
          onCommit={(v) => set({ terminalFontSize: Number(v.trim()) })}
        />
      </Field>
      <Field
        label="Terminal shell"
        hint="The program the terminal panel runs. Empty uses your login shell (PowerShell on Windows). Kept on this machine only."
      >
        <CommitInput
          key={shell}
          value={shell}
          placeholder="Default shell"
          onCommit={(value) => saveShell.mutate(value.trim())}
        />
      </Field>
    </SettingsPage>
  );
}

export function DiffSettings() {
  const [prefs, set] = useUi();
  if (!prefs) return null;
  return (
    <SettingsPage title="Diffs">
      <Field label="Diff layout" hint="Also switchable from any diff's header.">
        <Select
          value={prefs.diffView}
          onChange={(e) => set({ diffView: e.target.value as UiPrefs["diffView"] })}
        >
          <option value="unified">Unified</option>
          <option value="split">Side by side</option>
        </Select>
      </Field>
      <div className="space-y-1">
        <Checkbox
          label="Show changed files as a folder tree"
          checked={prefs.fileTree}
          onChange={(fileTree) => set({ fileTree })}
        />
        <p className="pl-6 text-xs text-fg-faint">
          Folders can be collapsed, and staged or unstaged as a whole. Also switchable above each
          file list.
        </p>
      </div>
      <div className="space-y-1">
        <Checkbox
          label="Ignore whitespace changes"
          checked={prefs.ignoreWhitespace}
          onChange={(ignoreWhitespace) => set({ ignoreWhitespace })}
        />
        <p className="pl-6 text-xs text-fg-faint">
          Applies to diffs and blame. Staging single lines is unavailable while it is on.
        </p>
      </div>
    </SettingsPage>
  );
}

export function GitSettings() {
  const git = useConfig().data?.portable.git;
  const save = useSetGitPrefs();
  if (!git) return null;
  return (
    <SettingsPage title="Git">
      <Field
        label="Fetch automatically every (minutes)"
        hint="Fetches all remotes of each open repository in the background, without asking for passwords. 0 turns it off."
      >
        <CommitInput
          key={git.autoFetchMinutes}
          value={String(git.autoFetchMinutes)}
          inputMode="numeric"
          className="w-24"
          validate={(v) => /^\d{1,4}$/.test(v.trim())}
          onCommit={(v) => save.mutate({ ...git, autoFetchMinutes: Number(v.trim()) })}
        />
      </Field>
    </SettingsPage>
  );
}

/** A text field saved when it loses focus or Enter is pressed. */
export function CommitInput({
  value,
  onCommit,
  validate = () => true,
  ...props
}: {
  value: string;
  onCommit: (value: string) => void;
  validate?: (value: string) => boolean;
  placeholder?: string;
  className?: string;
  inputMode?: "numeric" | "text";
  "aria-label"?: string;
}) {
  const [draft, setDraft] = useState(value);
  const valid = validate(draft);
  const commit = () => {
    if (!valid) setDraft(value);
    else if (draft !== value) onCommit(draft);
  };
  return (
    <TextInput
      {...props}
      value={draft}
      aria-invalid={!valid}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
  );
}
