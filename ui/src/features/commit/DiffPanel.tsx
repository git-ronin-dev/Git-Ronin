import { useQuery } from "@tanstack/react-query";
import { useCallback } from "react";

import type { FileChange } from "../../bindings/FileChange";
import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { useUiPrefs } from "../workspace/queries";
import { updateView } from "../workspace/view";
import { DiffHeader, DiffViewControls } from "./DiffChrome";
import { DiffView } from "./DiffView";
import { Stats } from "./FileList";
import { ImageDiff } from "./ImageDiff";
import { isImage } from "./lines";
import { useCommitDetail } from "./queries";
import { useEscape } from "./useEscape";

/** Replaces the graph while a file from a commit is open. */
export function DiffPanel({
  repo,
  oid,
  file,
  base: givenBase,
}: {
  repo: string;
  oid: string;
  file: FileChange;
  /** Diff against this commit instead of the first parent. */
  base?: string;
}) {
  const prefs = useUiPrefs();
  const detail = useCommitDetail(repo, givenBase ? null : oid);
  const base = givenBase ?? (detail.data ? (detail.data.parents[0] ?? null) : undefined);
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

  const close = useCallback(() => updateView(repo, { openFile: null }), [repo]);
  useEscape(close);

  return (
    <div className="flex h-full flex-col">
      <DiffHeader path={file.path} oldPath={file.oldPath} onClose={close}>
        <Stats file={file} />
        {file.status !== "deleted" && <FileTools repo={repo} path={file.path} rev={oid} />}
        {!image && <DiffViewControls />}
      </DiffHeader>
      <div className="min-h-0 flex-1 overflow-auto">
        {image ? (
          base !== undefined && (
            <ImageDiff
              repo={repo}
              before={
                base && file.status !== "added"
                  ? { kind: "commit", oid: base, path: file.oldPath ?? file.path }
                  : null
              }
              after={file.status === "deleted" ? null : { kind: "commit", oid, path: file.path }}
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

/** Buttons opening a file's blame (at `rev`, or the working copy) and history. */
export function FileTools({ repo, path, rev }: { repo: string; path: string; rev: string | null }) {
  const small = "h-6 px-2 text-xs";
  return (
    <>
      <Button
        variant="ghost"
        className={small}
        onClick={() => updateView(repo, { openFile: { kind: "blame", path, rev } })}
      >
        Blame
      </Button>
      <Button
        variant="ghost"
        className={small}
        onClick={() => updateView(repo, { openFile: { kind: "history", path } })}
      >
        History
      </Button>
    </>
  );
}
