import type { ReactNode } from "react";

import type { StatusEntry } from "../../bindings/StatusEntry";
import { Button } from "../../ui/Button";
import { updateView, useRepoView } from "../workspace/view";
import { ChangeList, type Side } from "./ChangeList";
import { CommitComposer } from "./CommitComposer";
import { countChanges, useWorkingActions, useWorkingStatus } from "./queries";

/** Details panel for the uncommitted-changes row: staging lists and the commit composer. */
export function ChangesPanel({ repo }: { repo: string }) {
  const status = useWorkingStatus(repo);
  const { openFile } = useRepoView(repo);
  const actions = useWorkingActions(repo);

  if (status.isError) {
    return <p className="p-4 text-danger select-text">{String(status.error)}</p>;
  }
  const s = status.data;
  const total = countChanges(s);
  const open = (entry: StatusEntry, side: Side) =>
    updateView(repo, {
      openFile:
        side === "conflicted"
          ? { kind: "conflict", path: entry.path }
          : { kind: "working", path: entry.path, staged: side === "staged" },
    });
  const activePath = (side: Side) => {
    if (side === "conflicted") return openFile?.kind === "conflict" ? openFile.path : null;
    return openFile?.kind === "working" && openFile.staged === (side === "staged")
      ? openFile.path
      : null;
  };
  const list = (side: Side, entries: StatusEntry[]) => (
    <ChangeList
      repo={repo}
      side={side}
      entries={entries}
      activePath={activePath(side)}
      onOpen={(e) => open(e, side)}
    />
  );

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-10 shrink-0 items-center justify-between border-b border-line px-3">
        <h2 className="font-semibold">Uncommitted changes</h2>
        {s && (
          <span className="text-xs text-fg-muted">
            {total === 0 ? "Working tree clean" : `${total} ${total === 1 ? "file" : "files"}`}
          </span>
        )}
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto py-1">
        {s && s.conflicted.length > 0 && (
          <Group
            title="Conflicts"
            count={s.conflicted.length}
            hint="Open a file to merge it, or fix the markers yourself and mark it resolved."
          >
            {list("conflicted", s.conflicted)}
          </Group>
        )}
        {s && (
          <Group
            title="Unstaged"
            count={s.unstaged.length}
            actions={
              <>
                <Button
                  variant="ghost"
                  className="h-6 px-2 text-xs"
                  disabled={s.unstaged.length === 0}
                  onClick={() => void actions.discard(s.unstaged)}
                >
                  Discard all
                </Button>
                <Button
                  variant="ghost"
                  className="h-6 px-2 text-xs"
                  disabled={s.unstaged.length === 0}
                  onClick={() => void actions.stage(s.unstaged)}
                >
                  Stage all
                </Button>
              </>
            }
          >
            {list("unstaged", s.unstaged)}
          </Group>
        )}
        {s && (
          <Group
            title="Staged"
            count={s.staged.length}
            actions={
              <Button
                variant="ghost"
                className="h-6 px-2 text-xs"
                disabled={s.staged.length === 0}
                onClick={() => void actions.unstage(s.staged)}
              >
                Unstage all
              </Button>
            }
          >
            {list("staged", s.staged)}
          </Group>
        )}
      </div>
      <CommitComposer
        repo={repo}
        stagedCount={s?.staged.length ?? 0}
        conflicts={s?.conflicted.length ?? 0}
      />
    </div>
  );
}

function Group({
  title,
  count,
  actions,
  hint,
  children,
}: {
  title: string;
  count: number;
  actions?: ReactNode;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <section aria-label={title} className="pb-2">
      <div className="flex h-8 items-center gap-2 px-3">
        <h3 className="text-xs font-semibold tracking-wide text-fg-muted uppercase">{title}</h3>
        <span className="text-xs text-fg-faint">{count}</span>
        <span className="ml-auto flex gap-1">{actions}</span>
      </div>
      {hint && <p className="px-3 pb-1 text-xs text-fg-faint">{hint}</p>}
      {count === 0 ? <p className="px-3 text-xs text-fg-faint">None</p> : children}
    </section>
  );
}
