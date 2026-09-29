import { clsx } from "clsx";
import { memo } from "react";

import { laneColor } from "../../ui/colors";
import { useDrafts } from "../changes/draft";
import { MID, ROW_HEIGHT, x } from "./geometry";

interface WorkingRowProps {
  repo: string;
  /** Files with uncommitted changes. */
  count: number;
  graphWidth: number;
  /** Lane of the HEAD commit when it is the first row, which the node links to. */
  headLane: number | null;
  selected: boolean;
  onSelect: () => void;
}

const RADIUS = 5;

/** The uncommitted-changes row above the newest commit. */
export const WorkingRow = memo(function WorkingRow({
  repo,
  count,
  graphWidth,
  headLane,
  selected,
  onSelect,
}: WorkingRowProps) {
  const summary = useDrafts((s) => s.drafts[repo]?.summary.trim() ?? "");
  const lane = headLane ?? 0;
  const color = laneColor(lane);
  return (
    <div
      role="row"
      aria-selected={selected}
      aria-label="Uncommitted changes"
      onMouseDown={onSelect}
      className={clsx("flex h-full items-center", selected ? "bg-accent/20" : "hover:bg-hover")}
    >
      <div style={{ width: graphWidth }} className="h-full shrink-0">
        {/* Overflows into the first row to reach the HEAD commit's node. */}
        <svg
          width={graphWidth}
          height={ROW_HEIGHT}
          className="pointer-events-none relative z-10 block overflow-visible"
          aria-hidden
        >
          {headLane !== null && (
            <line
              x1={x(lane)}
              y1={MID + RADIUS}
              x2={x(lane)}
              y2={ROW_HEIGHT + MID - RADIUS}
              stroke={color}
              strokeWidth={2}
              strokeDasharray="3 3"
            />
          )}
          <circle
            cx={x(lane)}
            cy={MID}
            r={RADIUS}
            fill="var(--rn-canvas)"
            stroke={color}
            strokeWidth={2}
            strokeDasharray="3 2"
          />
        </svg>
      </div>
      <div className="flex min-w-0 flex-1 items-center gap-2 px-2">
        <span className={clsx("truncate", summary ? "text-fg" : "text-fg-muted italic")}>
          {summary || "Uncommitted changes"}
        </span>
        <span className="shrink-0 rounded-sm bg-raised px-1.5 text-xs text-fg-muted">
          {count} {count === 1 ? "file" : "files"}
        </span>
      </div>
    </div>
  );
});
