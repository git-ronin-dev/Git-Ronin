import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { CredentialDialog } from "../features/ops/CredentialDialog";
import { Dialogs } from "../features/ops/Dialogs";
import { DragLayer } from "../features/ops/DragLayer";
import { useTasks } from "../features/ops/tasks";
import { useAutoFetch } from "../features/ops/useAutoFetch";
import { invalidateRepo, invalidateWorktree, useUiPrefs } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { ipc, onProgress, onRepoChanged, onWorktreeChanged } from "../lib/ipc";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { Toaster } from "../ui/Toast";
import { toast } from "../ui/toast-store";
import { TooltipProvider } from "../ui/Tooltip";
import { AppShell } from "./AppShell";
import { GitMissingDialog } from "./GitMissingDialog";
import { applyTheme } from "./theme";
import { useHotkeys } from "./useHotkeys";

export function App() {
  const client = useQueryClient();
  const theme = useUiPrefs()?.theme ?? "system";

  useEffect(() => applyTheme(theme), [theme]);
  useHotkeys();
  useAutoFetch();

  useEffect(() => {
    void useWorkspace.getState().restore();
    void ipc.configTakeWarnings().then((warnings) => {
      for (const w of warnings) toast.error("Settings problem", w);
    });
  }, []);

  useEffect(() => {
    const unlisten = [
      onRepoChanged((repo) => void invalidateRepo(client, repo)),
      onWorktreeChanged((repo) => void invalidateWorktree(client, repo)),
      onProgress((p) => useTasks.getState().progress(p.key, p.message, p.percent)),
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
      <Toaster />
    </TooltipProvider>
  );
}
