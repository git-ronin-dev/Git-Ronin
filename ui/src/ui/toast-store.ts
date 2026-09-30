import { create } from "zustand";

import { play } from "../lib/sound";

export type ToastKind = "info" | "success" | "error";

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface ToastItem {
  id: number;
  kind: ToastKind;
  title: string;
  description?: string;
  /** Stays until dismissed, like errors. */
  persist?: boolean;
  /** A button that runs something and dismisses the toast. */
  action?: ToastAction;
}

interface ToastState {
  items: ToastItem[];
  push: (item: Omit<ToastItem, "id">) => void;
  dismiss: (id: number) => void;
}

let nextId = 0;

export const useToasts = create<ToastState>()((set) => ({
  items: [],
  push: (item) => {
    if (item.kind !== "info") play(item.kind);
    set((s) => ({ items: [...s.items, { ...item, id: nextId++ }] }));
  },
  dismiss: (id) => set((s) => ({ items: s.items.filter((t) => t.id !== id) })),
}));

const show =
  (kind: ToastKind, persist = false) =>
  (title: string, description?: string, action?: ToastAction) =>
    useToasts.getState().push({ kind, title, description, persist, action });

export const toast = {
  info: show("info"),
  success: show("success"),
  error: show("error"),
  /** Output worth reading, e.g. from git hooks: stays until dismissed. */
  output: show("info", true),
};
