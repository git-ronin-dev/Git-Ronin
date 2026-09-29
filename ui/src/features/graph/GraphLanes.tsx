import { memo } from "react";

import type { Edge } from "../../bindings/Edge";
import type { GraphRow } from "../../bindings/GraphRow";
import { laneColor } from "../../ui/colors";
import { MID, ROW_HEIGHT, edgePath, x } from "./geometry";

/** The colour of the lane an edge belongs to (the non-node end). */
function edgeColor({ kind, from, to }: Edge) {
  return laneColor(kind === "out" ? to : from);
}

export const GraphLanes = memo(function GraphLanes({
  row,
  width,
}: {
  row: GraphRow;
  width: number;
}) {
  const isMerge = row.parents.length > 1;
  const color = laneColor(row.lane);
  return (
    <svg width={width} height={ROW_HEIGHT} className="block shrink-0" aria-hidden>
      {row.edges.map((edge, i) => (
        <path key={i} d={edgePath(edge)} stroke={edgeColor(edge)} strokeWidth={2} fill="none" />
      ))}
      <circle
        cx={x(row.lane)}
        cy={MID}
        r={isMerge ? 3.5 : 5}
        fill={isMerge ? "var(--rn-canvas)" : color}
        stroke={color}
        strokeWidth={2}
      />
    </svg>
  );
});
