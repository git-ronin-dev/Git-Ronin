import type { BisectState } from "../../bindings/BisectState";

export type BisectLabel = { kind: "bad" | "good" | "skipped" | "found"; label: string };

/** What a bisect has said about each commit, shown next to its refs. */
export function bisectLabels(state: BisectState | null | undefined): Map<string, BisectLabel> {
  const labels = new Map<string, BisectLabel>();
  if (!state) return labels;
  for (const oid of state.skipped) labels.set(oid, { kind: "skipped", label: "skipped" });
  for (const oid of state.good) labels.set(oid, { kind: "good", label: state.goodTerm });
  if (state.bad) labels.set(state.bad, { kind: "bad", label: state.badTerm });
  if (state.firstBad)
    labels.set(state.firstBad, { kind: "found", label: `first ${state.badTerm}` });
  return labels;
}
