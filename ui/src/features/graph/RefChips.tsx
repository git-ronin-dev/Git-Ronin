import { clsx } from "clsx";

import type { RefLabel } from "../../bindings/RefLabel";
import { laneColor } from "../../ui/colors";
import { Archive, Cloud, Laptop, Tag } from "../../ui/icons";
import { Tooltip } from "../../ui/Tooltip";
import type { GitActions } from "../ops/actions";
import { beginDrag, dropProps, useDrag } from "../ops/drag";
import type { RepoContext } from "../ops/menus";
import { MAX_CHIPS, chipRef, toChips, type Chip } from "./chips";

interface RefChipsProps {
  refs: RefLabel[];
  lane: number;
  oid: string;
  ctx: RepoContext;
  actions: GitActions;
}

/** Branch and tag labels of a commit. Branches can be dragged onto each other. */
export function RefChips({ refs, lane, oid, ctx, actions }: RefChipsProps) {
  if (refs.length === 0) return null;
  const chips = toChips(refs);
  const shown = chips.slice(0, MAX_CHIPS);
  const hidden = chips.slice(MAX_CHIPS);
  return (
    <span className="flex shrink-0 items-center gap-1">
      {shown.map((chip) => (
        <ChipLabel key={chip.key} chip={chip} lane={lane} oid={oid} ctx={ctx} actions={actions} />
      ))}
      {hidden.length > 0 && (
        <Tooltip content={hidden.map((c) => c.text).join(", ")}>
          <span className="rounded-sm bg-raised px-1 text-xs text-fg-muted">+{hidden.length}</span>
        </Tooltip>
      )}
    </span>
  );
}

function ChipLabel({
  chip,
  lane,
  oid,
  ctx,
  actions,
}: {
  chip: Chip;
  lane: number;
  oid: string;
  ctx: RepoContext;
  actions: GitActions;
}) {
  const ref = chipRef(chip, oid);
  const over = useDrag((s) => ref !== null && s.over === ref.fullName);
  const checkout = () => {
    if (!ref || chip.current) return;
    if (ref.kind === "local") void actions.checkout(ref.name);
    else void actions.checkoutRemote(ref.fullName, ref.branch!, ctx.localNames);
  };
  return (
    <span
      style={{ borderColor: laneColor(lane) }}
      title={ref ? "Double-click to check out; drag onto another branch" : undefined}
      onDoubleClick={checkout}
      onPointerDown={ref ? (e) => beginDrag(ctx.repo, ref, e) : undefined}
      {...(ref ? dropProps(ref) : {})}
      className={clsx(
        "flex h-5 max-w-48 items-center gap-1 rounded-sm border px-1.5 text-xs",
        chip.current ? "bg-raised font-semibold text-fg" : "text-fg-muted",
        over && "bg-accent/30 text-fg",
      )}
    >
      {chip.tag && <Tag className="size-3 shrink-0" />}
      {chip.stash && <Archive className="size-3 shrink-0" />}
      <span className="truncate">{chip.text}</span>
      {chip.local && <Laptop className="size-3 shrink-0" aria-label="local" />}
      {chip.remote && <Cloud className="size-3 shrink-0" aria-label="remote" />}
    </span>
  );
}
