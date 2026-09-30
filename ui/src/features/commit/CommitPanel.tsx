import { Copy } from "lucide-react";

import type { Signature } from "../../bindings/Signature";
import { copyText } from "../../lib/clipboard";
import { formatDate, relativeTime } from "../../lib/time";
import { Avatar } from "../../ui/Avatar";
import { revealCommit } from "../graph/reveal";
import { useUiPrefs } from "../workspace/queries";
import { updateView, useRepoView } from "../workspace/view";
import { FileList, Stats } from "./FileList";
import { useCommitDetail } from "./queries";

export function CommitPanel({ repo }: { repo: string }) {
  const { selected, openFile } = useRepoView(repo);
  const detail = useCommitDetail(repo, selected);
  const showAvatars = useUiPrefs()?.showAvatars ?? false;

  if (!selected) return <Hint>Select a commit to see its details.</Hint>;
  if (detail.isError) return <Hint>{String(detail.error)}</Hint>;
  if (!detail.data) return <Hint>Loading…</Hint>;

  const d = detail.data;
  const newline = d.message.indexOf("\n");
  const summary = newline < 0 ? d.message : d.message.slice(0, newline);
  const body = newline < 0 ? "" : d.message.slice(newline + 1).trim();
  const additions = d.files.reduce((n, f) => n + (f.additions ?? 0), 0);
  const deletions = d.files.reduce((n, f) => n + (f.deletions ?? 0), 0);
  const sameCommitter = d.committer.name === d.author.name && d.committer.email === d.author.email;

  return (
    <div className="flex h-full flex-col">
      <div className="max-h-[55%] shrink-0 overflow-y-auto border-b border-line p-4 select-text">
        <h2 className="text-sm font-semibold break-words">{summary}</h2>
        {body && <p className="mt-2 break-words whitespace-pre-wrap text-fg-muted">{body}</p>}

        <div className="mt-4 space-y-2 select-none">
          <Person label="Author" who={d.author} showAvatar={showAvatars} />
          {!sameCommitter && (
            <Person label="Committer" who={d.committer} showAvatar={showAvatars} />
          )}
          <div className="flex items-center gap-2 text-xs text-fg-muted">
            <span className="w-16 shrink-0">Commit</span>
            <span className="font-mono text-fg select-text">{d.oid.slice(0, 12)}</span>
            <button
              type="button"
              aria-label="Copy commit SHA"
              onClick={() => void copyText(d.oid, "Copied commit SHA")}
              className="text-fg-faint hover:text-fg"
            >
              <Copy className="size-3.5" />
            </button>
          </div>
          {d.parents.length > 0 && (
            <div className="flex items-center gap-2 text-xs text-fg-muted">
              <span className="w-16 shrink-0">{d.parents.length > 1 ? "Parents" : "Parent"}</span>
              {d.parents.map((p) => (
                <button
                  key={p}
                  type="button"
                  onClick={() => void revealCommit(repo, p)}
                  className="font-mono text-accent hover:underline"
                >
                  {p.slice(0, 7)}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="flex h-8 shrink-0 items-center justify-between px-3 text-xs text-fg-muted">
        <span>
          {d.files.length} {d.files.length === 1 ? "file" : "files"} changed
        </span>
        <Stats file={{ additions, deletions }} />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto pb-2">
        {d.filesError && (
          <p className="px-3 py-2 text-xs break-words text-danger select-text">
            Could not list changed files: {d.filesError}
          </p>
        )}
        <FileList
          files={d.files}
          activePath={
            openFile?.kind === "commit" && openFile.oid === d.oid ? openFile.file.path : null
          }
          onOpen={(file) => updateView(repo, { openFile: { kind: "commit", oid: d.oid, file } })}
          menu={(file) => [
            {
              label: "Blame at this commit",
              disabled: file.status === "deleted",
              onSelect: () =>
                updateView(repo, { openFile: { kind: "blame", path: file.path, rev: d.oid } }),
            },
            {
              label: "File history",
              onSelect: () => updateView(repo, { openFile: { kind: "history", path: file.path } }),
            },
            "separator",
            { label: "Copy path", onSelect: () => void copyText(file.path, "Copied path") },
          ]}
        />
      </div>
    </div>
  );
}

function Person({
  label,
  who,
  showAvatar,
}: {
  label: string;
  who: Signature;
  showAvatar: boolean;
}) {
  return (
    <div className="flex items-center gap-2 text-xs text-fg-muted">
      <span className="w-16 shrink-0">{label}</span>
      <Avatar name={who.name} email={who.email} size={20} remote={showAvatar} />
      <span className="min-w-0 truncate">
        <span className="text-fg">{who.name}</span>{" "}
        <span className="select-text">&lt;{who.email}&gt;</span>
      </span>
      <span className="ml-auto shrink-0" title={formatDate(who.time)}>
        {relativeTime(who.time)}
      </span>
    </div>
  );
}

function Hint({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center p-4 text-center text-fg-faint">
      {children}
    </div>
  );
}
