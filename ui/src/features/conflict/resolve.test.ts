import { describe, expect, it } from "vitest";

import type { MergeChunk } from "../../bindings/MergeChunk";
import {
  buildOutput,
  displayLines,
  hasMarkers,
  lineEnding,
  pickAll,
  togglePick,
  unresolvedCount,
  withLineEnding,
} from "./resolve";

const chunks: MergeChunk[] = [
  { kind: "resolved", text: "top\n" },
  { kind: "conflict", ours: "mine\n", base: "orig\n", theirs: "yours\n" },
  { kind: "resolved", text: "middle\n" },
  { kind: "conflict", ours: "a\n", base: "", theirs: "b\n" },
];
const labels = { ours: "main", theirs: "topic" };

describe("conflict resolution", () => {
  it("keeps picks in the order they were made", () => {
    let picks = togglePick({}, 1, "theirs");
    picks = togglePick(picks, 1, "ours");
    expect(picks[1]).toEqual(["theirs", "ours"]);
    picks = togglePick(picks, 1, "theirs");
    expect(picks[1]).toEqual(["ours"]);
  });

  it("builds the output from picks, marking what is unresolved", () => {
    const picks = togglePick(togglePick({}, 1, "ours"), 1, "theirs");
    expect(unresolvedCount(chunks, picks)).toBe(1);
    const output = buildOutput(chunks, picks, labels);
    expect(output).toBe("top\nmine\nyours\nmiddle\n<<<<<<< main\na\n=======\nb\n>>>>>>> topic\n");
    expect(hasMarkers(output)).toBe(true);

    const all = pickAll(chunks, "base");
    expect(unresolvedCount(chunks, all)).toBe(0);
    expect(buildOutput(chunks, all, labels)).toBe("top\norig\nmiddle\n");
    expect(hasMarkers(buildOutput(chunks, all, labels))).toBe(false);
  });

  it("closes unterminated sides before a marker", () => {
    const last: MergeChunk[] = [{ kind: "conflict", ours: "x", base: "", theirs: "y" }];
    expect(buildOutput(last, {}, labels)).toBe("<<<<<<< main\nx\n=======\ny\n>>>>>>> topic\n");
  });

  it("does not take look-alike lines for markers", () => {
    expect(hasMarkers("a\n=======\nb\n<<<<<<<<\n")).toBe(false);
    expect(hasMarkers("a\n>>>>>>> theirs\n")).toBe(true);
    expect(hasMarkers("<<<<<<<\r\n")).toBe(true);
  });

  it("keeps the file's line endings", () => {
    const crlf: MergeChunk[] = [{ kind: "resolved", text: "a\r\nb\r\nc\n" }];
    expect(lineEnding(crlf)).toBe("\r\n");
    expect(lineEnding(chunks)).toBe("\n");
    expect(withLineEnding("a\nb\n", "\r\n")).toBe("a\r\nb\r\n");
    expect(withLineEnding("a\r\nb\n", "\n")).toBe("a\nb\n");
    expect(displayLines("a\r\nb\r\n")).toEqual(["a", "b"]);
    expect(displayLines("")).toEqual([]);
    expect(displayLines("x")).toEqual(["x"]);
  });
});
