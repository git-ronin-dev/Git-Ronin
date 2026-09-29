import { describe, expect, it } from "vitest";

import type { DiffLine } from "../../bindings/DiffLine";
import { highlightHunk, highlightLines, loadParser } from "./highlight";

const line = (kind: DiffLine["kind"], text: string): DiffLine => ({
  kind,
  text,
  oldLine: null,
  newLine: null,
  noNewline: false,
});

describe("highlighting", () => {
  it("knows languages by file name and not unknown ones", async () => {
    expect(await loadParser("src/main.ts")).not.toBeNull();
    expect(await loadParser("notes.unknown-ext")).toBeNull();
  });

  it("keeps each line's text intact and marks tokens", async () => {
    const parser = (await loadParser("a.js"))!;
    const lines = ["const x = 1; // one", "", "  return 'two'"];
    const tokens = highlightLines(parser, lines);
    expect(tokens.map((t) => t.map((tok) => tok.text).join(""))).toEqual(lines);
    const classOf = (i: number, text: string) => tokens[i]!.find((t) => t.text === text)?.className;
    expect(classOf(0, "const")).toContain("tok-keyword");
    expect(classOf(0, "1")).toContain("tok-number");
    expect(classOf(0, "// one")).toContain("tok-comment");
  });

  it("colours lines inside a multi-line comment", async () => {
    const parser = (await loadParser("a.js"))!;
    const tokens = highlightLines(parser, ["/* start", "middle", "end */ let y"]);
    expect(tokens[1]![0]).toMatchObject({ text: "middle", className: "tok-comment" });
  });

  it("highlights both sides of a hunk", async () => {
    const parser = (await loadParser("a.js"))!;
    const removed = line("removed", "let a = 'old'");
    const added = line("added", "let a = 2");
    const tokens = highlightHunk(parser, [line("context", "// x"), removed, added]);
    expect(tokens.get(removed)!.some((t) => t.className.includes("tok-string"))).toBe(true);
    expect(tokens.get(added)!.some((t) => t.className.includes("tok-number"))).toBe(true);
  });
});
