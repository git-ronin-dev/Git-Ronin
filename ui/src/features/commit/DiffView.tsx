import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { Fragment, useMemo, useState } from "react";

import type { DiffLine } from "../../bindings/DiffLine";
import type { DiffView as Mode } from "../../bindings/DiffView";
import type { FileDiff } from "../../bindings/FileDiff";
import type { LineSelection } from "../../bindings/LineSelection";
import type { PatchTarget } from "../../bindings/PatchTarget";
import { Button } from "../../ui/Button";
import { hunkSelection, indexLines, isChange, pickLine, selectedIn } from "../changes/selection";
import type { Token } from "./highlight";
import { displayText, limitLines, pairLines } from "./lines";

type Tokens = Map<DiffLine, Token[]> | null;

/** Hunk and line actions for an uncommitted diff. */
export interface Staging {
  actions: { verb: string; target: PatchTarget }[];
  onApply: (target: PatchTarget, selection: LineSelection[]) => void;
  busy: boolean;
}

/** Clicking changed lines selects them for staging. */
interface Picking {
  selected: ReadonlySet<DiffLine>;
  onPick: (line: DiffLine, extend: boolean) => void;
}

const NONE: ReadonlySet<DiffLine> = new Set();

/** Rendering tens of thousands of lines at once stalls the webview. */
const LINE_LIMIT = 3000;

const lineBg = {
  context: "",
  added: "bg-success/12",
  removed: "bg-danger/12",
} as const;

const signs = { context: " ", added: "+", removed: "−" } as const;

interface DiffViewProps {
  diff: FileDiff;
  mode: Mode;
  path: string;
  staging?: Staging;
}

export function DiffView({ diff, mode, path, staging }: DiffViewProps) {
  const total = diff.hunks.reduce((n, h) => n + h.lines.length, 0);
  const [showAll, setShowAll] = useState(false);
  const hunks = useMemo(
    () => (showAll ? diff.hunks : limitLines(diff.hunks, LINE_LIMIT)),
    [diff, showAll],
  );
  const highlight = useHighlighter(path);
  const tokens: Tokens = useMemo(() => {
    if (!highlight) return null;
    const all = new Map<DiffLine, Token[]>();
    for (const hunk of hunks) {
      for (const [line, t] of highlight(hunk.lines)) all.set(line, t);
    }
    return all;
  }, [highlight, hunks]);

  // The selection belongs to the diff it was made on; a new diff clears it.
  const [picked, setPicked] = useState({
    diff,
    lines: NONE,
    anchor: null as DiffLine | null,
  });
  const selected = picked.diff === diff ? picked.lines : NONE;
  const index = useMemo(() => indexLines(diff.hunks), [diff]);
  const picking: Picking | undefined = staging && {
    selected,
    onPick: (line, extend) => {
      // Dragging to copy text is not a click on a line.
      if (window.getSelection()?.toString()) return;
      const anchor = picked.diff === diff ? picked.anchor : null;
      const lines = pickLine(diff.hunks, index, selected, anchor, line, extend);
      setPicked({ diff, lines, anchor: line });
    },
  };

  if (diff.binary) return <Notice>Binary file not shown.</Notice>;
  if (diff.hunks.length === 0) return <Notice>No content changes.</Notice>;

  return (
    <div className="font-mono text-xs leading-5 select-text [tab-size:4]">
      {hunks.map((hunk, i) => {
        const count = selectedIn(diff.hunks[i]!, selected);
        return (
          <Fragment key={i}>
            <div className="sticky top-0 z-10 flex items-center gap-2 border-y border-line bg-raised px-3 py-0.5 text-fg-muted">
              <span className="min-w-0 flex-1 truncate">{displayText(hunk.header)}</span>
              {staging?.actions.map(({ verb, target }) => (
                <button
                  key={target}
                  type="button"
                  disabled={staging.busy}
                  onClick={() =>
                    staging.onApply(target, [hunkSelection(diff.hunks, index, selected, i)])
                  }
                  className="shrink-0 rounded-sm px-1.5 font-sans text-fg-muted select-none hover:bg-hover hover:text-fg disabled:opacity-40"
                >
                  {verb} {count > 0 ? `${count} ${count === 1 ? "line" : "lines"}` : "hunk"}
                </button>
              ))}
            </div>
            {mode === "split" ? (
              <Split lines={hunk.lines} tokens={tokens} picking={picking} />
            ) : (
              <Unified lines={hunk.lines} tokens={tokens} picking={picking} />
            )}
          </Fragment>
        );
      })}
      {!showAll && total > LINE_LIMIT && (
        <div className="flex items-center justify-center gap-3 p-4 font-sans text-fg-muted select-none">
          Showing {LINE_LIMIT.toLocaleString()} of {total.toLocaleString()} lines.
          <Button onClick={() => setShowAll(true)}>Show all</Button>
        </div>
      )}
    </div>
  );
}

/**
 * A highlighter for the file's language, or null. The highlighting code and
 * each language's parser are loaded on first use, keeping startup lean.
 */
function useHighlighter(path: string) {
  return useQuery({
    queryKey: ["highlighter", path.split("/").pop()],
    queryFn: async () => {
      const { loadParser, highlightHunk } = await import("./highlight");
      const parser = await loadParser(path);
      return parser ? (lines: DiffLine[]) => highlightHunk(parser, lines) : null;
    },
    staleTime: Infinity,
  }).data;
}

/** Background and click handling for a line that may be picked for staging. */
function lineProps(line: DiffLine, picking?: Picking) {
  const pickable = picking && isChange(line);
  return {
    "aria-selected": pickable ? picking.selected.has(line) : undefined,
    onClick: pickable ? (e: React.MouseEvent) => picking.onPick(line, e.shiftKey) : undefined,
    className: clsx(
      pickable && picking.selected.has(line)
        ? "bg-accent/25 shadow-[inset_3px_0_0_var(--rn-accent)]"
        : lineBg[line.kind],
      pickable && "cursor-pointer",
    ),
  };
}

function Unified({
  lines,
  tokens,
  picking,
}: {
  lines: DiffLine[];
  tokens: Tokens;
  picking?: Picking;
}) {
  return lines.map((line, i) => {
    const { className, ...rest } = lineProps(line, picking);
    return (
      <div key={i} {...rest} className={clsx("flex", className)}>
        <Gutter n={line.oldLine} />
        <Gutter n={line.newLine} />
        <Code line={line} tokens={tokens?.get(line)} />
      </div>
    );
  });
}

function Split({
  lines,
  tokens,
  picking,
}: {
  lines: DiffLine[];
  tokens: Tokens;
  picking?: Picking;
}) {
  return pairLines(lines).map((row, i) => (
    <div key={i} className="grid grid-cols-2">
      <Side line={row.left} n={row.left?.oldLine} tokens={tokens} picking={picking} />
      <Side
        line={row.right}
        n={row.right?.newLine}
        tokens={tokens}
        picking={picking}
        className="border-l border-line"
      />
    </div>
  ));
}

function Side({
  line,
  n,
  tokens,
  picking,
  className,
}: {
  line?: DiffLine;
  n?: number | null;
  tokens: Tokens;
  picking?: Picking;
  className?: string;
}) {
  if (!line) {
    return (
      <div className={clsx("flex min-w-0 bg-raised/50", className)}>
        <Gutter n={n ?? null} />
        <span className="flex-1" />
      </div>
    );
  }
  const { className: lineClass, ...rest } = lineProps(line, picking);
  return (
    <div {...rest} className={clsx("flex min-w-0", lineClass, className)}>
      <Gutter n={n ?? null} />
      <Code line={line} tokens={tokens?.get(line)} />
    </div>
  );
}

function Gutter({ n }: { n: number | null }) {
  return <span className="w-12 shrink-0 pr-2 text-right text-fg-faint select-none">{n ?? ""}</span>;
}

function Code({ line, tokens }: { line: DiffLine; tokens?: Token[] }) {
  return (
    <span className="min-w-0 flex-1 pr-3 break-all whitespace-pre-wrap">
      <span className="inline-block w-4 text-fg-faint select-none">{signs[line.kind]}</span>
      {tokens
        ? tokens.map((t, i) =>
            t.className ? (
              <span key={i} className={t.className}>
                {t.text}
              </span>
            ) : (
              t.text
            ),
          )
        : displayText(line.text)}
      {line.noNewline && (
        <span className="ml-2 text-fg-faint select-none" title="No newline at end of file">
          ⏎̸
        </span>
      )}
    </span>
  );
}

function Notice({ children }: { children: React.ReactNode }) {
  return <p className="p-8 text-center text-fg-muted">{children}</p>;
}
