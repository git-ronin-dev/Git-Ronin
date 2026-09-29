import { describe, expect, it } from "vitest";

import { relativeTime } from "./time";

describe("relativeTime", () => {
  const now = 1_700_000_000;

  it("picks the largest fitting unit", () => {
    expect(relativeTime(now - 30, now)).toMatch(/now|second/);
    expect(relativeTime(now - 5 * 60, now)).toMatch(/5 minutes ago/);
    expect(relativeTime(now - 3 * 3600, now)).toMatch(/3 hours ago/);
    expect(relativeTime(now - 2 * 86400, now)).toMatch(/2 days ago/);
    expect(relativeTime(now - 400 * 86400, now)).toMatch(/last year|1 year ago/);
  });
});
