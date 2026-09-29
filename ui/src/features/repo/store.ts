import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";

import type { RepoInfo } from "../../bindings/RepoInfo";
import { ipc } from "../../lib/ipc";
import { toast } from "../../ui/toast-store";

interface RepoState {
  repo: RepoInfo | null;
  opening: boolean;
  open: (path: string) => Promise<void>;
  /** Shows a folder picker, then opens the chosen repository. */
  pickAndOpen: () => Promise<void>;
  close: () => void;
}

export const useRepoStore = create<RepoState>()((set, get) => ({
  repo: null,
  opening: false,
  open: async (path) => {
    set({ opening: true });
    try {
      set({ repo: await ipc.openRepo(path) });
    } catch (err) {
      toast.error("Could not open repository", String(err));
    } finally {
      set({ opening: false });
    }
  },
  pickAndOpen: async () => {
    const path = await openDialog({ directory: true, title: "Open repository" });
    if (path) await get().open(path);
  },
  close: () => set({ repo: null }),
}));
