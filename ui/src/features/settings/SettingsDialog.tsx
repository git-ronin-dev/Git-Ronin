import { clsx } from "clsx";

import { useOverlays, type SettingsSection } from "../../app/overlays";
import { Dialog } from "../../ui/Dialog";
import { KeyboardSettings } from "./KeyboardSettings";
import { DiffSettings, GeneralSettings, GitSettings } from "./PreferenceSettings";
import { ProfilesSettings } from "./ProfilesSettings";
import { SshSettings } from "./SshSettings";
import { SyncSettings } from "./SyncSettings";

const SECTIONS: { id: SettingsSection; label: string }[] = [
  { id: "general", label: "General" },
  { id: "diffs", label: "Diffs" },
  { id: "git", label: "Git" },
  { id: "profiles", label: "Profiles" },
  { id: "keyboard", label: "Keyboard" },
  { id: "ssh", label: "SSH keys" },
  { id: "sync", label: "Sync & backup" },
];

export function SettingsDialog() {
  const section = useOverlays((s) => s.settings);
  return (
    <Dialog
      open={section !== null}
      onOpenChange={(open) => !open && useOverlays.setState({ settings: null })}
      title="Settings"
      size="large"
    >
      <div className="flex h-full gap-4">
        <nav aria-label="Settings sections" className="w-40 shrink-0 space-y-0.5">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              type="button"
              aria-current={s.id === section ? "page" : undefined}
              onClick={() => useOverlays.setState({ settings: s.id })}
              className={clsx(
                "flex h-8 w-full items-center rounded-md px-2 text-left",
                s.id === section
                  ? "bg-hover text-fg"
                  : "text-fg-muted hover:bg-hover hover:text-fg",
              )}
            >
              {s.label}
            </button>
          ))}
        </nav>
        <div className="min-w-0 flex-1 overflow-y-auto pr-1">
          {section === "general" && <GeneralSettings />}
          {section === "diffs" && <DiffSettings />}
          {section === "git" && <GitSettings />}
          {section === "profiles" && <ProfilesSettings />}
          {section === "keyboard" && <KeyboardSettings />}
          {section === "ssh" && <SshSettings />}
          {section === "sync" && <SyncSettings />}
        </div>
      </div>
    </Dialog>
  );
}
