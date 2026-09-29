import { TriangleAlert } from "lucide-react";

import type { Operation } from "../../bindings/Operation";
import { Button } from "../../ui/Button";
import { useRepoInfo } from "../workspace/queries";
import { useGitActions } from "./actions";

const NAMES: Record<Operation, string> = {
  merge: "A merge",
  rebase: "A rebase",
  cherryPick: "A cherry-pick",
  revert: "A revert",
  applyMailbox: "Applying patches",
  bisect: "A bisect",
};

/** Shown while a merge, rebase, … waits for conflicts to be resolved. */
export function OperationBanner({ repo }: { repo: string }) {
  const operation = useRepoInfo(repo).data?.operation;
  const actions = useGitActions(repo);
  if (!operation) return null;
  const bisect = operation === "bisect";
  return (
    <div
      role="status"
      className="flex shrink-0 items-center gap-2 border-b border-line bg-warning/10 px-3 py-1.5"
    >
      <TriangleAlert className="size-4 shrink-0 text-warning" />
      <span className="min-w-0 flex-1 truncate">
        {NAMES[operation]} is in progress.{" "}
        <span className="text-fg-muted">
          {bisect
            ? "Bisecting isn't supported yet; end it to continue."
            : "Resolve conflicts and stage the files, then continue."}
        </span>
      </span>
      {!bisect && (
        <Button variant="primary" className="h-6" onClick={() => void actions.continueOperation()}>
          Continue
        </Button>
      )}
      {!bisect && operation !== "merge" && (
        <Button className="h-6" onClick={() => void actions.skipOperation()}>
          Skip commit
        </Button>
      )}
      <Button className="h-6" onClick={() => void actions.abortOperation()}>
        {bisect ? "End" : "Abort"}
      </Button>
    </div>
  );
}
