import type { BisectState } from "../../bindings/BisectState";
import type { Operation } from "../../bindings/Operation";
import type { RepoInfo } from "../../bindings/RepoInfo";
import { Button } from "../../ui/Button";
import { SearchCheck, TriangleAlert } from "../../ui/icons";
import { useCommitDetail } from "../commit/queries";
import { revealCommit } from "../graph/reveal";
import { useRepoInfo } from "../workspace/queries";
import { useGitActions } from "./actions";
import { useBisect } from "./queries";

const NAMES: Record<Operation, string> = {
  merge: "A merge",
  rebase: "A rebase",
  cherryPick: "A cherry-pick",
  revert: "A revert",
  applyMailbox: "Applying patches",
  bisect: "A bisect",
};

/** Shown while a merge, rebase, … waits for the user, and during a bisect. */
export function OperationBanner({ repo }: { repo: string }) {
  const info = useRepoInfo(repo).data;
  const operation = info?.operation;
  if (!operation) return null;
  if (operation === "bisect") return <BisectBar repo={repo} />;
  return <Stopped repo={repo} info={info} operation={operation} />;
}

function Bar({ icon, children }: { icon?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div
      role="status"
      className="flex shrink-0 items-center gap-2 border-b border-line bg-warning/10 px-3 py-1.5"
    >
      {icon ?? <TriangleAlert className="size-4 shrink-0 text-warning" />}
      {children}
    </div>
  );
}

function Stopped({
  repo,
  info,
  operation,
}: {
  repo: string;
  info: RepoInfo;
  operation: Operation;
}) {
  const actions = useGitActions(repo);
  const rebase = info.rebase;
  const title =
    operation === "rebase" && rebase
      ? `Rebasing${rebase.branch ? ` ${rebase.branch}` : ""}: step ${rebase.step} of ${rebase.total}.`
      : `${NAMES[operation]} is in progress.`;
  const hint = rebase?.editing
    ? `Stopped to edit ${rebase.stoppedAt?.slice(0, 7) ?? "a commit"}: amend it or add commits, then continue.`
    : "Resolve conflicts and stage the files, then continue.";
  return (
    <Bar>
      <span className="min-w-0 flex-1 truncate">
        {title} <span className="text-fg-muted">{hint}</span>
      </span>
      <Button variant="primary" className="h-6" onClick={() => void actions.continueOperation()}>
        Continue
      </Button>
      {operation !== "merge" && (
        <Button className="h-6" onClick={() => void actions.skipOperation()}>
          Skip commit
        </Button>
      )}
      <Button className="h-6" onClick={() => void actions.abortOperation()}>
        Abort
      </Button>
    </Bar>
  );
}

function BisectBar({ repo }: { repo: string }) {
  const actions = useGitActions(repo);
  const state = useBisect(repo, true).data;
  const end = (
    <Button className="h-6" onClick={() => void actions.endBisect()}>
      End bisect
    </Button>
  );
  if (!state) {
    return (
      <Bar icon={<SearchCheck className="size-4 shrink-0 text-accent" />}>
        <span className="min-w-0 flex-1 truncate">Bisecting…</span>
        {end}
      </Bar>
    );
  }
  return (
    <Bar icon={<SearchCheck className="size-4 shrink-0 text-accent" />}>
      <span className="min-w-0 flex-1 truncate">
        <BisectSummary repo={repo} state={state} />
      </span>
      {state.firstBad ? (
        <Button
          variant="primary"
          className="h-6"
          onClick={() => void revealCommit(repo, state.firstBad!)}
        >
          Show commit
        </Button>
      ) : (
        state.bad &&
        state.good.length > 0 &&
        !state.stuck && (
          <>
            <Button className="h-6" onClick={() => void actions.bisect("good", "HEAD")}>
              Good
            </Button>
            <Button className="h-6" onClick={() => void actions.bisect("bad", "HEAD")}>
              Bad
            </Button>
            <Button className="h-6" onClick={() => void actions.bisect("skip", "HEAD")}>
              Skip
            </Button>
          </>
        )
      )}
      {end}
    </Bar>
  );
}

function BisectSummary({ repo, state }: { repo: string; state: BisectState }) {
  const found = useCommitDetail(repo, state.firstBad).data;
  if (state.firstBad) {
    const summary = found?.message.split("\n", 1)[0] ?? "";
    return (
      <>
        Found it: <span className="font-mono">{state.firstBad.slice(0, 7)}</span> {summary}{" "}
        <span className="text-fg-muted">is the first {state.badTerm} commit.</span>
      </>
    );
  }
  if (state.stuck) {
    return (
      <>
        Only skipped commits are left.{" "}
        <span className="text-fg-muted">
          The first {state.badTerm} commit is one of the {state.remaining} left; end the bisect.
        </span>
      </>
    );
  }
  if (!state.bad || state.good.length === 0) {
    const missing = !state.bad ? state.badTerm : state.goodTerm;
    return (
      <>
        Bisecting.{" "}
        <span className="text-fg-muted">
          Right-click a {missing} commit in the graph and mark it {missing} to start.
        </span>
      </>
    );
  }
  const steps = Math.ceil(Math.log2(Math.max(state.remaining, 1)));
  return (
    <>
      Bisecting: {state.remaining} {state.remaining === 1 ? "commit" : "commits"} left, about{" "}
      {steps} {steps === 1 ? "step" : "steps"}.{" "}
      <span className="text-fg-muted">
        Test the checked-out commit and mark it {state.goodTerm} or {state.badTerm}.
      </span>
    </>
  );
}
