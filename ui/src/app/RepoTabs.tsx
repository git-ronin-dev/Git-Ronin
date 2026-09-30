import { clsx } from "clsx";
import { Plus, UserRound, X } from "lucide-react";
import { forwardRef, type ComponentProps } from "react";

import { activeProfileId } from "../features/commands/commands";
import { openDialog } from "../features/ops/dialogs";
import { useConfig } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { DropdownMenu } from "../ui/DropdownMenu";
import { openSettings, useOverlays } from "./overlays";

export function RepoTabs() {
  const { tabs, active, activate, close, pickAndOpen, pickAndInit } = useWorkspace();
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
      <DropdownMenu
        items={[
          { label: "Open repository… (Ctrl+O)", onSelect: () => void pickAndOpen() },
          { label: "Clone…", onSelect: () => openDialog({ kind: "clone" }) },
          { label: "New repository…", onSelect: () => void pickAndInit() },
          "separator",
          { label: "Workspaces…", onSelect: () => useOverlays.setState({ workspaces: true }) },
        ]}
      >
        <button
          type="button"
          aria-label="Open, clone or create a repository"
          className="mb-1 flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-hover hover:text-fg"
        >
          <Plus className="size-4" />
        </button>
      </DropdownMenu>
      <ProfileMenu />
    </div>
  );
}

/** The profile in use; switches to another. */
function ProfileMenu() {
  const config = useConfig().data;
  const switchProfile = useWorkspace((s) => s.switchProfile);
  const profiles = config?.portable.profiles ?? [];
  const active = activeProfileId(config);
  const current = profiles.find((p) => p.id === active);
  if (!current) return null;
  return (
    <DropdownMenu
      align="end"
      items={[
        ...profiles.map((p) => ({
          label: `${p.id === active ? "✓" : "\u2003"} ${p.name}`,
          disabled: p.id === active,
          onSelect: () => void switchProfile(p.id),
        })),
        "separator",
        { label: "Manage profiles…", onSelect: () => openSettings("profiles") },
      ]}
    >
      <ProfileButton name={current.name} />
    </DropdownMenu>
  );
}

const ProfileButton = forwardRef<HTMLButtonElement, ComponentProps<"button"> & { name: string }>(
  function ProfileButton({ name, ...props }, ref) {
    return (
      <button
        ref={ref}
        type="button"
        aria-label={`Profile: ${name}`}
        {...props}
        className="mb-1 ml-auto flex h-7 max-w-40 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs text-fg-muted hover:bg-hover hover:text-fg"
      >
        <UserRound className="size-3.5 shrink-0" />
        <span className="truncate">{name}</span>
      </button>
    );
  },
);
