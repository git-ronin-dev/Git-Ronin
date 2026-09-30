import type { MergeChunk } from "../../bindings/MergeChunk";

export type Side = "ours" | "base" | "theirs";

/**
 * What to keep of each chunk, by chunk index: the sides taken, in the
 * order they were picked. Resolved chunks are always kept as they are; a
 * conflict with nothing taken is unresolved.
 */
export type Picks = Record<number, Side[]>;

type ConflictChunk = Extract<MergeChunk, { kind: "conflict" }>;

export function isConflict(chunk: MergeChunk): chunk is ConflictChunk {
  return chunk.kind === "conflict";
}

/** Takes `side` of a conflict, or drops it again if it was taken. */
export function togglePick(picks: Picks, index: number, side: Side): Picks {
  const current = picks[index] ?? [];
  const next = current.includes(side) ? current.filter((s) => s !== side) : [...current, side];
  return { ...picks, [index]: next };
}

/** Takes `side` of every conflict, replacing earlier picks. */
export function pickAll(chunks: MergeChunk[], side: Side): Picks {
  const picks: Picks = {};
  chunks.forEach((chunk, i) => {
    if (isConflict(chunk)) picks[i] = [side];
  });
  return picks;
}

export function unresolvedCount(chunks: MergeChunk[], picks: Picks): number {
  return chunks.filter((c, i) => isConflict(c) && !picks[i]?.length).length;
}

const MARKER = 7;

/**
 * The merged file: resolved chunks as they are, conflicts as the sides
 * picked (in order), and unresolved conflicts between the usual markers.
 */
export function buildOutput(
  chunks: MergeChunk[],
  picks: Picks,
  labels: { ours: string; theirs: string },
): string {
  return chunks
    .map((chunk, i) => {
      if (!isConflict(chunk)) return chunk.text;
      const taken = picks[i] ?? [];
      if (taken.length > 0) return taken.map((side) => chunk[side]).join("");
      const eol = /\r\n/.test(chunk.ours + chunk.theirs) ? "\r\n" : "\n";
      const line = (text: string) => (text && !text.endsWith("\n") ? text + eol : text);
      return (
        `${"<".repeat(MARKER)} ${labels.ours}${eol}${line(chunk.ours)}` +
        `${"=".repeat(MARKER)}${eol}${line(chunk.theirs)}` +
        `${">".repeat(MARKER)} ${labels.theirs}${eol}`
      );
    })
    .join("");
}

/** Whether `text` still has conflict markers in it. */
export function hasMarkers(text: string): boolean {
  return /^(<{7}|>{7})( |\r?$)/m.test(text);
}

/** The file's line ending, judging by its chunks: CRLF if most lines use it. */
export function lineEnding(chunks: MergeChunk[]): "\n" | "\r\n" {
  let crlf = 0;
  let lf = 0;
  for (const chunk of chunks) {
    const texts = isConflict(chunk) ? [chunk.ours, chunk.base, chunk.theirs] : [chunk.text];
    for (const text of texts) {
      const all = text.match(/\n/g)?.length ?? 0;
      const withCr = text.match(/\r\n/g)?.length ?? 0;
      crlf += withCr;
      lf += all - withCr;
    }
  }
  return crlf > lf ? "\r\n" : "\n";
}

/** Text typed in the editor, which only knows `\n`, with the file's line endings. */
export function withLineEnding(text: string, eol: "\n" | "\r\n"): string {
  const normal = text.replace(/\r\n/g, "\n");
  return eol === "\n" ? normal : normal.replace(/\n/g, "\r\n");
}

/** Lines of a chunk's text for display, without line endings. */
export function displayLines(text: string): string[] {
  if (!text) return [];
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines.map((l) => (l.endsWith("\r") ? l.slice(0, -1) : l));
}
