import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";

import { useOverlays } from "../../app/overlays";
import type { Config } from "../../bindings/Config";
import type { GitPrefs } from "../../bindings/GitPrefs";
import type { Portable } from "../../bindings/Portable";
import type { Profile } from "../../bindings/Profile";
import { ipc } from "../../lib/ipc";
import { toast } from "../../ui/toast-store";
import { keys } from "../workspace/queries";

export const syncKeys = {
  status: ["sync"] as const,
};

export const settingsKeys = {
  identity: ["gitIdentity"] as const,
  sshKeys: ["sshKeys"] as const,
};

/** Updates part of the portable settings in the cache and saves it. */
function usePortableMutation<T>(
  save: (value: T) => Promise<unknown>,
  patch: (portable: Portable, value: T) => Portable,
) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: save,
    onMutate: (value: T) => {
      client.setQueryData<Config>(keys.config, (c) =>
        c ? { ...c, portable: patch(c.portable, value) } : c,
      );
    },
    onSuccess: (warning) => {
      if (typeof warning === "string") toast.error("Profile not fully applied", warning);
    },
    onError: (err) => {
      toast.error("Could not save settings", String(err));
      void client.invalidateQueries({ queryKey: keys.config });
    },
  });
}

export function useSetGitPrefs() {
  return usePortableMutation(ipc.configSetGit, (p, git: GitPrefs) => ({ ...p, git }));
}

export function useSetKeybindings() {
  return usePortableMutation(
    ipc.configSetKeybindings,
    (p, keybindings: Record<string, string>) => ({
      ...p,
      keybindings,
    }),
  );
}

export function useSetProfiles() {
  return usePortableMutation(ipc.configSetProfiles, (p, profiles: Profile[]) => ({
    ...p,
    profiles,
  }));
}

export function useSetTerminalShell() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ipc.configSetTerminalShell,
    onMutate: (terminalShell: string) => {
      client.setQueryData<Config>(keys.config, (c) =>
        c ? { ...c, local: { ...c.local, terminalShell } } : c,
      );
    },
    onError: (err) => toast.error("Could not save settings", String(err)),
  });
}

/** Who git commits as without a profile's overrides. */
export function useGitIdentity() {
  return useQuery({ queryKey: settingsKeys.identity, queryFn: ipc.gitIdentity });
}

export function useSshKeys() {
  return useQuery({ queryKey: settingsKeys.sshKeys, queryFn: ipc.sshKeys });
}

export function useSyncStatus() {
  return useQuery({ queryKey: syncKeys.status, queryFn: ipc.syncStatus });
}

const FILTERS = [{ name: "Git Ronin settings", extensions: ["toml"] }];

/** Asks where to, then writes the portable settings there. */
export async function exportSettings() {
  const path = await save({
    title: "Export settings",
    defaultPath: "git-ronin.ronin.toml",
    filters: FILTERS,
  });
  if (!path) return;
  try {
    await ipc.settingsExport(path);
    toast.success("Settings exported", path);
  } catch (err) {
    toast.error("Could not export settings", String(err));
  }
}

/** Asks for a settings file, then shows what importing it would change. */
export async function importSettings() {
  const path = await open({ title: "Import settings", filters: FILTERS, multiple: false });
  if (path) useOverlays.setState({ importPath: path, palette: false });
}

/** A new profile id that isn't taken. */
export function newProfileId(profiles: Profile[]): string {
  for (let i = 1; ; i++) {
    const id = `profile-${i}`;
    if (!profiles.some((p) => p.id === id)) return id;
  }
}
