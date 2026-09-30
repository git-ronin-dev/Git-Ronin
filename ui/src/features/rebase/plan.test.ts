import { describe, expect, it } from "vitest";

import type { RebaseCommit } from "../../bindings/RebaseCommit";
import { initialRows, moveRow, problem, setAction, toSteps, unchanged } from "./plan";

const commit = (n: number): RebaseCommit => ({
  oid: String(n).repeat(40),
  message: `c${n}\n\nbody ${n}`,
  authorName: "Ronin Test",
  authorEmail: "test@ronin.invalid",
  time: 1_700_000_000 + n,
});
const commits = [commit(1), commit(2), commit(3)];

describe("rebase plan", () => {
  it("shows commits newest first and runs them oldest first", () => {
    const rows = initialRows(commits);
    expect(rows.map((r) => r.commit.oid[0])).toEqual(["3", "2", "1"]);
    expect(unchanged(rows, commits)).toBe(true);
    expect(toSteps(rows).map((s) => s.oid[0])).toEqual(["1", "2", "3"]);
  });

  it("reorders, rewords and squashes", () => {
    let rows = moveRow(initialRows(commits), 0, 2);
    expect(rows.map((r) => r.commit.oid[0])).toEqual(["2", "1", "3"]);
    expect(unchanged(rows, commits)).toBe(false);
    rows = setAction(rows, 1, "reword");
    // A reword starts from the commit's own message.
    expect(rows[1]!.message).toBe("c1\n\nbody 1");
    rows = setAction(rows, 0, "squash");
    expect(problem(rows)).toBeNull();
    expect(toSteps(rows)).toEqual([
      { oid: "3".repeat(40), action: "pick", message: null },
      { oid: "1".repeat(40), action: "reword", message: "c1\n\nbody 1" },
      { oid: "2".repeat(40), action: "squash", message: null },
    ]);
    expect(moveRow(rows, 0, 5)).toBe(rows);
  });

  it("explains plans git would refuse", () => {
    let rows = setAction(initialRows(commits), 2, "fixup");
    expect(problem(rows)).toMatch(/can't be squashed/);
    rows = setAction(rows, 2, "drop");
    rows = setAction(rows, 1, "squash");
    expect(problem(rows)).toMatch(/can't be squashed/);
    rows = setAction(setAction(initialRows(commits), 0, "reword"), 0, "reword");
    rows = rows.map((r, i) => (i === 0 ? { ...r, message: "  " } : r));
    expect(problem(rows)).toMatch(/message/);
    const dropped = initialRows(commits).map((r) => ({ ...r, action: "drop" as const }));
    expect(problem(dropped)).toMatch(/at least one/);
  });
});
