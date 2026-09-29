import { create } from "zustand";

/** A long-running command the user is waiting for. */
export interface Task {
  id: number;
  label: string;
  /** Latest progress line from git. */
  message: string | null;
  percent: number | null;
}

interface TaskState {
  /** Keyed by repository path (or clone destination). */
  tasks: Record<string, Task>;
  progress: (key: string, message: string, percent: number | null) => void;
}

let nextId = 0;

export const useTasks = create<TaskState>()((set) => ({
  tasks: {},
  progress: (key, message, percent) =>
    set((s) => {
      const task = s.tasks[key];
      return task ? { tasks: { ...s.tasks, [key]: { ...task, message, percent } } } : s;
    }),
}));

/** Shows `label` as running for `key`; call the result when it ends. */
export function startTask(key: string, label: string): () => void {
  const id = nextId++;
  useTasks.setState((s) => ({
    tasks: { ...s.tasks, [key]: { id, label, message: null, percent: null } },
  }));
  return () =>
    useTasks.setState((s) => {
      if (s.tasks[key]?.id !== id) return s;
      const tasks = { ...s.tasks };
      delete tasks[key];
      return { tasks };
    });
}

export function useTask(key: string): Task | undefined {
  return useTasks((s) => s.tasks[key]);
}
