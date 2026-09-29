import { Toaster } from "../ui/Toast";
import { TooltipProvider } from "../ui/Tooltip";
import { AppShell } from "./AppShell";
import { GitMissingDialog } from "./GitMissingDialog";

export function App() {
  return (
    <TooltipProvider delayDuration={400}>
      <AppShell />
      <GitMissingDialog />
      <Toaster />
    </TooltipProvider>
  );
}
