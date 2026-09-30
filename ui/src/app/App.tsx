import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { CommandPalette } from "../features/commands/CommandPalette";
import { LaunchpadDialog } from "../features/hosting/LaunchpadDialog";
import { useShortcuts } from "../features/commands/useShortcuts";
import { CredentialDialog } from "../features/ops/CredentialDialog";
import { Dialogs } from "../features/ops/Dialogs";
import { DragLayer } from "../features/ops/DragLayer";
import { useTasks } from "../features/ops/tasks";
import { useAutoFetch } from "../features/ops/useAutoFetch";
import { ImportDialog } from "../features/settings/ImportDialog";
import { syncKeys, useSyncStatus } from "../features/settings/queries";
import { SettingsDialog } from "../features/settings/SettingsDialog";
import { StashDialog } from "../features/stash/StashDialog";
import { useUpdateCheck } from "../features/updates/useUpdateCheck";
import {
  invalidateRepo,
  invalidateWorktree,
  keys,
  useUiPrefs,
} from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { WorkspacesDialog } from "../features/workspaces/WorkspacesDialog";
import {
  ipc,
  onConfigChanged,
  onOpenPaths,
  onProgress,
  onRepoChanged,
  onSyncChanged,
  onWorktreeChanged,
} from "../lib/ipc";
import { setSoundsEnabled } from "../lib/sound";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { Toaster } from "../ui/Toast";
import { toast } from "../ui/toast-store";
import { TooltipProvider } from "../ui/Tooltip";
import { AppShell } from "./AppShell";
import { GitMissingDialog } from "./GitMissingDialog";
import { useOverlays } from "./overlays";
import { applyTheme } from "./theme";

export function App() {
  const client = useQueryClient();
  const prefs = useUiPrefs();
  const theme = prefs?.theme ?? "system";
  const sounds = prefs?.sounds ?? false;

  useEffect(() => applyTheme(theme), [theme]);
  useEffect(() => setSoundsEnabled(sounds), [sounds]);
  useShortcuts();
  useAutoFetch();
  useSyncConflictNotice();
  useUpdateCheck();

  useEffect(() => {
    void useWorkspace.getState().restore();
    showWarnings();
  }, []);

  useEffect(() => {
    // Repository data belongs to the profile it was loaded under.
    useWorkspace.setState({
      onProfileSwitch: () => {
        client.removeQueries({ predicate: (q) => q.queryKey[0] !== keys.config[0] });
        void client.invalidateQueries({ queryKey: keys.config });
      },
    });
    const unlisten = [
      onRepoChanged((repo) => void invalidateRepo(client, repo)),
      onWorktreeChanged((repo) => void invalidateWorktree(client, repo)),
      onProgress((p) => useTasks.getState().progress(p.key, p.message, p.percent)),
      onConfigChanged(() => {
        void client.invalidateQueries({ queryKey: keys.config });
        showWarnings();
      }),
      onSyncChanged(() => void client.invalidateQueries({ queryKey: syncKeys.status })),
      onOpenPaths((paths) => void useWorkspace.getState().openAll(paths)),
    ];
    return () => unlisten.forEach((u) => void u.then((stop) => stop()));
  }, [client]);

  return (
    <TooltipProvider delayDuration={400}>
      <AppShell />
      <GitMissingDialog />
      <Dialogs />
      <CredentialDialog />
      <ConfirmDialog />
      <DragLayer />
      <CommandPalette />
      <SettingsDialog />
      <ImportDialog />
      <WorkspacesDialog />
      <LaunchpadDialog />
      <GlobalStashDialog />
      <Toaster />
    </TooltipProvider>
  );
}

/** Says when a settings sync needs the user to choose between two machines. */
function useSyncConflictNotice() {
  const conflicts = useSyncStatus().data?.conflicts.length ?? 0;
  useEffect(() => {
    if (conflicts > 0)
      toast.info(
        "Settings changed on two machines",
        "Choose which to keep in Settings → Sync & backup.",
      );
  }, [conflicts]);
}

function showWarnings() {
  void ipc.configTakeWarnings().then((warnings) => {
    for (const w of warnings) toast.error("Settings problem", w);
  });
}

/** The stash dialog, for whichever repository asked. */
function GlobalStashDialog() {
  const repo = useOverlays((s) => s.stash);
  return repo ? (
    <StashDialog
      key={repo}
      repo={repo}
      open
      onOpenChange={(open) => !open && useOverlays.setState({ stash: null })}
    />
  ) : null;
}
