import type { DiffLine } from "../../bindings/DiffLine";
import type { Hunk } from "../../bindings/Hunk";
import type { LineSelection } from "../../bindings/LineSelection";

/** Where each line sits in a diff: hunk index and line index. */
export type LineIndex = Map<DiffLine, [hunk: number, line: number]>;

export function indexLines(hunks: Hunk[]): LineIndex {
  const index: LineIndex = new Map();
  hunks.forEach((hunk, h) => hunk.lines.forEach((line, l) => index.set(line, [h, l])));
  return index;
}

export const isChange = (line: DiffLine) => line.kind !== "context";

/**
 * Selection after clicking `line`: toggles it, or with `extend` adds the
 * changed lines between `anchor` and it (within one hunk).
 */
export function pickLine(
  hunks: Hunk[],
  index: LineIndex,
  selected: ReadonlySet<DiffLine>,
  anchor: DiffLine | null,
  line: DiffLine,
  extend: boolean,
): Set<DiffLine> {
  const next = new Set(selected);
  const at = index.get(line);
  const from = anchor ? index.get(anchor) : undefined;
  if (!at || !isChange(line)) return next;
  if (extend && from && from[0] === at[0]) {
    const [lo, hi] = from[1] < at[1] ? [from[1], at[1]] : [at[1], from[1]];
    for (const l of hunks[at[0]]!.lines.slice(lo, hi + 1)) if (isChange(l)) next.add(l);
  } else if (!next.delete(line)) {
    next.add(line);
  }
  return next;
}

/** The selected lines of hunk `hunk`, or all of it when none are selected. */
export function hunkSelection(
  hunks: Hunk[],
  index: LineIndex,
  selected: ReadonlySet<DiffLine>,
  hunk: number,
): LineSelection {
  const lines = hunks[hunk]!.lines;
  const chosen = lines.filter((l) => selected.has(l));
  const picked = chosen.length > 0 ? chosen : lines;
  return { hunk, lines: picked.map((l) => index.get(l)![1]) };
}

/** Number of selected lines in hunk `hunk`. */
export function selectedIn(hunk: Hunk, selected: ReadonlySet<DiffLine>): number {
  return hunk.lines.reduce((n, l) => n + Number(selected.has(l)), 0);
}
