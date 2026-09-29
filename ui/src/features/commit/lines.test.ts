import { describe, expect, it } from "vitest";

import type { DiffLine } from "../../bindings/DiffLine";
import { isImage, limitLines, pairLines, splitPath } from "./lines";

const line = (kind: DiffLine["kind"], text: string): DiffLine => ({
  kind,
  text,
  oldLine: kind === "added" ? null : 1,
  newLine: kind === "removed" ? null : 1,
  noNewline: false,
});

describe("pairLines", () => {
  it("pairs removals with the additions that follow", () => {
    const rows = pairLines([
      line("context", "a"),
      line("removed", "b"),
      line("removed", "c"),
      line("added", "B"),
      line("context", "d"),
      line("added", "e"),
    ]);
    const texts = rows.map((r) => [r.left?.text ?? null, r.right?.text ?? null]);
    expect(texts).toEqual([
      ["a", "a"],
      ["b", "B"],
      ["c", null],
      ["d", "d"],
      [null, "e"],
    ]);
  });
});

describe("helpers", () => {
  it("recognises images", () => {
    expect(isImage("logo.PNG")).toBe(true);
    expect(isImage("icons/a.svg")).toBe(true);
    expect(isImage("main.rs")).toBe(false);
  });

  it("splits paths", () => {
    expect(splitPath("src/app/main.ts")).toEqual(["src/app/", "main.ts"]);
    expect(splitPath("README.md")).toEqual(["", "README.md"]);
  });
});

describe("limitLines", () => {
  const hunk = (n: number) => ({
    header: "@@",
    oldStart: 1,
    oldLines: n,
    newStart: 1,
    newLines: n,
    lines: Array.from({ length: n }, (_, i) => line("context", String(i))),
  });

  it("keeps the first lines across hunks and drops the rest", () => {
    const limited = limitLines([hunk(3), hunk(3), hunk(3)], 4);
    expect(limited.map((h) => h.lines.length)).toEqual([3, 1]);
  });

  it("leaves small diffs untouched", () => {
    expect(limitLines([hunk(2)], 10)[0]!.lines).toHaveLength(2);
  });
});
