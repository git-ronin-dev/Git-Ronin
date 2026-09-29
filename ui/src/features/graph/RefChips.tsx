import { clsx } from "clsx";
import { Cloud, Laptop, Tag } from "lucide-react";

import type { RefLabel } from "../../bindings/RefLabel";
import { laneColor } from "../../ui/colors";
import { Tooltip } from "../../ui/Tooltip";
import { MAX_CHIPS, toChips } from "./chips";

export function RefChips({ refs, lane }: { refs: RefLabel[]; lane: number }) {
  if (refs.length === 0) return null;
  const chips = toChips(refs);
  const shown = chips.slice(0, MAX_CHIPS);
  const hidden = chips.slice(MAX_CHIPS);
  return (
    <span className="flex shrink-0 items-center gap-1">
      {shown.map((chip) => (
        <span
          key={chip.key}
          style={{ borderColor: laneColor(lane) }}
          className={clsx(
            "flex h-5 max-w-48 items-center gap-1 rounded-sm border px-1.5 text-xs",
            chip.current ? "bg-raised font-semibold text-fg" : "text-fg-muted",
          )}
        >
          {chip.tag && <Tag className="size-3 shrink-0" />}
          <span className="truncate">{chip.text}</span>
          {chip.local && <Laptop className="size-3 shrink-0" aria-label="local" />}
          {chip.remote && <Cloud className="size-3 shrink-0" aria-label="remote" />}
        </span>
      ))}
      {hidden.length > 0 && (
        <Tooltip content={hidden.map((c) => c.text).join(", ")}>
          <span className="rounded-sm bg-raised px-1 text-xs text-fg-muted">+{hidden.length}</span>
        </Tooltip>
      )}
    </span>
  );
}
