import { describe, expect, it } from "vitest";

import { fuzzyFilter, fuzzyScore } from "./fuzzy";

describe("fuzzy matching", () => {
  it("matches in-order subsequences only", () => {
    expect(fuzzyScore("pll", "Pull")).not.toBeNull();
    expect(fuzzyScore("lup", "Pull")).toBeNull();
    expect(fuzzyScore("", "anything")).toBe(0);
  });

  it("prefers word starts and adjacent letters", () => {
    const items = ["Repository: Revert commit", "Branch: Check out branch", "Tabs: Close tab"];
    expect(fuzzyFilter(items, "cob", (s) => s)[0]).toBe("Branch: Check out branch");
    expect(fuzzyFilter(items, "close", (s) => s)).toEqual(["Tabs: Close tab"]);
    expect(fuzzyFilter(items, " ", (s) => s)).toEqual(items);
  });
});
