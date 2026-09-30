import { create } from "zustand";

import type { AvailableUpdate } from "../../bindings/AvailableUpdate";
import { ipc } from "../../lib/ipc";
import { confirm } from "../../ui/confirm-store";
import { toast } from "../../ui/toast-store";

export type UpdatePhase =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "current"; checkedAt: number }
  | { phase: "available"; update: AvailableUpdate }
  | { phase: "downloading"; update: AvailableUpdate; downloaded: number; total: number | null }
  | { phase: "installing"; update: AvailableUpdate }
  | { phase: "failed"; message: string };

export const useUpdates = create<UpdatePhase>()(() => ({ phase: "idle" }));

const busy = () => {
  const { phase } = useUpdates.getState();
  return phase === "checking" || phase === "downloading" || phase === "installing";
};

/**
 * Asks the release channel for a newer version. A background check fails
 * quietly (Settings → General shows why); a manual one says what it found.
 */
export async function checkForUpdates(manual: boolean) {
  if (busy()) return;
  useUpdates.setState({ phase: "checking" }, true);
  try {
    const update = await ipc.updateCheck();
    if (update) {
      useUpdates.setState({ phase: "available", update }, true);
      if (manual) void installUpdate();
    } else {
      useUpdates.setState({ phase: "current", checkedAt: Date.now() }, true);
      if (manual) toast.success("Git Ronin is up to date");
    }
  } catch (err) {
    useUpdates.setState({ phase: "failed", message: String(err) }, true);
    if (manual) toast.error("Could not check for updates", String(err));
  }
}

/** Asks, then downloads and installs the update found, and restarts. */
export async function installUpdate() {
  const state = useUpdates.getState();
  if (state.phase !== "available") return;
  const { update } = state;
  const notes = update.notes.trim();
  const ok = await confirm({
    title: `Update to Git Ronin ${update.version}?`,
    message:
      (notes ? `${notes.length > 1200 ? `${notes.slice(0, 1200)}…` : notes}\n\n` : "") +
      "Git Ronin restarts once the update is installed. Anything running in the terminal panel stops.",
    confirmLabel: "Install and restart",
  });
  if (!ok) return;
  useUpdates.setState({ phase: "downloading", update, downloaded: 0, total: null }, true);
  try {
    await ipc.updateInstall((p) => {
      if (p.kind === "downloading")
        useUpdates.setState(
          { phase: "downloading", update, downloaded: p.downloaded, total: p.total },
          true,
        );
      else useUpdates.setState({ phase: "installing", update }, true);
    });
  } catch (err) {
    useUpdates.setState({ phase: "available", update }, true);
    toast.error("Could not install the update", String(err));
  }
}
