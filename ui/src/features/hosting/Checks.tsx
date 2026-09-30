import { clsx } from "clsx";
import { CircleCheck, CircleDashed, CircleMinus, CircleX } from "lucide-react";

import type { CheckState } from "../../bindings/CheckState";
import type { CiStatus } from "../../bindings/CiStatus";
import { openUrl } from "../../lib/open";

const STATES: Record<CheckState, { icon: typeof CircleCheck; className: string; label: string }> = {
  success: { icon: CircleCheck, className: "text-success", label: "passed" },
  failure: { icon: CircleX, className: "text-danger", label: "failed" },
  pending: { icon: CircleDashed, className: "text-warning", label: "running" },
  neutral: { icon: CircleMinus, className: "text-fg-faint", label: "skipped" },
};

/** The combined state of a commit's checks, as an icon. */
export function CheckIcon({ state, className }: { state: CheckState; className?: string }) {
  const { icon: Icon, className: color, label } = STATES[state];
  return (
    <Icon aria-label={`Checks ${label}`} className={clsx("size-3.5 shrink-0", color, className)} />
  );
}

/** Every check on a commit, linking to its details. */
export function Checks({ status }: { status: CiStatus }) {
  if (status.checks.length === 0)
    return <p className="text-xs text-fg-faint">No checks reported for this commit.</p>;
  return (
    <ul className="space-y-1">
      {status.checks.map((check, i) => (
        <li key={`${check.name}-${i}`} className="flex items-center gap-2 text-xs">
          <CheckIcon state={check.state} />
          {check.url ? (
            <button
              type="button"
              className="truncate text-fg hover:underline"
              onClick={() => void openUrl(check.url!)}
            >
              {check.name}
            </button>
          ) : (
            <span className="truncate">{check.name}</span>
          )}
          {check.description && <span className="truncate text-fg-faint">{check.description}</span>}
        </li>
      ))}
    </ul>
  );
}
