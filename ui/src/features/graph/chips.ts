import type { RefLabel } from "../../bindings/RefLabel";
import type { DragRef } from "../ops/drag";

export interface Chip {
  /** Full name of the chip's main ref (the local branch, if any). */
  key: string;
  kind: RefLabel["kind"];
  text: string;
  /** For a remote-only chip: its remote. */
  remoteName: string | null;
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
      const chip: Chip = {
        key: r.fullName,
        kind: r.kind,
        text: r.name,
        remoteName: null,
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
          kind: r.kind,
          text: r.name,
          remoteName: r.remote,
          local: false,
          remote: true,
          tag: false,
          current: false,
        });
    } else if (r.kind === "tag") {
      chips.push({
        key: r.fullName,
        kind: r.kind,
        text: r.name,
        remoteName: null,
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

/** The branch a chip stands for, if it is one. */
export function chipRef(chip: Chip, oid: string): DragRef | null {
  if (chip.kind === "local") {
    return { kind: "local", name: chip.text, fullName: chip.key, oid };
  }
  if (chip.kind === "remote" && chip.remoteName) {
    const branch = chip.text.slice(chip.remoteName.length + 1);
    return {
      kind: "remote",
      name: chip.text,
      fullName: chip.key,
      oid,
      remote: chip.remoteName,
      branch,
    };
  }
  return null;
}
