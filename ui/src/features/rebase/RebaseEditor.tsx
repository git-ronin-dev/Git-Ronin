import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { ArrowDown, ArrowLeft, ArrowUp, CornerLeftDown, GripVertical } from "lucide-react";
import { useCallback, useState } from "react";

import type { RebaseAction } from "../../bindings/RebaseAction";
import type { RebasePlan } from "../../bindings/RebasePlan";
import { ipc } from "../../lib/ipc";
import { formatDate, relativeTime } from "../../lib/time";
import { Button } from "../../ui/Button";
import { confirm } from "../../ui/confirm-store";
import { useEscape } from "../commit/useEscape";
import { shortRev, useGitActions } from "../ops/actions";
import { describeHead } from "../repo/head";
import { useRepoInfo } from "../workspace/queries";
import { updateView } from "../workspace/view";
import {
  ACTIONS,
  initialRows,
  moveRow,
  problem,
  setAction,
  summary,
  toSteps,
  unchanged,
  type Row,
} from "./plan";

/** Replaces the graph while an interactive rebase onto `base` is planned. */
export function RebaseEditor({ repo, base }: { repo: string; base: string | null }) {
  // Planned once: refreshing it would throw away the user's edits.
  const plan = useQuery({
    queryKey: [repo, "rebasePlan", base],
    queryFn: () => ipc.rebasePlan(repo, base),
    staleTime: Infinity,
    gcTime: 0,
  });
  const close = useCallback(() => updateView(repo, { rebase: null }), [repo]);

  if (!plan.data) {
    return (
      <div className="flex h-full flex-col">
        <Header onClose={close} title="Interactive rebase" />
        <EscapeCloses onClose={close} />
        <p
          className={clsx(
            "p-8 text-center select-text",
            plan.isError ? "text-danger" : "text-fg-faint",
          )}
        >
          {plan.isError ? String(plan.error) : "Loading…"}
        </p>
      </div>
    );
  }
  return <Editor key={plan.data.head} repo={repo} plan={plan.data} onClose={close} />;
}

function EscapeCloses({ onClose }: { onClose: () => void }) {
  useEscape(onClose);
  return null;
}

function Header({
  title,
  detail,
  onClose,
  children,
}: {
  title: string;
  detail?: string;
  onClose: () => void;
  children?: React.ReactNode;
}) {
  return (
    <header className="flex h-10 shrink-0 items-center gap-2 border-b border-line bg-surface px-2">
      <button
        type="button"
        onClick={onClose}
        aria-label="Cancel and go back to the graph"
        title="Cancel (Esc)"
        className="flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-hover hover:text-fg"
      >
        <ArrowLeft className="size-4" />
      </button>
      <span className="min-w-0 flex-1 truncate">
        <span className="font-medium">{title}</span>
        {detail && <span className="text-fg-muted"> {detail}</span>}
      </span>
      {children}
    </header>
  );
}

function Editor({ repo, plan, onClose }: { repo: string; plan: RebasePlan; onClose: () => void }) {
  const actions = useGitActions(repo);
  const info = useRepoInfo(repo).data;
  const [rows, setRows] = useState(() => initialRows(plan.commits));
  const [selected, setSelected] = useState(0);
  const [dragging, setDragging] = useState<{ from: number; to: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const edited = !unchanged(rows, plan.commits);

  // Leaving asks first once the plan has changes worth keeping.
  const cancel = useCallback(async () => {
    const ok =
      !edited ||
      (await confirm({
        title: "Discard the rebase plan",
        message: "Go back to the graph? The changes to this plan are lost.",
        confirmLabel: "Discard",
        danger: true,
      }));
    if (ok) onClose();
  }, [edited, onClose]);
  const escape = useCallback(() => void cancel(), [cancel]);
  useEscape(escape);

  const blocker =
    plan.commits.length === 0
      ? "There are no commits to rebase"
      : unchanged(rows, plan.commits)
        ? "Change something first"
        : problem(rows);
  const onto = plan.base ? shortRev(plan.base) : "the root";
  const branch = info ? describeHead(info.head) : "HEAD";

  const start = async () => {
    if (blocker || busy) return;
    setBusy(true);
    const ok = await actions.interactiveRebase(plan.base, plan.head, toSteps(rows));
    setBusy(false);
    if (ok) onClose();
  };

  const move = (from: number, to: number) => {
    if (to < 0 || to >= rows.length) return;
    setRows((r) => moveRow(r, from, to));
    setSelected(to);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const to = selected + (e.key === "ArrowDown" ? 1 : -1);
      if (e.altKey) move(selected, to);
      else setSelected(Math.max(0, Math.min(rows.length - 1, to)));
      return;
    }
    const choice = ACTIONS.find((a) => a.key === e.key.toLowerCase());
    if (choice && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      setRows((r) => setAction(r, selected, choice.action));
    }
  };

  // Pointer-driven reordering by the grip (HTML drag and drop is unreliable
  // in the webview).
  const beginDrag = (from: number, e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    setSelected(from);
    setDragging({ from, to: from });
    const target = (ev: PointerEvent) => {
      const el = document.elementFromPoint(ev.clientX, ev.clientY)?.closest("[data-row]");
      const index = el ? Number(el.getAttribute("data-row")) : null;
      return index ?? from;
    };
    const moveTo = (ev: PointerEvent) => setDragging({ from, to: target(ev) });
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", moveTo);
      window.removeEventListener("pointerup", up);
      setDragging(null);
      move(from, target(ev));
    };
    window.addEventListener("pointermove", moveTo);
    window.addEventListener("pointerup", up);
  };

  return (
    <div className="flex h-full flex-col">
      <Header
        onClose={() => void cancel()}
        title="Interactive rebase"
        detail={`of ${branch} onto ${onto}, newest commit first`}
      >
        <Button className="h-6 px-2 text-xs" onClick={() => void cancel()}>
          Cancel
        </Button>
        <Button
          variant="primary"
          className="h-6 px-2 text-xs"
          disabled={busy || blocker !== null}
          title={blocker ?? undefined}
          onClick={() => void start()}
        >
          {busy ? "Rebasing…" : "Start rebase"}
        </Button>
      </Header>
      <div className="shrink-0 space-y-1 border-b border-line bg-raised px-3 py-1.5 text-xs text-fg-muted">
        <p>
          Drag rows (or Alt+↑/↓) to reorder. Keys:{" "}
          {ACTIONS.map((a) => `${a.key} ${a.label.toLowerCase()}`).join(", ")}. Uncommitted changes
          are stashed and restored.
        </p>
        {plan.merges > 0 && (
          <p className="text-warning">
            {plan.merges} merge {plan.merges === 1 ? "commit is" : "commits are"} in this range;
            rebasing flattens {plan.merges === 1 ? "it" : "them"} away.
          </p>
        )}
        {blocker && !unchanged(rows, plan.commits) && <p className="text-warning">{blocker}</p>}
      </div>
      <ol
        role="listbox"
        aria-label="Commits to rebase"
        tabIndex={0}
        onKeyDown={onKeyDown}
        className="min-h-0 flex-1 overflow-y-auto outline-none"
      >
        {rows.map((row, i) => (
          <RowView
            key={row.commit.oid}
            row={row}
            index={i}
            count={rows.length}
            selected={i === selected}
            dropHere={dragging !== null && dragging.to === i && dragging.from !== i}
            dropBelow={dragging !== null && dragging.from < dragging.to}
            onSelect={() => setSelected(i)}
            onAction={(action) => setRows((r) => setAction(r, i, action))}
            onMessage={(message) =>
              setRows((r) => r.map((x, k) => (k === i ? { ...x, message } : x)))
            }
            onMove={(to) => move(i, to)}
            onGrip={(e) => beginDrag(i, e)}
          />
        ))}
        {plan.commits.length === 0 && (
          <li className="p-8 text-center text-fg-faint">
            Nothing to rebase: HEAD is already at {onto}.
          </li>
        )}
      </ol>
    </div>
  );
}

function RowView({
  row,
  index,
  count,
  selected,
  dropHere,
  dropBelow,
  onSelect,
  onAction,
  onMessage,
  onMove,
  onGrip,
}: {
  row: Row;
  index: number;
  count: number;
  selected: boolean;
  dropHere: boolean;
  dropBelow: boolean;
  onSelect: () => void;
  onAction: (action: RebaseAction) => void;
  onMessage: (message: string) => void;
  onMove: (to: number) => void;
  onGrip: (e: React.PointerEvent) => void;
}) {
  const { commit, action } = row;
  const melds = action === "squash" || action === "fixup";
  const edited =
    (action === "reword" || action === "squash") &&
    !!row.message.trim() &&
    row.message.trim() !== commit.message.trim();
  return (
    <li
      data-row={index}
      role="option"
      aria-selected={selected}
      onMouseDown={onSelect}
      className={clsx(
        "border-b border-line",
        selected ? "bg-accent/15" : "hover:bg-hover",
        dropHere &&
          (dropBelow
            ? "shadow-[inset_0_-2px_0_var(--rn-accent)]"
            : "shadow-[inset_0_2px_0_var(--rn-accent)]"),
      )}
    >
      <div className="flex h-9 items-center gap-2 pr-3 pl-1">
        <span
          onPointerDown={onGrip}
          title="Drag to reorder"
          className="flex h-full w-5 cursor-grab items-center justify-center text-fg-faint hover:text-fg"
        >
          <GripVertical className="size-4" />
        </span>
        <select
          value={action}
          onChange={(e) => onAction(e.target.value as RebaseAction)}
          aria-label={`Action for ${commit.oid.slice(0, 7)}`}
          title={ACTIONS.find((a) => a.action === action)?.hint}
          className="h-7 w-24 shrink-0 rounded-md border border-line bg-canvas px-1.5 text-xs text-fg outline-none focus:border-accent"
        >
          {ACTIONS.map((a) => (
            <option key={a.action} value={a.action}>
              {a.label}
            </option>
          ))}
        </select>
        {melds && (
          <CornerLeftDown
            className="size-3.5 shrink-0 text-fg-faint"
            aria-label="melds into the commit below"
          />
        )}
        <span className="shrink-0 font-mono text-xs text-fg-faint">{commit.oid.slice(0, 7)}</span>
        <span
          className={clsx(
            "min-w-0 flex-1 truncate",
            action === "drop" && "text-fg-faint line-through",
          )}
          title={commit.message}
        >
          {edited ? summary(row.message) : summary(commit.message)}
        </span>
        {edited && <span className="shrink-0 text-xs text-accent">new message</span>}
        <span className="w-36 shrink-0 truncate text-fg-muted">{commit.authorName}</span>
        <span className="w-24 shrink-0 truncate text-fg-muted" title={formatDate(commit.time)}>
          {relativeTime(commit.time)}
        </span>
        <span className="flex shrink-0">
          <IconButton label="Move up" disabled={index === 0} onClick={() => onMove(index - 1)}>
            <ArrowUp className="size-3.5" />
          </IconButton>
          <IconButton
            label="Move down"
            disabled={index === count - 1}
            onClick={() => onMove(index + 1)}
          >
            <ArrowDown className="size-3.5" />
          </IconButton>
        </span>
      </div>
      {(action === "reword" || action === "squash") && (
        <div className="px-8 pb-2">
          <textarea
            value={row.message}
            onChange={(e) => onMessage(e.target.value)}
            rows={3}
            // Switching a commit to reword is followed by typing its message.
            autoFocus={action === "reword"}
            aria-label={`New message for ${commit.oid.slice(0, 7)}`}
            placeholder={
              action === "squash"
                ? "Message for the combined commit (leave empty to keep both messages)"
                : "New commit message"
            }
            className="block w-full resize-y rounded-md border border-line bg-canvas px-2 py-1.5 outline-none focus:border-accent"
          />
        </div>
      )}
    </li>
  );
}

function IconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="flex size-6 items-center justify-center rounded-sm text-fg-muted hover:bg-raised hover:text-fg disabled:opacity-30"
    >
      {children}
    </button>
  );
}
