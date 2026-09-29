import { create } from "zustand";

export type ToastKind = "info" | "success" | "error";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  title: string;
  description?: string;
}

interface ToastState {
  items: ToastItem[];
  push: (item: Omit<ToastItem, "id">) => void;
  dismiss: (id: number) => void;
}

let nextId = 0;

export const useToasts = create<ToastState>()((set) => ({
  items: [],
  push: (item) => set((s) => ({ items: [...s.items, { ...item, id: nextId++ }] })),
  dismiss: (id) => set((s) => ({ items: s.items.filter((t) => t.id !== id) })),
}));

const show = (kind: ToastKind) => (title: string, description?: string) =>
  useToasts.getState().push({ kind, title, description });

export const toast = { info: show("info"), success: show("success"), error: show("error") };
