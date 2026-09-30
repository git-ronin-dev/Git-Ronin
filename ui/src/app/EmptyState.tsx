import { FolderOpen, FolderPlus, Download } from "lucide-react";

import { useShortcutLabel } from "../features/commands/useShortcuts";
import { openDialog } from "../features/ops/dialogs";
import { useConfig } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { Button } from "../ui/Button";

export function EmptyState() {
  const { pickAndOpen, pickAndInit, open, opening } = useWorkspace();
  const recent = useConfig().data?.local.recentRepos ?? [];
  const openKey = useShortcutLabel("repo.open");
  const paletteKey = useShortcutLabel("palette.open");

  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 overflow-y-auto p-8 text-center">
      <h1 className="text-2xl font-semibold tracking-tight">Git Ronin</h1>
      <p className="text-fg-muted">Open a repository to get started.</p>
      <div className="flex flex-wrap justify-center gap-2">
        <Button variant="primary" onClick={() => void pickAndOpen()} disabled={opening}>
          <FolderOpen className="size-4" />
          Open repository…
        </Button>
        <Button onClick={() => openDialog({ kind: "clone" })} disabled={opening}>
          <Download className="size-4" />
          Clone…
        </Button>
        <Button onClick={() => void pickAndInit()} disabled={opening}>
          <FolderPlus className="size-4" />
          New repository…
        </Button>
      </div>
      <p className="text-xs text-fg-faint">
        {openKey && `${openKey} opens a repository · `}
        {paletteKey && `${paletteKey} shows every command`}
      </p>

      {recent.length > 0 && (
        <section className="mt-6 w-full max-w-md text-left">
          <h2 className="mb-2 px-3 text-xs font-semibold tracking-wide text-fg-faint uppercase">
            Recent
          </h2>
          <ul>
            {recent.map((path) => (
              <li key={path}>
                <button
                  type="button"
                  disabled={opening}
                  onClick={() => void open(path)}
                  className="flex w-full flex-col rounded-md px-3 py-1.5 text-left hover:bg-hover"
                >
                  <span>{path.split(/[\\/]/).filter(Boolean).at(-1)}</span>
                  <span className="truncate text-xs text-fg-faint">{path}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
