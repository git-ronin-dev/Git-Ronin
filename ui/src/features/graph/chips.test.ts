import { describe, expect, it } from "vitest";

import type { RefLabel } from "../../bindings/RefLabel";
import { row } from "../../test/fixtures";
import { rowStash, toChips } from "./chips";

const label = (kind: RefLabel["kind"], name: string, extra: Partial<RefLabel> = {}): RefLabel => ({
  kind,
  name,
  fullName: `refs/x/${name}`,
  current: false,
  remote: null,
  ...extra,
});

describe("toChips", () => {
  it("merges a local branch with its same-named remote branch", () => {
    const chips = toChips([
      label("remote", "origin/feature/x", { remote: "origin" }),
      label("local", "feature/x"),
    ]);
    expect(chips).toHaveLength(1);
    expect(chips[0]).toMatchObject({ text: "feature/x", local: true, remote: true });
  });

  it("does not merge on a mere suffix match", () => {
    const chips = toChips([
      label("remote", "origin/feature/x", { remote: "origin" }),
      label("local", "x"),
    ]);
    expect(chips.map((c) => c.text)).toEqual(["x", "origin/feature/x"]);
  });

  it("puts the current branch first and tags last", () => {
    const chips = toChips([
      label("tag", "v1"),
      label("local", "dev"),
      label("local", "main", { current: true }),
    ]);
    expect(chips.map((c) => c.text)).toEqual(["main", "dev", "v1"]);
  });

  it("shows a stash as its own chip, after branches", () => {
    const chips = toChips([label("stash", "stash@{1}"), label("local", "dev")]);
    expect(chips.map((c) => [c.text, c.stash])).toEqual([
      ["dev", false],
      ["stash@{1}", true],
    ]);
  });
});

describe("rowStash", () => {
  it("reads the stash entry from a row's label", () => {
    const stash = { ...row("s", "On main: work"), refs: [label("stash", "stash@{2}")] };
    expect(rowStash(stash)).toEqual({ index: 2, oid: stash.oid, message: "On main: work" });
    expect(rowStash(row("c", "commit"))).toBeNull();
  });
});
