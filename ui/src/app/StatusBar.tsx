import { useQuery } from "@tanstack/react-query";
import { GitBranch, TriangleAlert } from "lucide-react";

import { describeHead } from "../features/repo/head";
import { useRepoStore } from "../features/repo/store";
import { ipc } from "../lib/ipc";

export function StatusBar() {
  const repo = useRepoStore((s) => s.repo);
  const git = useQuery({ queryKey: ["gitVersion"], queryFn: ipc.gitVersion });

  return (
    <footer className="flex h-6 shrink-0 items-center gap-4 border-t border-line bg-surface px-3 text-xs text-fg-muted">
      {repo && (
        <>
          <span className="flex items-center gap-1">
            <GitBranch className="size-3" />
            {describeHead(repo.head)}
          </span>
          <span className="truncate text-fg-faint select-text">{repo.path}</span>
        </>
      )}
      <span className="ml-auto">
        {git.data && `git ${git.data.major}.${git.data.minor}.${git.data.patch}`}
        {git.isError && (
          <span className="flex items-center gap-1 text-warning">
            <TriangleAlert className="size-3" />
            git unavailable
          </span>
        )}
      </span>
    </footer>
  );
}
