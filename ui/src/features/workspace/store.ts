import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";

import type { RepoInfo } from "../../bindings/RepoInfo";
import { ipc } from "../../lib/ipc";
import { toast } from "../../ui/toast-store";

export interface Tab {
  path: string;
  name: string;
}

interface WorkspaceState {
  tabs: Tab[];
  active: string | null;
  opening: boolean;
  /** Reopens the tabs of the previous session. */
  restore: () => Promise<void>;
  open: (path: string) => Promise<void>;
  /** Opens repositories named on the command line, one after the other. */
  openAll: (paths: string[]) => Promise<void>;
  /** Shows a folder picker, then opens the chosen repository. */
  pickAndOpen: () => Promise<void>;
  /** Shows a folder picker, then creates a repository there and opens it. */
  pickAndInit: () => Promise<void>;
  /** Shows a repository the backend already opened (after a clone or init). */
  adopt: (info: RepoInfo) => void;
  close: (path: string) => Promise<void>;
  activate: (path: string) => void;
  /** Switches profile and shows its tabs. */
  switchProfile: (id: string) => Promise<void>;
  /** Called after a profile switch, so settings and data can be reloaded. */
  onProfileSwitch: () => void;
}

const reportError = (title: string) => (err: unknown) => toast.error(title, String(err));

export const useWorkspace = create<WorkspaceState>()((set, get) => ({
  tabs: [],
  active: null,
  opening: false,

  restore: async () => {
    try {
      const [infos, config] = await Promise.all([ipc.restoreTabs(), ipc.configGet()]);
      const tabs = infos.map(({ path, name }) => ({ path, name }));
      const saved = config.local.activeTab;
      const active = tabs.some((t) => t.path === saved) ? saved : (tabs[0]?.path ?? null);
      set({ tabs, active });
    } catch (err) {
      reportError("Could not restore open repositories")(err);
    }
    await get().openAll(await ipc.takeLaunchPaths().catch(() => []));
  },

  openAll: async (paths) => {
    for (const path of paths) await get().open(path);
  },

  open: async (path) => {
    set({ opening: true });
    try {
      get().adopt(await ipc.openRepo(path));
    } catch (err) {
      reportError("Could not open repository")(err);
    } finally {
      set({ opening: false });
    }
  },

  pickAndOpen: async () => {
    const path = await openDialog({ directory: true, title: "Open repository" });
    if (path) await get().open(path);
  },

  pickAndInit: async () => {
    const path = await openDialog({ directory: true, title: "Create repository in" });
    if (!path) return;
    set({ opening: true });
    try {
      get().adopt(await ipc.initRepo(path));
    } catch (err) {
      reportError("Could not create repository")(err);
    } finally {
      set({ opening: false });
    }
  },

  adopt: ({ path, name }) =>
    set((s) => ({
      tabs: s.tabs.some((t) => t.path === path) ? s.tabs : [...s.tabs, { path, name }],
      active: path,
    })),

  close: async (path) => {
    const { tabs, active } = get();
    const index = tabs.findIndex((t) => t.path === path);
    const remaining = tabs.filter((t) => t.path !== path);
    // Closing the active tab activates its neighbour, like a browser.
    const next =
      active === path ? (remaining[Math.min(index, remaining.length - 1)]?.path ?? null) : active;
    set({ tabs: remaining, active: next });
    await ipc.closeRepo(path).catch(reportError("Could not close repository"));
    if (next !== active) void ipc.setActiveTab(next).catch(() => {});
  },

  activate: (path) => {
    set({ active: path });
    void ipc.setActiveTab(path).catch(() => {});
  },

  switchProfile: async (id) => {
    set({ opening: true });
    try {
      const warning = await ipc.profileActivate(id);
      if (warning) toast.error("Profile not fully applied", warning);
      set({ tabs: [], active: null });
      get().onProfileSwitch();
      await get().restore();
    } catch (err) {
      reportError("Could not switch profile")(err);
    } finally {
      set({ opening: false });
    }
  },

  onProfileSwitch: () => {},
}));
