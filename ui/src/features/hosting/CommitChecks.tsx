import { useState } from "react";

import { CheckIcon, Checks } from "./Checks";
import { pickLink, useCiStatus, useRepoLinks } from "./queries";

const SUMMARY = {
  success: "All checks passed",
  failure: "Some checks failed",
  pending: "Checks are running",
  neutral: "Checks skipped",
} as const;

/** The CI checks a hosting service reports for a commit, when there are any. */
export function CommitChecks({ repo, oid }: { repo: string; oid: string }) {
  const link = pickLink(useRepoLinks(repo).data, "ci");
  const ci = useCiStatus(link, oid);
  const [open, setOpen] = useState(false);
  const state = ci.data?.state;
  if (!state) return null;
  return (
    <div className="text-xs text-fg-muted">
      <div className="flex items-center gap-2">
        <span className="w-16 shrink-0">Checks</span>
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="flex items-center gap-1.5 text-fg hover:underline"
        >
          <CheckIcon state={state} />
          {SUMMARY[state]} ({ci.data!.checks.length})
        </button>
      </div>
      {open && (
        <div className="mt-2 ml-[4.5rem]">
          <Checks status={ci.data!} />
        </div>
      )}
    </div>
  );
}
