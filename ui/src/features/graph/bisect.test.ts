import { describe, expect, it } from "vitest";

import { bisectLabels } from "./bisect";

const state = {
  badTerm: "bad",
  goodTerm: "good",
  bad: "b",
  good: ["g1", "g2"],
  skipped: ["s"],
  remaining: 3,
  firstBad: null,
  stuck: false,
};

describe("bisectLabels", () => {
  it("labels good, bad and skipped commits with the bisect's terms", () => {
    const labels = bisectLabels({ ...state, badTerm: "new", goodTerm: "old" });
    expect(labels.get("b")).toEqual({ kind: "bad", label: "new" });
    expect(labels.get("g2")).toEqual({ kind: "good", label: "old" });
    expect(labels.get("s")).toEqual({ kind: "skipped", label: "skipped" });
    expect(labels.has("other")).toBe(false);
  });

  it("puts the first bad commit above its other labels", () => {
    const labels = bisectLabels({ ...state, bad: "f", firstBad: "f" });
    expect(labels.get("f")).toEqual({ kind: "found", label: "first bad" });
  });

  it("has nothing without a bisect", () => {
    expect(bisectLabels(null).size).toBe(0);
  });
});
