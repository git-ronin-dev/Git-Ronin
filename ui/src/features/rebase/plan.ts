import type { RebaseAction } from "../../bindings/RebaseAction";
import type { RebaseCommit } from "../../bindings/RebaseCommit";
import type { RebaseStep } from "../../bindings/RebaseStep";

/** One commit in the editor. Rows are shown newest first, like the graph. */
export interface Row {
  commit: RebaseCommit;
  action: RebaseAction;
  /** The new message, for reword (required) and squash (optional). */
  message: string;
}

export const ACTIONS: { action: RebaseAction; label: string; key: string; hint: string }[] = [
  { action: "pick", label: "Pick", key: "p", hint: "Keep the commit" },
  { action: "reword", label: "Reword", key: "r", hint: "Keep it with a new message" },
  { action: "edit", label: "Edit", key: "e", hint: "Stop after it, to amend it" },
  {
    action: "squash",
    label: "Squash",
    key: "s",
    hint: "Meld into the commit below, combining messages",
  },
  {
    action: "fixup",
    label: "Fixup",
    key: "f",
    hint: "Meld into the commit below, keeping its message",
  },
  { action: "drop", label: "Drop", key: "d", hint: "Leave the commit out" },
];

/** Rows for `commits` (oldest first, as planned), newest first. */
export function initialRows(commits: RebaseCommit[]): Row[] {
  return [...commits].reverse().map((commit) => ({ commit, action: "pick", message: "" }));
}

export function summary(message: string): string {
  return message.split("\n", 1)[0] ?? "";
}

/** Sets a row's action, starting a reword from the commit's own message. */
export function setAction(rows: Row[], index: number, action: RebaseAction): Row[] {
  return rows.map((row, i) => {
    if (i !== index) return row;
    const message = action === "reword" && !row.message.trim() ? row.commit.message : row.message;
    return { ...row, action, message };
  });
}

/** Moves the row at `from` to `to`. */
export function moveRow(rows: Row[], from: number, to: number): Row[] {
  if (from === to || to < 0 || to >= rows.length) return rows;
  const next = [...rows];
  const [row] = next.splice(from, 1);
  next.splice(to, 0, row!);
  return next;
}

/** Why the plan can't run, or null. */
export function problem(rows: Row[]): string | null {
  const oldestKept = [...rows].reverse().find((r) => r.action !== "drop");
  if (!oldestKept) return "Keep at least one commit (to remove them all, reset instead)";
  if (oldestKept.action === "squash" || oldestKept.action === "fixup") {
    return "The oldest commit kept can't be squashed: there is nothing below it to meld into";
  }
  if (rows.some((r) => r.action === "reword" && !r.message.trim())) {
    return "Write a message for each reworded commit";
  }
  return null;
}

/** Whether the plan would change nothing. */
export function unchanged(rows: Row[], commits: RebaseCommit[]): boolean {
  const newestFirst = [...commits].reverse();
  return rows.every((row, i) => row.action === "pick" && row.commit.oid === newestFirst[i]?.oid);
}

/** The plan as git runs it: oldest first. */
export function toSteps(rows: Row[]): RebaseStep[] {
  return [...rows].reverse().map((row) => ({
    oid: row.commit.oid,
    action: row.action,
    message:
      (row.action === "reword" || row.action === "squash") && row.message.trim()
        ? row.message.trim()
        : null,
  }));
}
