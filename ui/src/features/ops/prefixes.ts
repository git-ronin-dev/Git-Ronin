import type { FlowConfig } from "../../bindings/FlowConfig";
import type { Refs } from "../../bindings/Refs";

/** Prefixes most teams use, offered until the repository has its own. */
const COMMON = ["feature/", "fix/"];

/**
 * Prefixes to offer for a new branch name: Git Flow's, then the folders
 * existing branches use (most used first).
 */
export function branchPrefixes(refs: Refs | undefined, flow: FlowConfig | null): string[] {
  const counts = new Map<string, number>();
  for (const b of refs?.local ?? []) {
    const slash = b.name.indexOf("/");
    if (slash > 0) {
      const prefix = b.name.slice(0, slash + 1);
      counts.set(prefix, (counts.get(prefix) ?? 0) + 1);
    }
  }
  const used = [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([p]) => p);
  const flowPrefixes = flow ? [flow.featurePrefix, flow.releasePrefix, flow.hotfixPrefix] : [];
  const all = [...flowPrefixes, ...used, ...(used.length === 0 && !flow ? COMMON : [])];
  return [...new Set(all.filter((p) => p.trim()))].slice(0, 6);
}
