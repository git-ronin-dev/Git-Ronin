import { describe, expect, it } from "vitest";

import type { DiffLine } from "../../bindings/DiffLine";
import type { Hunk } from "../../bindings/Hunk";
import type { StatusEntry } from "../../bindings/StatusEntry";
import { composeMessage, splitMessage } from "./draft";
import { ignoreChoices } from "./ignore";
import { countChanges, discardMessage, unstagePaths } from "./queries";
import { hunkSelection, indexLines, pickLine, selectedIn } from "./selection";

const entry = (path: string, status: StatusEntry["status"], oldPath?: string): StatusEntry => ({
  path,
  oldPath: oldPath ?? null,
  status,
  submodule: null,
});

describe("commit messages", () => {
  it("joins summary and body with a blank line", () => {
    expect(composeMessage({ summary: "  Fix parser ", body: "" })).toBe("Fix parser\n");
    expect(composeMessage({ summary: "Fix", body: "\n\nWhy it broke.\n\n" })).toBe(
      "Fix\n\nWhy it broke.\n",
    );
    expect(composeMessage({ summary: " ", body: "orphan body" })).toBe("");
  });

  it("splits a message back into summary and body", () => {
    expect(splitMessage("Fix\n\nWhy.\nMore.\n")).toEqual({ summary: "Fix", body: "Why.\nMore." });
    expect(splitMessage("Only a summary\n")).toEqual({ summary: "Only a summary", body: "" });
  });
});

describe("working status helpers", () => {
  it("counts each path once", () => {
    expect(
      countChanges({
        staged: [entry("a", "modified")],
        unstaged: [entry("a", "modified"), entry("b", "untracked")],
        conflicted: [entry("c", "conflicted")],
      }),
    ).toBe(3);
    expect(countChanges(undefined)).toBe(0);
  });

  it("unstages both sides of a rename", () => {
    expect(unstagePaths([entry("new", "renamed", "old"), entry("x", "added")])).toEqual([
      "old",
      "new",
      "x",
    ]);
  });

  it("warns when discarding deletes untracked files", () => {
    expect(discardMessage([entry("a.txt", "modified")])).toBe(
      "Discard all changes to a.txt? This cannot be undone.",
    );
    expect(discardMessage([entry("n.txt", "untracked")])).toBe(
      "Delete the untracked file n.txt? This cannot be undone.",
    );
    expect(discardMessage([entry("a", "modified"), entry("b", "untracked")])).toContain(
      "1 untracked file is deleted.",
    );
  });

  it("offers the ignore patterns that apply", () => {
    expect(ignoreChoices("logs/app.log").map((c) => c.scope)).toEqual([
      "file",
      "extension",
      "folder",
    ]);
    expect(ignoreChoices("Makefile").map((c) => c.scope)).toEqual(["file"]);
    expect(ignoreChoices(".env").map((c) => c.scope)).toEqual(["file"]);
  });
});

describe("line selection", () => {
  const line = (kind: DiffLine["kind"], text: string): DiffLine => ({
    kind,
    text,
    oldLine: null,
    newLine: null,
    noNewline: false,
  });
  const hunk = (lines: DiffLine[]): Hunk => ({
    header: "@@",
    oldStart: 1,
    oldLines: 1,
    newStart: 1,
    newLines: 1,
    lines,
  });
  const hunks = [
    hunk([line("context", "a"), line("removed", "b"), line("added", "B"), line("context", "c")]),
    hunk([line("added", "x"), line("context", "y"), line("added", "z")]),
  ];
  const index = indexLines(hunks);
  const [h0, h1] = hunks as [Hunk, Hunk];

  it("toggles changed lines only", () => {
    let selected = pickLine(hunks, index, new Set(), null, h0.lines[1]!, false);
    expect([...selected]).toEqual([h0.lines[1]]);
    selected = pickLine(hunks, index, selected, null, h0.lines[0]!, false);
    expect(selected.size).toBe(1);
    selected = pickLine(hunks, index, selected, null, h0.lines[1]!, false);
    expect(selected.size).toBe(0);
  });

  it("extends a range within a hunk, skipping context", () => {
    const selected = pickLine(hunks, index, new Set(), h1.lines[0]!, h1.lines[2]!, true);
    expect([...selected]).toEqual([h1.lines[0], h1.lines[2]]);
    // Across hunks, shift-click just toggles.
    const other = pickLine(hunks, index, new Set(), h0.lines[1]!, h1.lines[2]!, true);
    expect([...other]).toEqual([h1.lines[2]]);
  });

  it("acts on the selected lines of a hunk, or all of it", () => {
    const selected = new Set([h1.lines[2]!]);
    expect(hunkSelection(hunks, index, selected, 1)).toEqual({ hunk: 1, lines: [2] });
    expect(hunkSelection(hunks, index, selected, 0)).toEqual({ hunk: 0, lines: [0, 1, 2, 3] });
    expect(selectedIn(h1, selected)).toBe(1);
    expect(selectedIn(h0, selected)).toBe(0);
  });
});
