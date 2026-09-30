import { useQuery } from "@tanstack/react-query";

import { useTask } from "../features/ops/tasks";
import { UpdateStatus } from "../features/updates/UpdateStatus";
import { describeHead } from "../features/repo/head";
import { useRefs, useRepoInfo } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { ipc } from "../lib/ipc";
import { GitBranch, LoaderCircle, TriangleAlert } from "../ui/icons";

export function StatusBar() {
  const active = useWorkspace((s) => s.active);
  const git = useQuery({ queryKey: ["gitVersion"], queryFn: ipc.gitVersion });

  return (
    <footer className="flex h-6 shrink-0 items-center gap-4 border-t border-line bg-surface px-3 text-xs text-fg-muted">
      {active && <RepoStatus repo={active} />}
      {active && <TaskStatus repo={active} />}
      <span className="ml-auto" />
      <UpdateStatus />
      <span className="shrink-0">
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

function RepoStatus({ repo }: { repo: string }) {
  const info = useRepoInfo(repo).data;
  const current = useRefs(repo).data?.local.find((b) => b.isHead);
  const upstream = current?.upstream;
  if (!info) return null;
  return (
    <>
      <span className="flex shrink-0 items-center gap-1">
        <GitBranch className="size-3" />
        {describeHead(info.head)}
        {upstream && !upstream.gone && (upstream.ahead > 0 || upstream.behind > 0) && (
          <span className="text-fg-faint">
            {upstream.ahead > 0 && ` ↑${upstream.ahead}`}
            {upstream.behind > 0 && ` ↓${upstream.behind}`}
          </span>
        )}
      </span>
      <span className="truncate text-fg-faint select-text">{info.path}</span>
    </>
  );
}

/** The network command running in the repository, with git's progress. */
function TaskStatus({ repo }: { repo: string }) {
  const task = useTask(repo);
  if (!task) return null;
  return (
    <span role="status" className="flex min-w-0 shrink items-center gap-1.5 text-fg">
      <LoaderCircle className="size-3 shrink-0 animate-spin" />
      <span className="shrink-0">{task.label}…</span>
      {task.message && (
        <span className="truncate text-fg-muted">
          {task.message}
          {task.percent !== null && ` ${task.percent}%`}
        </span>
      )}
    </span>
  );
}
