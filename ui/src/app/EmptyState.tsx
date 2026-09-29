import { FolderOpen } from "lucide-react";

import { useConfig } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { Button } from "../ui/Button";

export function EmptyState() {
  const { pickAndOpen, open, opening } = useWorkspace();
  const recent = useConfig().data?.local.recentRepos ?? [];

  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 overflow-y-auto p-8 text-center">
      <h1 className="text-2xl font-semibold tracking-tight">Git Ronin</h1>
      <p className="text-fg-muted">Open a repository to get started.</p>
      <Button variant="primary" onClick={() => void pickAndOpen()} disabled={opening}>
        <FolderOpen className="size-4" />
        Open repository…
      </Button>
      <p className="text-xs text-fg-faint">Ctrl+O</p>

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
