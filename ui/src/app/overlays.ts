import { create } from "zustand";

export type SettingsSection =
  "general" | "diffs" | "git" | "profiles" | "keyboard" | "ssh" | "sync";

/** App-wide dialogs and panels any command can open. */
interface OverlayState {
  palette: boolean;
  settings: SettingsSection | null;
  workspaces: boolean;
  /** Repository whose stash dialog is open. */
  stash: string | null;
  /** A settings file being imported. */
  importPath: string | null;
  /** Hooks for panels the shell owns; set by `AppShell`. */
  toggleSidebar: () => void;
  toggleDetails: () => void;
}

export const useOverlays = create<OverlayState>()(() => ({
  palette: false,
  settings: null,
  workspaces: false,
  stash: null,
  importPath: null,
  toggleSidebar: () => {},
  toggleDetails: () => {},
}));

export function openSettings(section: SettingsSection = "general") {
  useOverlays.setState({ settings: section, palette: false });
}
