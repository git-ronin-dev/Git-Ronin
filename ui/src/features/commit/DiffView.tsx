import { clsx } from "clsx";
import { Fragment, useState } from "react";

import type { DiffLine } from "../../bindings/DiffLine";
import type { DiffView as Mode } from "../../bindings/DiffView";
import type { FileDiff } from "../../bindings/FileDiff";
import { Button } from "../../ui/Button";
import { limitLines, pairLines } from "./lines";

/** Rendering tens of thousands of lines at once stalls the webview. */
const LINE_LIMIT = 3000;

const lineBg = {
  context: "",
  added: "bg-success/12",
  removed: "bg-danger/12",
} as const;

const signs = { context: " ", added: "+", removed: "−" } as const;

export function DiffView({ diff, mode }: { diff: FileDiff; mode: Mode }) {
  const total = diff.hunks.reduce((n, h) => n + h.lines.length, 0);
  const [showAll, setShowAll] = useState(false);

  if (diff.binary) return <Notice>Binary file not shown.</Notice>;
  if (diff.hunks.length === 0) return <Notice>No content changes.</Notice>;

  const hunks = showAll ? diff.hunks : limitLines(diff.hunks, LINE_LIMIT);

  return (
    <div className="font-mono text-xs leading-5 select-text [tab-size:4]">
      {hunks.map((hunk, i) => (
        <Fragment key={i}>
          <div className="sticky top-0 z-10 border-y border-line bg-raised px-3 py-0.5 text-fg-muted">
            {hunk.header}
          </div>
          {mode === "split" ? <Split lines={hunk.lines} /> : <Unified lines={hunk.lines} />}
        </Fragment>
      ))}
      {!showAll && total > LINE_LIMIT && (
        <div className="flex items-center justify-center gap-3 p-4 font-sans text-fg-muted select-none">
          Showing {LINE_LIMIT.toLocaleString()} of {total.toLocaleString()} lines.
          <Button onClick={() => setShowAll(true)}>Show all</Button>
        </div>
      )}
    </div>
  );
}

function Unified({ lines }: { lines: DiffLine[] }) {
  return lines.map((line, i) => (
    <div key={i} className={clsx("flex", lineBg[line.kind])}>
      <Gutter n={line.oldLine} />
      <Gutter n={line.newLine} />
      <Code line={line} />
    </div>
  ));
}

function Split({ lines }: { lines: DiffLine[] }) {
  return pairLines(lines).map((row, i) => (
    <div key={i} className="grid grid-cols-2">
      <Side line={row.left} n={row.left?.oldLine} />
      <Side line={row.right} n={row.right?.newLine} className="border-l border-line" />
    </div>
  ));
}

function Side({ line, n, className }: { line?: DiffLine; n?: number | null; className?: string }) {
  return (
    <div className={clsx("flex min-w-0", line ? lineBg[line.kind] : "bg-raised/50", className)}>
      <Gutter n={n ?? null} />
      {line ? <Code line={line} /> : <span className="flex-1" />}
    </div>
  );
}

function Gutter({ n }: { n: number | null }) {
  return <span className="w-12 shrink-0 pr-2 text-right text-fg-faint select-none">{n ?? ""}</span>;
}

function Code({ line }: { line: DiffLine }) {
  return (
    <span className="min-w-0 flex-1 pr-3 break-all whitespace-pre-wrap">
      <span className="inline-block w-4 text-fg-faint select-none">{signs[line.kind]}</span>
      {line.text}
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
