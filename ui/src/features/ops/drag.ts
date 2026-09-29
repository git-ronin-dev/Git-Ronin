import { create } from "zustand";

import type { GitActions } from "./actions";

/** A branch that can be dragged or dropped on. */
export interface DragRef {
  kind: "local" | "remote";
  /** Short name: `main`, or `origin/main` for a remote branch. */
  name: string;
  fullName: string;
  oid: string;
  /** For remote branches: the remote, and the branch name on it. */
  remote?: string;
  branch?: string;
}

export interface DropAction {
  label: string;
  run: (a: GitActions) => Promise<unknown>;
}

/**
 * What dropping `source` onto `target` can do. Merge and rebase act on the
 * checked-out branch (`head`), so when neither side is checked out the
 * action checks one out first.
 */
export function dropActions(source: DragRef, target: DragRef, head: string | null): DropAction[] {
  if (source.fullName === target.fullName) return [];
  const s = source.name;
  const t = target.name;

  if (target.kind === "remote") {
    if (source.kind !== "local" || !target.remote || !target.branch) return [];
    const push = { remote: target.remote, branch: target.branch };
    return [{ label: `Push ${s} to ${t}`, run: (a) => a.pushTo(s, push) }];
  }

  if (target.name === head) {
    return [
      { label: `Merge ${s} into ${t}`, run: (a) => a.merge(s, t) },
      { label: `Rebase ${t} onto ${s}`, run: (a) => a.rebase(s, t) },
      { label: `Reset ${t} to ${s}`, run: (a) => a.reset(source.oid, "mixed", t) },
    ];
  }

  if (source.kind === "local" && source.name === head) {
    return [
      { label: `Merge ${t} into ${s}`, run: (a) => a.merge(t, s) },
      { label: `Rebase ${s} onto ${t}`, run: (a) => a.rebase(t, s) },
      { label: `Reset ${s} to ${t}`, run: (a) => a.reset(target.oid, "mixed", s) },
    ];
  }

  const actions: DropAction[] = [
    {
      label: `Check out ${t} and merge ${s} into it`,
      run: async (a) => (await a.checkout(t)) && a.merge(s, t),
    },
  ];
  if (source.kind === "local") {
    actions.push({
      label: `Check out ${s} and rebase it onto ${t}`,
      run: async (a) => (await a.checkout(s)) && a.rebase(t, s),
    });
  }
  actions.push({ label: `Move ${t} to ${s}`, run: (a) => a.moveBranch(t, source.oid) });
  return actions;
}

interface DragState {
  repo: string | null;
  /** Being dragged, with the pointer position. */
  dragging: { source: DragRef; x: number; y: number } | null;
  /** Full name of the drop target under the pointer. */
  over: string | null;
  /** Dropped, waiting for the user to pick an action. */
  dropped: { source: DragRef; target: DragRef; x: number; y: number } | null;
}

export const useDrag = create<DragState>()(() => ({
  repo: null,
  dragging: null,
  over: null,
  dropped: null,
}));

/** Pointer movement before a press becomes a drag. */
const THRESHOLD = 5;

/** Attribute marking a drop target; its value is the JSON of a `DragRef`. */
export const DROP_ATTR = "data-drop-ref";

export function dropProps(ref: DragRef) {
  return { [DROP_ATTR]: JSON.stringify(ref) };
}

function targetAt(x: number, y: number): DragRef | null {
  const el = document.elementFromPoint(x, y)?.closest(`[${DROP_ATTR}]`);
  const json = el?.getAttribute(DROP_ATTR);
  if (!json) return null;
  try {
    return JSON.parse(json) as DragRef;
  } catch {
    return null;
  }
}

/** Starts tracking a possible drag of `source` from a pointer press. */
export function beginDrag(
  repo: string,
  source: DragRef,
  e: { button: number; clientX: number; clientY: number },
) {
  if (e.button !== 0) return;
  const start = { x: e.clientX, y: e.clientY };
  let active = false;

  const move = (ev: PointerEvent) => {
    if (!active) {
      if (Math.hypot(ev.clientX - start.x, ev.clientY - start.y) < THRESHOLD) return;
      active = true;
      document.body.classList.add("dragging");
    }
    const over = targetAt(ev.clientX, ev.clientY);
    useDrag.setState({
      repo,
      dragging: { source, x: ev.clientX, y: ev.clientY },
      over: over && over.fullName !== source.fullName ? over.fullName : null,
    });
  };
  const stop = (ev: PointerEvent | null) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("keydown", key);
    document.body.classList.remove("dragging");
    const target = ev && active ? targetAt(ev.clientX, ev.clientY) : null;
    useDrag.setState({
      dragging: null,
      over: null,
      dropped:
        target && ev && target.fullName !== source.fullName
          ? { source, target, x: ev.clientX, y: ev.clientY }
          : null,
    });
    if (active) {
      // The press ended a drag, not a click. Browsers send no click when
      // the release lands elsewhere, so stop waiting for one soon.
      const swallow = (c: Event) => c.stopPropagation();
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, true), 100);
    }
  };
  const up = (ev: PointerEvent) => stop(ev);
  const key = (ev: KeyboardEvent) => ev.key === "Escape" && stop(null);

  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("keydown", key);
}
