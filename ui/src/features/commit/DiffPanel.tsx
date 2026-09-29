import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { ArrowLeft } from "lucide-react";
import { useEffect } from "react";

import type { FileChange } from "../../bindings/FileChange";
import { ipc } from "../../lib/ipc";
import { useSetUiPrefs, useUiPrefs } from "../workspace/queries";
import { updateView } from "../workspace/view";
import { useCommitDetail } from "./queries";
import { DiffView } from "./DiffView";
import { Stats } from "./FileList";
import { ImageDiff } from "./ImageDiff";
import { isImage, splitPath } from "./lines";

/** Replaces the graph while a file from a commit is open. */
export function DiffPanel({ repo, oid, file }: { repo: string; oid: string; file: FileChange }) {
  const prefs = useUiPrefs();
  const setPrefs = useSetUiPrefs();
  const detail = useCommitDetail(repo, oid);
  const base = detail.data ? (detail.data.parents[0] ?? null) : undefined;
  const image = isImage(file.path);
  const ignoreWhitespace = prefs?.ignoreWhitespace ?? false;
  const mode = prefs?.diffView ?? "unified";

  const diff = useQuery({
    queryKey: [repo, "diff", base, oid, file.path, file.oldPath, ignoreWhitespace],
    queryFn: () =>
      ipc.fileDiff({
        repo,
        base: base ?? null,
        target: oid,
        path: file.path,
        oldPath: file.oldPath,
        options: { ignoreWhitespace, contextLines: 3 },
      }),
    enabled: base !== undefined && !image,
    staleTime: Infinity,
  });

  const close = () => updateView(repo, { openFile: null });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && updateView(repo, { openFile: null });
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [repo]);

  const [dir, name] = splitPath(file.path);
  return (
    <div className="flex h-full flex-col">
      <header className="flex h-10 shrink-0 items-center gap-2 border-b border-line bg-surface px-2">
        <button
          type="button"
          onClick={close}
          aria-label="Back to graph"
          title="Back to graph (Esc)"
          className="flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-hover hover:text-fg"
        >
          <ArrowLeft className="size-4" />
        </button>
        <span className="min-w-0 flex-1 truncate select-text" title={file.path}>
          {file.oldPath && <span className="text-fg-faint">{file.oldPath} → </span>}
          <span className="text-fg-faint">{dir}</span>
          <span className="font-medium">{name}</span>
        </span>
        <Stats file={file} />
        {prefs && !image && (
          <>
            <label className="ml-2 flex items-center gap-1.5 text-xs text-fg-muted">
              <input
                type="checkbox"
                checked={ignoreWhitespace}
                onChange={(e) => setPrefs.mutate({ ...prefs, ignoreWhitespace: e.target.checked })}
              />
              Ignore whitespace
            </label>
            <div className="flex rounded-md border border-line text-xs">
              {(["unified", "split"] as const).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => setPrefs.mutate({ ...prefs, diffView: m })}
                  className={clsx(
                    "h-6 px-2 capitalize first:rounded-l-md last:rounded-r-md",
                    mode === m ? "bg-raised text-fg" : "text-fg-muted hover:text-fg",
                  )}
                >
                  {m}
                </button>
              ))}
            </div>
          </>
        )}
      </header>
      <div className="min-h-0 flex-1 overflow-auto">
        {image ? (
          base !== undefined && (
            <ImageDiff
              repo={repo}
              path={file.path}
              before={
                base && file.status !== "added"
                  ? { oid: base, path: file.oldPath ?? file.path }
                  : null
              }
              after={file.status === "deleted" ? null : oid}
            />
          )
        ) : diff.isError ? (
          <p className="p-8 text-center text-danger select-text">{String(diff.error)}</p>
        ) : diff.data ? (
          <DiffView diff={diff.data} mode={mode} path={file.path} />
        ) : (
          <p className="p-8 text-center text-fg-faint">Loading…</p>
        )}
      </div>
    </div>
  );
}
