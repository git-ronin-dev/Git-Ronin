import { memo, useId } from "react";

import type { Edge } from "../../bindings/Edge";
import type { GraphRow } from "../../bindings/GraphRow";
import { laneColor } from "../../ui/colors";
import { ensoPath } from "../../ui/enso";
import { MID, ROW_HEIGHT, edgePath, x } from "./geometry";

/** The colour of the lane an edge belongs to (the non-node end). */
function edgeColor({ kind, from, to }: Edge) {
  return laneColor(kind === "out" ? to : from);
}

/** Radius of the ensō around a commit; edges stop short of it. */
const RING = 4.6;

export const GraphLanes = memo(function GraphLanes({
  row,
  width,
}: {
  row: GraphRow;
  width: number;
}) {
  const mask = useId();
  const isMerge = row.parents.length > 1;
  const color = laneColor(row.lane);
  const cx = x(row.lane);
  return (
    <svg width={width} height={ROW_HEIGHT} className="block shrink-0" aria-hidden>
      {/* User space: a row of straight lanes has a zero-width bounding box. */}
      <mask id={mask} maskUnits="userSpaceOnUse" x={0} y={0} width={width} height={ROW_HEIGHT}>
        <rect width={width} height={ROW_HEIGHT} fill="white" />
        <circle cx={cx} cy={MID} r={RING + 1} fill="black" />
      </mask>
      <g mask={`url(#${mask})`} fill="none">
        {/* A faint, wider stroke under each lane: ink soaking into paper. */}
        {row.edges.map((edge, i) => (
          <path
            key={`ink${i}`}
            d={edgePath(edge)}
            stroke={edgeColor(edge)}
            strokeWidth={5}
            strokeOpacity={0.14}
          />
        ))}
        {row.edges.map((edge, i) => (
          <path key={i} d={edgePath(edge)} stroke={edgeColor(edge)} strokeWidth={2} />
        ))}
      </g>
      {/* Commits are ensō; a merge is left empty inside. */}
      <path
        d={ensoPath(cx, MID, RING, 45)}
        stroke={color}
        strokeWidth={2.2}
        strokeLinecap="round"
        fill="none"
      />
      {!isMerge && <circle cx={cx} cy={MID} r={2} fill={color} />}
    </svg>
  );
});
