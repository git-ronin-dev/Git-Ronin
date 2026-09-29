import type { Edge } from "../../bindings/Edge";

export const ROW_HEIGHT = 28;
export const LANE_WIDTH = 16;

export const MID = ROW_HEIGHT / 2;
export const x = (lane: number) => LANE_WIDTH / 2 + lane * LANE_WIDTH;

/** SVG path for one edge. Lane changes are S-curves that leave and arrive vertically. */
export function edgePath({ kind, from, to }: Edge): string {
  const x1 = x(from);
  const x2 = x(to);
  switch (kind) {
    case "pass":
      return `M${x1} 0V${ROW_HEIGHT}`;
    case "in":
      return x1 === x2
        ? `M${x1} 0V${MID}`
        : `M${x1} 0C${x1} ${MID / 2} ${x2} ${MID / 2} ${x2} ${MID}`;
    case "out": {
      const bend = MID + MID / 2;
      return x1 === x2
        ? `M${x1} ${MID}V${ROW_HEIGHT}`
        : `M${x1} ${MID}C${x1} ${bend} ${x2} ${bend} ${x2} ${ROW_HEIGHT}`;
    }
  }
}
