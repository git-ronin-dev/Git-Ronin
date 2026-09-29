import { FolderOpen } from "lucide-react";

import { useRepoStore } from "../features/repo/store";
import { Button } from "../ui/Button";

export function EmptyState() {
  const pickAndOpen = useRepoStore((s) => s.pickAndOpen);
  const opening = useRepoStore((s) => s.opening);

  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
      <h1 className="text-2xl font-semibold tracking-tight">Git Ronin</h1>
      <p className="text-fg-muted">Open a repository to get started.</p>
      <Button variant="primary" onClick={pickAndOpen} disabled={opening}>
        <FolderOpen className="size-4" />
        Open repository…
      </Button>
      <p className="text-xs text-fg-faint">Ctrl+O</p>
    </div>
  );
}
