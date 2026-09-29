import { describe, expect, it } from "vitest";

import { edgePath, LANE_WIDTH, ROW_HEIGHT } from "./geometry";

describe("edgePath", () => {
  const x = (lane: number) => LANE_WIDTH / 2 + lane * LANE_WIDTH;

  it("draws pass-through lanes as full-height verticals", () => {
    expect(edgePath({ kind: "pass", from: 2, to: 2 })).toBe(`M${x(2)} 0V${ROW_HEIGHT}`);
  });

  it("joins rows seamlessly: edges start or end on the row borders at lane centres", () => {
    const into = edgePath({ kind: "in", from: 1, to: 0 });
    const outOf = edgePath({ kind: "out", from: 0, to: 1 });
    expect(into.startsWith(`M${x(1)} 0`)).toBe(true);
    expect(outOf.endsWith(`${x(1)} ${ROW_HEIGHT}`)).toBe(true);
  });
});
