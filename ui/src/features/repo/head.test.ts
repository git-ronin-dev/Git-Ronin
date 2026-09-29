import { describe, expect, it } from "vitest";

import { describeHead } from "./head";

describe("describeHead", () => {
  it("names the current branch", () => {
    expect(describeHead({ kind: "branch", name: "main", unborn: false })).toBe("main");
  });

  it("flags branches without commits", () => {
    expect(describeHead({ kind: "branch", name: "main", unborn: true })).toBe(
      "main (no commits yet)",
    );
  });

  it("shortens detached commit ids", () => {
    expect(describeHead({ kind: "detached", oid: "0123456789abcdef" })).toBe("detached at 0123456");
  });
});
