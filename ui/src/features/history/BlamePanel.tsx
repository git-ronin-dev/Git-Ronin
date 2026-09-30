import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useCallback, useMemo, useState } from "react";

import type { Blame } from "../../bindings/Blame";
import type { BlameCommit } from "../../bindings/BlameCommit";
import { ipc } from "../../lib/ipc";
import { formatDate, relativeTime } from "../../lib/time";
import { Button } from "../../ui/Button";
import { DiffHeader } from "../commit/DiffChrome";
import type { Token } from "../commit/highlight";
import { displayText } from "../commit/lines";
import { useEscape } from "../commit/useEscape";
import { useLineHighlighter } from "../commit/useSyntax";
import { revealCommit } from "../graph/reveal";
import { keys, useSetUiPrefs, useUiPrefs } from "../workspace/queries";
import { updateView } from "../workspace/view";

/** Rendering tens of thousands of lines at once stalls the webview. */
const LINE_LIMIT = 3000;

const uncommitted = (c: BlameCommit) => /^0+$/.test(c.oid);

/** Replaces the graph with who last changed each line of a file. */
export function BlamePanel({
  repo,
  path,
  rev,
}: {
  repo: string;
  path: string;
  /** A commit, or null for the working tree. */
  rev: string | null;
}) {
  const prefs = useUiPrefs();
  const setPrefs = useSetUiPrefs();
  const ignoreWhitespace = prefs?.ignoreWhitespace ?? false;
  const blame = useQuery({
    queryKey: [...keys.history(repo), "blame", path, rev, ignoreWhitespace],
    queryFn: () => ipc.blame(repo, path, rev, ignoreWhitespace),
    // While the same file's blame refreshes, keep showing the last one.
    placeholderData: (previous, query) => (query?.queryKey[3] === path ? previous : undefined),
  });
  const close = useCallback(() => updateView(repo, { openFile: null }), [repo]);
  useEscape(close);

  return (
    <div className="flex h-full flex-col">
      <DiffHeader path={path} oldPath={null} onClose={close}>
        <span className="text-xs text-fg-muted">
          Blame {rev ? `at ${rev.slice(0, 7)}` : "of the working copy"}
        </span>
        {prefs && (
          <label className="ml-2 flex items-center gap-1.5 text-xs text-fg-muted">
            <input
              type="checkbox"
              checked={ignoreWhitespace}
              onChange={(e) => setPrefs.mutate({ ...prefs, ignoreWhitespace: e.target.checked })}
            />
            Ignore whitespace
          </label>
        )}
        <Button
          variant="ghost"
          className="h-6 px-2 text-xs"
          onClick={() => updateView(repo, { openFile: { kind: "history", path } })}
        >
          File history
        </Button>
      </DiffHeader>
      <div className="min-h-0 flex-1 overflow-auto">
        {blame.isError ? (
          <p className="p-8 text-center text-danger select-text">{String(blame.error)}</p>
        ) : blame.data ? (
          <BlameLines repo={repo} path={path} blame={blame.data} />
        ) : (
          <p className="p-8 text-center text-fg-faint">Loading…</p>
        )}
      </div>
    </div>
  );
}

function BlameLines({ repo, path, blame }: { repo: string; path: string; blame: Blame }) {
  const [showAll, setShowAll] = useState(false);
  const highlight = useLineHighlighter(path);
  const lines = useMemo(
    () => (showAll ? blame.lines : blame.lines.slice(0, LINE_LIMIT)),
    [blame, showAll],
  );
  const tokens: Token[][] | null = useMemo(
    () => (highlight ? highlight(lines.map((l) => displayText(l.text))) : null),
    [highlight, lines],
  );
  // Older changes fade: the age bar is brightest for the newest commit.
  const [oldest, newest] = useMemo(() => {
    const times = blame.commits.filter((c) => !uncommitted(c)).map((c) => c.time);
    return [Math.min(...times), Math.max(...times)];
  }, [blame.commits]);
  const age = (c: BlameCommit) =>
    uncommitted(c) || newest === oldest ? 1 : 0.15 + (0.85 * (c.time - oldest)) / (newest - oldest);

  if (blame.lines.length === 0) return <p className="p-8 text-center text-fg-muted">Empty file.</p>;
  return (
    <div className="font-mono text-xs leading-5 [tab-size:4]">
      {lines.map((line, i) => {
        const commit = blame.commits[line.commit]!;
        const first = i === 0 || lines[i - 1]!.commit !== line.commit;
        return (
          <div key={i} className={clsx("flex", first && i > 0 && "border-t border-line")}>
            <div
              className="flex w-72 shrink-0 items-center gap-2 border-r border-line pr-2 font-sans select-none"
              style={{
                boxShadow: `inset 3px 0 0 color-mix(in srgb, var(--rn-accent) ${Math.round(age(commit) * 100)}%, transparent)`,
              }}
            >
              {first && (
                <button
                  type="button"
                  disabled={uncommitted(commit)}
                  onClick={() => void revealCommit(repo, commit.oid)}
                  title={
                    uncommitted(commit)
                      ? "Not committed yet"
                      : `${commit.summary}\n${commit.authorName} <${commit.authorEmail}>\n${formatDate(commit.time)}${commit.path !== path ? `\nas ${commit.path}` : ""}`
                  }
                  className="flex min-w-0 flex-1 items-center gap-2 pl-3 text-left hover:text-fg disabled:cursor-default"
                >
                  <span className="shrink-0 font-mono text-fg-faint">
                    {uncommitted(commit) ? "·······" : commit.oid.slice(0, 7)}
                  </span>
                  <span className="min-w-0 flex-1 truncate text-fg-muted">
                    {uncommitted(commit) ? "Uncommitted" : commit.summary}
                  </span>
                  <span className="shrink-0 text-fg-faint">
                    {uncommitted(commit) ? "" : relativeTime(commit.time)}
                  </span>
                </button>
              )}
            </div>
            <span className="w-12 shrink-0 pr-2 text-right text-fg-faint select-none">{i + 1}</span>
            <span className="min-w-0 flex-1 pr-3 break-all whitespace-pre-wrap select-text">
              {tokens?.[i]
                ? tokens[i].map((t, k) =>
                    t.className ? (
                      <span key={k} className={t.className}>
                        {t.text}
                      </span>
                    ) : (
                      t.text
                    ),
                  )
                : displayText(line.text)}
            </span>
          </div>
        );
      })}
      {!showAll && blame.lines.length > LINE_LIMIT && (
        <div className="flex items-center justify-center gap-3 p-4 font-sans text-fg-muted">
          Showing {LINE_LIMIT.toLocaleString()} of {blame.lines.length.toLocaleString()} lines.
          <Button onClick={() => setShowAll(true)}>Show all</Button>
        </div>
      )}
    </div>
  );
}
