import { useQuery, useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useCallback, useEffect, useMemo, useState } from "react";

import type { Conflict } from "../../bindings/Conflict";
import type { MergeChunk } from "../../bindings/MergeChunk";
import type { Resolution } from "../../bindings/Resolution";
import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { confirm } from "../../ui/confirm-store";
import { toast } from "../../ui/toast-store";
import { useWorkingStatus } from "../changes/queries";
import { DiffHeader } from "../commit/DiffChrome";
import { useEscape } from "../commit/useEscape";
import { invalidateWorktree } from "../workspace/queries";
import { updateView } from "../workspace/view";
import {
  buildOutput,
  displayLines,
  hasMarkers,
  isConflict,
  lineEnding,
  pickAll,
  togglePick,
  unresolvedCount,
  withLineEnding,
  type Picks,
  type Side,
} from "./resolve";

/** Replaces the graph while a conflicted file is being resolved. */
export function ConflictEditor({ repo, path }: { repo: string; path: string }) {
  const client = useQueryClient();
  const status = useWorkingStatus(repo).data;
  // Not refreshed with the working copy: that would throw away the picks.
  const conflict = useQuery({
    queryKey: [repo, "conflict", path],
    queryFn: () => ipc.conflict(repo, path),
    staleTime: Infinity,
    gcTime: 0,
  });
  const close = useCallback(() => updateView(repo, { openFile: null }), [repo]);

  // Resolved elsewhere (or never conflicted): nothing left to do here.
  const gone = status !== undefined && !status.conflicted.some((e) => e.path === path);
  useEffect(() => {
    if (gone) close();
  }, [gone, close]);

  const resolve = async (resolution: Resolution, what: string) => {
    try {
      await ipc.resolveConflict(repo, path, resolution);
      toast.success(`Resolved ${path}`, what);
      close();
      return true;
    } catch (err) {
      toast.error("Could not resolve the conflict", String(err));
      return false;
    } finally {
      void invalidateWorktree(client, repo);
    }
  };

  return (
    <div className="flex h-full flex-col">
      {conflict.isError ? (
        <>
          <DiffHeader path={path} oldPath={null} onClose={close} />
          <p className="p-8 text-center text-danger select-text">{String(conflict.error)}</p>
        </>
      ) : !conflict.data ? (
        <>
          <DiffHeader path={path} oldPath={null} onClose={close} />
          <p className="p-8 text-center text-fg-faint">Loading…</p>
        </>
      ) : conflict.data.text ? (
        <TextMerge conflict={conflict.data} onClose={close} onResolve={resolve} />
      ) : (
        <WholeFile conflict={conflict.data} onClose={close} onResolve={resolve} />
      )}
      {!conflict.data?.text && <EscapeCloses onClose={close} />}
    </div>
  );
}

type OnResolve = (resolution: Resolution, what: string) => Promise<boolean>;

function EscapeCloses({ onClose }: { onClose: () => void }) {
  useEscape(onClose);
  return null;
}

/** Chunk-by-chunk merge of two text versions, with an editable output. */
function TextMerge({
  conflict,
  onClose,
  onResolve,
}: {
  conflict: Conflict;
  onClose: () => void;
  onResolve: OnResolve;
}) {
  const { chunks } = conflict;
  const labels = useMemo(
    () => ({ ours: conflict.ours.label, theirs: conflict.theirs.label }),
    [conflict],
  );
  const [picks, setPicks] = useState<Picks>({});
  // Set once the output is edited by hand; picks no longer change it then.
  const [custom, setCustom] = useState<string | null>(null);
  const [showBase, setShowBase] = useState(false);
  const [busy, setBusy] = useState(false);
  const eol = useMemo(() => lineEnding(chunks), [chunks]);
  const generated = useMemo(() => buildOutput(chunks, picks, labels), [chunks, picks, labels]);
  const unresolved = unresolvedCount(chunks, picks);
  const conflicts = chunks.filter(isConflict).length;
  const dirty = custom !== null || Object.values(picks).some((p) => p.length > 0);

  // Leaving asks first once there is merge work to lose.
  const leave = useCallback(async () => {
    const ok =
      !dirty ||
      (await confirm({
        title: "Discard the merge",
        message: "Go back without saving? The sides picked and any edits are lost.",
        confirmLabel: "Discard",
        danger: true,
      }));
    if (ok) onClose();
  }, [dirty, onClose]);
  const escape = useCallback(() => void leave(), [leave]);
  useEscape(escape);

  const pick = (index: number, side: Side) => {
    if (custom !== null) return;
    setPicks((p) => togglePick(p, index, side));
  };
  const save = async () => {
    const text = custom !== null ? withLineEnding(custom, eol) : generated;
    if (custom === null && unresolved > 0) return;
    if (hasMarkers(text)) {
      const ok = await confirm({
        title: "Conflict markers left",
        message: "The output still contains conflict markers. Save it like this anyway?",
        confirmLabel: "Save anyway",
        danger: true,
      });
      if (!ok) return;
    }
    setBusy(true);
    const done = await onResolve({ kind: "content", text }, "Saved the merged file.");
    if (!done) setBusy(false);
  };

  const blocker =
    custom === null && unresolved > 0
      ? `Pick a side for ${unresolved} ${unresolved === 1 ? "conflict" : "conflicts"}`
      : null;
  const small = "h-6 px-2 text-xs";

  return (
    <>
      <DiffHeader path={conflict.path} oldPath={null} onClose={escape}>
        <label className="flex items-center gap-1.5 text-xs text-fg-muted">
          <input
            type="checkbox"
            checked={showBase}
            onChange={(e) => setShowBase(e.target.checked)}
          />
          Show base
        </label>
        <Button
          className={small}
          disabled={custom !== null}
          onClick={() => setPicks(pickAll(chunks, "ours"))}
        >
          Take all ours
        </Button>
        <Button
          className={small}
          disabled={custom !== null}
          onClick={() => setPicks(pickAll(chunks, "theirs"))}
        >
          Take all theirs
        </Button>
        <Button
          variant="primary"
          className={small}
          disabled={busy || blocker !== null}
          title={blocker ?? "Save the output and mark the file resolved"}
          onClick={() => void save()}
        >
          Save and mark resolved
        </Button>
      </DiffHeader>
      <div className="flex min-h-0 flex-1 flex-col">
        <div
          className={clsx(
            "grid shrink-0 border-b border-line bg-raised text-xs font-semibold",
            showBase ? "grid-cols-3" : "grid-cols-2",
          )}
        >
          <SideTitle side="ours" label={labels.ours} present />
          {showBase && <SideTitle side="base" label="Common ancestor" present={conflict.hasBase} />}
          <SideTitle side="theirs" label={labels.theirs} present />
        </div>
        <div className="min-h-0 flex-1 overflow-auto font-mono text-xs leading-5 select-text [tab-size:4]">
          {chunks.map((chunk, i) => (
            <ChunkRow
              key={i}
              chunk={chunk}
              picked={picks[i] ?? []}
              showBase={showBase}
              locked={custom !== null}
              onPick={(side) => pick(i, side)}
            />
          ))}
        </div>
        <section
          aria-label="Output"
          className="flex h-[40%] min-h-32 shrink-0 flex-col border-t border-line"
        >
          <div className="flex h-8 shrink-0 items-center gap-2 bg-surface px-3 text-xs">
            <span className="font-semibold">Output</span>
            <span className="text-fg-muted">
              {custom !== null
                ? "Edited by hand"
                : unresolved > 0
                  ? `${unresolved} of ${conflicts} ${conflicts === 1 ? "conflict" : "conflicts"} unresolved`
                  : "All conflicts resolved"}
            </span>
            {custom !== null && (
              <Button
                variant="ghost"
                className="ml-auto h-6 px-2 text-xs"
                onClick={() => setCustom(null)}
              >
                Discard edits
              </Button>
            )}
          </div>
          <textarea
            aria-label="Merged file"
            value={custom ?? generated}
            onChange={(e) => setCustom(e.target.value)}
            spellCheck={false}
            className="min-h-0 flex-1 resize-none bg-canvas p-3 font-mono text-xs leading-5 text-fg outline-none [tab-size:4]"
          />
        </section>
      </div>
    </>
  );
}

const sideTint: Record<Side, string> = {
  ours: "text-accent",
  base: "text-fg-muted",
  theirs: "text-success",
};

function SideTitle({ side, label, present }: { side: Side; label: string; present: boolean }) {
  const name = { ours: "Ours", base: "Base", theirs: "Theirs" }[side];
  return (
    <div className="flex h-7 min-w-0 items-center gap-1.5 border-l border-line px-3 first:border-l-0">
      <span className={sideTint[side]}>{name}</span>
      <span className="truncate font-normal text-fg-muted" title={label}>
        {label}
        {!present && " (no file)"}
      </span>
    </div>
  );
}

/** Unchanged runs longer than this are folded. */
const FOLD = 8;

function ChunkRow({
  chunk,
  picked,
  showBase,
  locked,
  onPick,
}: {
  chunk: MergeChunk;
  picked: Side[];
  showBase: boolean;
  locked: boolean;
  onPick: (side: Side) => void;
}) {
  const [open, setOpen] = useState(false);
  const cols = showBase ? "grid-cols-3" : "grid-cols-2";
  if (!isConflict(chunk)) {
    const lines = displayLines(chunk.text);
    const folded = !open && lines.length > FOLD;
    const shown = folded ? [...lines.slice(0, 3), null, ...lines.slice(-3)] : lines;
    const column = (
      <div className="min-w-0 border-l border-line text-fg-muted first:border-l-0">
        {shown.map((line, k) =>
          line === null ? (
            <button
              key={k}
              type="button"
              onClick={() => setOpen(true)}
              className="block w-full bg-raised/60 px-3 text-left font-sans text-fg-faint select-none hover:text-fg"
            >
              ⋯ {lines.length - 6} unchanged lines
            </button>
          ) : (
            <Line key={k} text={line} />
          ),
        )}
      </div>
    );
    return (
      <div className={clsx("grid", cols)}>
        {column}
        {showBase && column}
        {column}
      </div>
    );
  }
  const sides: Side[] = showBase ? ["ours", "base", "theirs"] : ["ours", "theirs"];
  const unresolved = picked.length === 0;
  return (
    <div className={clsx("grid border-y", cols, unresolved ? "border-warning/60" : "border-line")}>
      {sides.map((side) => {
        const order = picked.indexOf(side);
        const taken = order >= 0;
        const lines = displayLines(chunk[side]);
        return (
          <div
            key={side}
            className={clsx(
              "min-w-0 border-l border-line first:border-l-0",
              taken ? "bg-success/12" : unresolved ? "bg-warning/8" : "opacity-50",
            )}
          >
            <label className="flex h-6 items-center gap-2 bg-raised/70 px-3 font-sans text-fg-muted select-none">
              <input
                type="checkbox"
                checked={taken}
                disabled={locked}
                onChange={() => onPick(side)}
                aria-label={`Take ${side}`}
              />
              <span className={sideTint[side]}>Take {side}</span>
              {picked.length > 1 && taken && (
                <span className="text-fg-faint" title="Order in the output">
                  #{order + 1}
                </span>
              )}
              {lines.length === 0 && <span className="text-fg-faint">(nothing)</span>}
            </label>
            {lines.map((line, k) => (
              <Line key={k} text={line} />
            ))}
          </div>
        );
      })}
    </div>
  );
}

function Line({ text }: { text: string }) {
  return <div className="min-h-5 px-3 break-all whitespace-pre-wrap">{text}</div>;
}

/** Binary, submodule and delete/modify conflicts: one side or the other. */
function WholeFile({
  conflict,
  onClose,
  onResolve,
}: {
  conflict: Conflict;
  onClose: () => void;
  onResolve: OnResolve;
}) {
  const [busy, setBusy] = useState(false);
  const run = async (resolution: Resolution, what: string) => {
    setBusy(true);
    if (!(await onResolve(resolution, what))) setBusy(false);
  };
  const { ours, theirs } = conflict;
  const why = !ours.present
    ? `${ours.label} deleted the file; ${theirs.label} changed it.`
    : !theirs.present
      ? `${ours.label} changed the file; ${theirs.label} deleted it.`
      : "Both sides changed this file, and it can't be merged line by line (it is binary or a submodule).";
  const choice = (side: "ours" | "theirs") => {
    const s = conflict[side];
    return (
      <Button
        disabled={busy}
        onClick={() =>
          void run(
            { kind: side },
            s.present ? `Kept the version from ${s.label}.` : "Deleted the file.",
          )
        }
      >
        {s.present ? `Keep ${s.label}'s version` : `Delete it, like ${s.label}`}
      </Button>
    );
  };
  return (
    <>
      <DiffHeader path={conflict.path} oldPath={null} onClose={onClose} />
      <div className="mx-auto flex max-w-lg flex-col items-center gap-4 p-10 text-center">
        <p className="text-fg-muted">{why}</p>
        <div className="flex flex-wrap justify-center gap-2">
          {choice("ours")}
          {choice("theirs")}
          {ours.present && theirs.present && (
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => void run({ kind: "delete" }, "Deleted the file.")}
            >
              Delete the file
            </Button>
          )}
        </div>
      </div>
    </>
  );
}
