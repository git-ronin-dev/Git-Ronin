import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useCallback, useEffect, useState } from "react";

import type { BlobSource } from "../../bindings/BlobSource";
import type { LineSelection } from "../../bindings/LineSelection";
import type { PatchTarget } from "../../bindings/PatchTarget";
import type { StatusEntry } from "../../bindings/StatusEntry";
import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { DiffHeader, DiffViewControls } from "../commit/DiffChrome";
import { DiffView, type Staging } from "../commit/DiffView";
import { ImageDiff, type BlobRef } from "../commit/ImageDiff";
import { isImage } from "../commit/lines";
import { useEscape } from "../commit/useEscape";
import { keys, useUiPrefs } from "../workspace/queries";
import { updateView } from "../workspace/view";
import { useWorkingActions, useWorkingStatus } from "./queries";

interface WorkingDiffPanelProps {
  repo: string;
  path: string;
  /** Show the staged change (HEAD to index) rather than the unstaged one. */
  staged: boolean;
}

/** Replaces the graph while an uncommitted file is open, with staging controls. */
export function WorkingDiffPanel({ repo, path, staged }: WorkingDiffPanelProps) {
  const prefs = useUiPrefs();
  const status = useWorkingStatus(repo).data;
  const actions = useWorkingActions(repo);
  const [busy, setBusy] = useState(false);

  const stagedEntry = status?.staged.find((e) => e.path === path);
  const unstagedEntry =
    status?.unstaged.find((e) => e.path === path) ??
    status?.conflicted.find((e) => e.path === path);
  const entry = staged ? stagedEntry : unstagedEntry;
  const other = staged ? unstagedEntry : stagedEntry;

  const close = useCallback(() => updateView(repo, { openFile: null }), [repo]);
  useEscape(close);

  // Follow the file to the other list when all of it moves there; close it
  // once it has no changes left.
  const gone = status !== undefined && !entry;
  const hasOther = other !== undefined;
  useEffect(() => {
    if (!gone) return;
    updateView(repo, {
      openFile: hasOther ? { kind: "working", path, staged: !staged } : null,
    });
  }, [gone, hasOther, repo, path, staged]);

  const image = isImage(path);
  const ignoreWhitespace = prefs?.ignoreWhitespace ?? false;
  const diff = useQuery({
    queryKey: [
      ...keys.working(repo),
      "diff",
      path,
      staged,
      entry?.status,
      entry?.oldPath,
      ignoreWhitespace,
    ],
    queryFn: () => ipc.workingDiff(repo, entry!, staged, { ignoreWhitespace, contextLines: 3 }),
    enabled: entry !== undefined && !image,
    // While a refreshed diff of the same file loads, keep showing the last one.
    placeholderData: (previous, query) =>
      query?.queryKey[3] === path && query.queryKey[4] === staged ? previous : undefined,
  });

  if (!entry) return null;

  const run = async (action: () => Promise<boolean>) => {
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  };

  // Hunks and lines of plain modifications; whole files otherwise.
  const pickable = entry.status === "modified" && entry.submodule === null && !diff.data?.binary;
  const staging: Staging | undefined =
    pickable && !ignoreWhitespace && diff.data && !diff.isPlaceholderData
      ? {
          actions: staged
            ? [{ verb: "Unstage", target: "unstage" }]
            : [
                { verb: "Discard", target: "discard" },
                { verb: "Stage", target: "stage" },
              ],
          busy,
          onApply: (target: PatchTarget, selection: LineSelection[]) =>
            void run(() => actions.applyLines(path, diff.data.hunks, selection, target)),
        }
      : undefined;

  return (
    <div className="flex h-full flex-col">
      <DiffHeader path={path} oldPath={staged ? entry.oldPath : null} onClose={close}>
        {other && (
          <div className="flex rounded-md border border-line text-xs">
            {[false, true].map((s) => (
              <button
                key={String(s)}
                type="button"
                aria-pressed={staged === s}
                onClick={() => updateView(repo, { openFile: { kind: "working", path, staged: s } })}
                className={clsx(
                  "h-6 px-2 first:rounded-l-md last:rounded-r-md",
                  staged === s ? "bg-raised text-fg" : "text-fg-muted hover:text-fg",
                )}
              >
                {s ? "Staged" : "Unstaged"}
              </button>
            ))}
          </div>
        )}
        <FileActions
          entry={entry}
          staged={staged}
          busy={busy}
          onStage={() => void run(() => actions.stage([entry]))}
          onUnstage={() => void run(() => actions.unstage([entry]))}
          onDiscard={() => void run(() => actions.discard([entry]))}
        />
        {!image && <DiffViewControls />}
      </DiffHeader>
      {pickable && ignoreWhitespace && (
        <p className="shrink-0 border-b border-line bg-raised px-3 py-1 text-xs text-fg-muted">
          Turn off “Ignore whitespace” to stage or discard single hunks and lines.
        </p>
      )}
      <div className="min-h-0 flex-1 overflow-auto">
        {image ? (
          <ImageDiff repo={repo} {...imageVersions(entry, staged)} />
        ) : diff.isError ? (
          <p className="p-8 text-center text-danger select-text">{String(diff.error)}</p>
        ) : diff.data ? (
          <DiffView
            diff={diff.data}
            mode={prefs?.diffView ?? "unified"}
            path={path}
            staging={staging}
          />
        ) : (
          <p className="p-8 text-center text-fg-faint">Loading…</p>
        )}
      </div>
    </div>
  );
}

function FileActions({
  entry,
  staged,
  busy,
  onStage,
  onUnstage,
  onDiscard,
}: {
  entry: StatusEntry;
  staged: boolean;
  busy: boolean;
  onStage: () => void;
  onUnstage: () => void;
  onDiscard: () => void;
}) {
  const small = "h-6 px-2 text-xs";
  if (staged) {
    return (
      <Button className={small} disabled={busy} onClick={onUnstage}>
        Unstage file
      </Button>
    );
  }
  if (entry.status === "conflicted") {
    return (
      <Button className={small} disabled={busy} onClick={onStage}>
        Mark resolved
      </Button>
    );
  }
  return (
    <>
      <Button variant="ghost" className={small} disabled={busy} onClick={onDiscard}>
        Discard file
      </Button>
      <Button className={small} disabled={busy} onClick={onStage}>
        Stage file
      </Button>
    </>
  );
}

/** The two versions an uncommitted image change compares. */
function imageVersions(
  entry: StatusEntry,
  staged: boolean,
): { before: BlobRef | null; after: BlobRef | null } {
  const at = (source: BlobSource, path = entry.path): BlobRef => ({
    kind: "working",
    source,
    path,
  });
  const deleted = entry.status === "deleted";
  return staged
    ? {
        before: entry.status === "added" ? null : at("head", entry.oldPath ?? entry.path),
        after: deleted ? null : at("index"),
      }
    : {
        before: entry.status === "untracked" ? null : at("index"),
        after: deleted ? null : at("worktree"),
      };
}
