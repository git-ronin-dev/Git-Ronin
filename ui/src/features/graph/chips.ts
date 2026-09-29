import type { RefLabel } from "../../bindings/RefLabel";

export interface Chip {
  key: string;
  text: string;
  local: boolean;
  remote: boolean;
  tag: boolean;
  current: boolean;
}

export const MAX_CHIPS = 2;

/** Groups labels into chips: a local branch and its same-named remote branches share one. */
export function toChips(refs: RefLabel[]): Chip[] {
  const chips: Chip[] = [];
  const byBranch = new Map<string, Chip>();
  for (const r of refs) {
    if (r.kind === "local" || r.kind === "head") {
      const chip = {
        key: r.fullName,
        text: r.name,
        local: r.kind === "local",
        remote: false,
        tag: false,
        current: r.current,
      };
      chips.push(chip);
      if (r.kind === "local") byBranch.set(r.name, chip);
    }
  }
  for (const r of refs) {
    if (r.kind === "remote") {
      const branch = r.remote ? r.name.slice(r.remote.length + 1) : r.name;
      const local = byBranch.get(branch);
      if (local) local.remote = true;
      else
        chips.push({
          key: r.fullName,
          text: r.name,
          local: false,
          remote: true,
          tag: false,
          current: false,
        });
    } else if (r.kind === "tag") {
      chips.push({
        key: r.fullName,
        text: r.name,
        local: false,
        remote: false,
        tag: true,
        current: false,
      });
    }
  }
  // The checked-out branch first, then branches, then tags.
  return chips.sort(
    (a, b) => Number(b.current) - Number(a.current) || Number(a.tag) - Number(b.tag),
  );
}
