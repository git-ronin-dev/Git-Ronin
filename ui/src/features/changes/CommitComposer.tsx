import { useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useEffect, useRef, useState } from "react";

import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { toast } from "../../ui/toast-store";
import { usePendingMessage } from "../ops/queries";
import { invalidateRepo, keys, useRepoInfo } from "../workspace/queries";
import { updateView } from "../workspace/view";
import { composeMessage, splitMessage, useDraft, useDrafts } from "./draft";

/** Summaries longer than this get cut off in many tools. */
const SUMMARY_LIMIT = 72;

interface CommitComposerProps {
  repo: string;
  stagedCount: number;
  conflicts: number;
}

export function CommitComposer({ repo, stagedCount, conflicts }: CommitComposerProps) {
  const client = useQueryClient();
  const draft = useDraft(repo);
  const { update, clear } = useDrafts.getState();
  const info = useRepoInfo(repo).data;
  const head = info?.head;
  const unborn = head?.kind === "branch" && head.unborn;
  const operation = info?.operation ?? null;
  // A commit concludes a stopped merge, cherry-pick or revert.
  const concludes = operation === "merge" || operation === "cherryPick" || operation === "revert";
  const [busy, setBusy] = useState(false);
  usePrefill(repo, concludes);

  const message = composeMessage(draft);
  const blocker =
    operation !== null && !concludes
      ? "Use Continue above to go on"
      : conflicts > 0
        ? "Resolve conflicts first"
        : !draft.amend && stagedCount === 0
          ? "Stage changes to commit"
          : !message
            ? "Write a summary"
            : null;

  const headMessage = () =>
    client.fetchQuery({
      queryKey: keys.headMessage(repo),
      queryFn: () => ipc.headMessage(repo),
    });

  const toggleAmend = async (amend: boolean) => {
    update(repo, { amend });
    // Start from the message being amended, unless the user already wrote one.
    const current = useDrafts.getState().drafts[repo];
    const previous = await headMessage().catch(() => null);
    if (!previous || !current) return;
    if (amend && !current.summary.trim() && !current.body.trim()) {
      update(repo, splitMessage(previous));
    } else if (!amend && composeMessage(current) === composeMessage(splitMessage(previous))) {
      update(repo, { summary: "", body: "" });
    }
  };

  const submit = async () => {
    if (blocker || busy) return;
    setBusy(true);
    try {
      const result = await ipc.commit(repo, message, {
        amend: draft.amend,
        signoff: draft.signoff,
        noVerify: draft.noVerify,
      });
      clear(repo);
      updateView(repo, { selected: result.oid, openFile: null });
      if (result.output) toast.output("Output from git hooks", result.output);
    } catch (err) {
      toast.error(draft.amend ? "Could not amend commit" : "Could not commit", String(err));
    } finally {
      setBusy(false);
      void invalidateRepo(client, repo);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void submit();
    }
  };

  const length = draft.summary.length;
  return (
    <form
      aria-label="Commit"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
      className="shrink-0 space-y-2 border-t border-line p-3"
    >
      <div className="relative">
        <input
          value={draft.summary}
          onChange={(e) => update(repo, { summary: e.target.value })}
          onKeyDown={onKeyDown}
          placeholder="Summary"
          aria-label="Commit summary"
          className="h-8 w-full rounded-md border border-line bg-canvas pr-12 pl-2 outline-none focus:border-accent"
        />
        <span
          title={`Summaries over ${SUMMARY_LIMIT} characters get cut off in many tools`}
          className={clsx(
            "absolute top-1/2 right-2 -translate-y-1/2 font-mono text-xs",
            length > SUMMARY_LIMIT ? "text-warning" : "text-fg-faint",
          )}
        >
          {length}/{SUMMARY_LIMIT}
        </span>
      </div>
      <textarea
        value={draft.body}
        onChange={(e) => update(repo, { body: e.target.value })}
        onKeyDown={onKeyDown}
        placeholder="Description"
        aria-label="Commit description"
        rows={4}
        className="block w-full resize-y rounded-md border border-line bg-canvas px-2 py-1.5 outline-none focus:border-accent"
      />
      <div className="flex gap-4 text-xs text-fg-muted">
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={draft.amend}
            disabled={unborn || operation !== null}
            onChange={(e) => void toggleAmend(e.target.checked)}
          />
          Amend previous commit
        </label>
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={draft.signoff}
            onChange={(e) => update(repo, { signoff: e.target.checked })}
          />
          Sign off
        </label>
        <label
          className="flex items-center gap-1.5"
          title="Don't run the pre-commit and commit-msg hooks"
        >
          <input
            type="checkbox"
            checked={draft.noVerify}
            onChange={(e) => update(repo, { noVerify: e.target.checked })}
          />
          Skip hooks
        </label>
      </div>
      <Button
        type="submit"
        variant="primary"
        disabled={blocker !== null || busy}
        title="Ctrl+Enter"
        className="w-full justify-center"
      >
        {busy
          ? "Committing…"
          : blocker !== null
            ? blocker
            : draft.amend
              ? "Amend previous commit"
              : stagedCount > 0
                ? `Commit ${stagedCount} ${stagedCount === 1 ? "file" : "files"}`
                : "Commit"}
      </Button>
    </form>
  );
}

/** Starts the message of a commit that concludes a merge (etc.) from the one git prepared. */
function usePrefill(repo: string, enabled: boolean) {
  const pending = usePendingMessage(repo, enabled).data;
  const used = useRef<string | null>(null);
  useEffect(() => {
    if (!enabled || !pending || used.current === pending) return;
    used.current = pending;
    const draft = useDrafts.getState().drafts[repo];
    if (!draft?.summary.trim() && !draft?.body.trim()) {
      useDrafts.getState().update(repo, splitMessage(pending));
    }
  }, [repo, enabled, pending]);
}
