import type { DiffLine } from "../../bindings/DiffLine";
import type { Hunk } from "../../bindings/Hunk";

export interface SplitRow {
  left?: DiffLine;
  right?: DiffLine;
}

/**
 * Lines side by side for split view: context on both sides, and each run of
 * removals paired with the run of additions that follows it.
 */
export function pairLines(lines: DiffLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (line.kind === "context") {
      rows.push({ left: line, right: line });
      i++;
      continue;
    }
    const removed: DiffLine[] = [];
    const added: DiffLine[] = [];
    while (lines[i]?.kind === "removed") removed.push(lines[i++]!);
    while (lines[i]?.kind === "added") added.push(lines[i++]!);
    for (let k = 0; k < Math.max(removed.length, added.length); k++) {
      rows.push({ left: removed[k], right: added[k] });
    }
  }
  return rows;
}

const IMAGE = /\.(png|jpe?g|gif|webp|bmp|ico|svg|avif)$/i;

export function isImage(path: string) {
  return IMAGE.test(path);
}

/** Splits "src/app/main.ts" into ["src/app/", "main.ts"]. */
export function splitPath(path: string): [string, string] {
  const slash = path.lastIndexOf("/");
  return [path.slice(0, slash + 1), path.slice(slash + 1)];
}

/** Keeps the first `limit` lines across hunks, dropping hunks left empty. */
export function limitLines(hunks: Hunk[], limit: number): Hunk[] {
  const result: Hunk[] = [];
  let remaining = limit;
  for (const hunk of hunks) {
    if (remaining <= 0) break;
    const lines = hunk.lines.slice(0, remaining);
    remaining -= lines.length;
    result.push({ ...hunk, lines });
  }
  return result;
}
