import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useCallback, useState } from "react";

import type { FileCommit } from "../../bindings/FileCommit";
import { ipc } from "../../lib/ipc";
import { formatDate, relativeTime } from "../../lib/time";
import { Button } from "../../ui/Button";
import { DiffHeader, DiffViewControls } from "../commit/DiffChrome";
import { DiffView } from "../commit/DiffView";
import { Badge } from "../commit/FileList";
import { ImageDiff } from "../commit/ImageDiff";
import { isImage } from "../commit/lines";
import { useEscape } from "../commit/useEscape";
import { revealCommit } from "../graph/reveal";
import { keys, useUiPrefs } from "../workspace/queries";
import { updateView } from "../workspace/view";

const PAGE = 100;

/** Replaces the graph with the commits that changed a file, and each change. */
export function FileHistoryPanel({ repo, path }: { repo: string; path: string }) {
  const log = useInfiniteQuery({
    queryKey: [...keys.history(repo), "log", path],
    queryFn: ({ pageParam }) => ipc.fileLog(repo, path, null, pageParam, PAGE),
    initialPageParam: 0,
    getNextPageParam: (last, pages) =>
      last.length < PAGE ? undefined : pages.reduce((n, p) => n + p.length, 0),
  });
  const commits = log.data?.pages.flat() ?? [];
  const [picked, setPicked] = useState<string | null>(null);
  const selected = commits.find((c) => c.oid === picked) ?? commits[0];
  const close = useCallback(() => updateView(repo, { openFile: null }), [repo]);
  useEscape(close);

  return (
    <div className="flex h-full flex-col">
      <DiffHeader path={path} oldPath={null} onClose={close}>
        <span className="text-xs text-fg-muted">History</span>
        <Button
          variant="ghost"
          className="h-6 px-2 text-xs"
          onClick={() => updateView(repo, { openFile: { kind: "blame", path, rev: null } })}
        >
          Blame
        </Button>
      </DiffHeader>
      <div className="flex min-h-0 flex-1">
        <ul
          role="listbox"
          aria-label="Commits that changed the file"
          className="w-72 shrink-0 overflow-y-auto border-r border-line"
        >
          {log.isError && <li className="p-3 text-danger select-text">{String(log.error)}</li>}
          {log.data && commits.length === 0 && (
            <li className="p-3 text-fg-faint">No commits changed this file.</li>
          )}
          {commits.map((c) => (
            <li key={c.oid}>
              <button
                type="button"
                role="option"
                aria-selected={c.oid === selected?.oid}
                onClick={() => setPicked(c.oid)}
                onDoubleClick={() => void revealCommit(repo, c.oid)}
                title="Double-click to show it in the graph"
                className={clsx(
                  "flex w-full flex-col gap-0.5 border-b border-line px-3 py-1.5 text-left",
                  c.oid === selected?.oid ? "bg-accent/20" : "hover:bg-hover",
                )}
              >
                <span className="flex w-full items-center gap-2">
                  <Badge status={c.status} />
                  <span className="min-w-0 flex-1 truncate">{c.summary}</span>
                </span>
                <span className="flex w-full gap-2 pl-5 text-xs text-fg-muted">
                  <span className="font-mono text-fg-faint">{c.oid.slice(0, 7)}</span>
                  <span className="min-w-0 flex-1 truncate">{c.authorName}</span>
                  <span className="shrink-0" title={formatDate(c.time)}>
                    {relativeTime(c.time)}
                  </span>
                </span>
                {c.oldPath && (
                  <span
                    className="truncate pl-5 text-xs text-warning"
                    title={`${c.oldPath} → ${c.path}`}
                  >
                    renamed from {c.oldPath}
                  </span>
                )}
              </button>
            </li>
          ))}
          {log.hasNextPage && (
            <li className="p-2 text-center">
              <Button
                variant="ghost"
                className="h-6 px-2 text-xs"
                disabled={log.isFetchingNextPage}
                onClick={() => void log.fetchNextPage()}
              >
                {log.isFetchingNextPage ? "Loading…" : "Load older commits"}
              </Button>
            </li>
          )}
        </ul>
        <div className="flex min-w-0 flex-1 flex-col">
          {selected ? (
            <CommitChange repo={repo} commit={selected} />
          ) : (
            <p className="p-8 text-center text-fg-faint">{log.isPending ? "Loading…" : ""}</p>
          )}
        </div>
      </div>
    </div>
  );
}

/** The file's change in one commit. */
function CommitChange({ repo, commit }: { repo: string; commit: FileCommit }) {
  const prefs = useUiPrefs();
  const ignoreWhitespace = prefs?.ignoreWhitespace ?? false;
  const base = commit.parents[0] ?? null;
  const image = isImage(commit.path);
  const diff = useQuery({
    queryKey: [repo, "diff", base, commit.oid, commit.path, commit.oldPath, ignoreWhitespace],
    queryFn: () =>
      ipc.fileDiff({
        repo,
        base,
        target: commit.oid,
        path: commit.path,
        oldPath: commit.oldPath,
        options: { ignoreWhitespace, contextLines: 3 },
      }),
    enabled: !image,
    staleTime: Infinity,
  });
  return (
    <>
      <div className="shrink-0 border-b border-line px-3 py-1.5">
        <div className="flex items-center gap-2">
          <span className="shrink-0 font-mono text-xs text-fg-faint">{commit.oid.slice(0, 7)}</span>
          <span className="min-w-0 flex-1 truncate font-medium" title={commit.summary}>
            {commit.summary}
          </span>
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-1 whitespace-nowrap">
          <Button
            variant="ghost"
            className="h-6 px-2 text-xs"
            onClick={() =>
              updateView(repo, { openFile: { kind: "blame", path: commit.path, rev: commit.oid } })
            }
          >
            Blame here
          </Button>
          <Button
            variant="ghost"
            className="h-6 px-2 text-xs"
            onClick={() => void revealCommit(repo, commit.oid)}
          >
            Show in graph
          </Button>
          <span className="ml-auto flex items-center gap-2">{!image && <DiffViewControls />}</span>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {image ? (
          <ImageDiff
            repo={repo}
            before={
              base && commit.status !== "added"
                ? { kind: "commit", oid: base, path: commit.oldPath ?? commit.path }
                : null
            }
            after={
              commit.status === "deleted"
                ? null
                : { kind: "commit", oid: commit.oid, path: commit.path }
            }
          />
        ) : diff.isError ? (
          <p className="p-8 text-center text-danger select-text">{String(diff.error)}</p>
        ) : diff.data ? (
          <DiffView diff={diff.data} mode={prefs?.diffView ?? "unified"} path={commit.path} />
        ) : (
          <p className="p-8 text-center text-fg-faint">Loading…</p>
        )}
      </div>
    </>
  );
}
