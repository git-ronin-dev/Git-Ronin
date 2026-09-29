import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { invalidateRepo, useUiPrefs } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { ipc, onRepoChanged } from "../lib/ipc";
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

  useEffect(() => {
    void useWorkspace.getState().restore();
    void ipc.configTakeWarnings().then((warnings) => {
      for (const w of warnings) toast.error("Settings problem", w);
    });
  }, []);

  useEffect(() => {
    const unlisten = onRepoChanged((repo) => void invalidateRepo(client, repo));
    return () => void unlisten.then((stop) => stop());
  }, [client]);

  return (
    <TooltipProvider delayDuration={400}>
      <AppShell />
      <GitMissingDialog />
      <Toaster />
    </TooltipProvider>
  );
}
