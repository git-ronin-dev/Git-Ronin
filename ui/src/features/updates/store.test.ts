import { beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdateProgress } from "../../bindings/UpdateProgress";
import { ipc } from "../../lib/ipc";
import { confirm } from "../../ui/confirm-store";
import { useToasts } from "../../ui/toast-store";
import { checkForUpdates, installUpdate, useUpdates } from "./store";

vi.mock("../../lib/ipc", () => ({ ipc: { updateCheck: vi.fn(), updateInstall: vi.fn() } }));
vi.mock("../../ui/confirm-store", () => ({ confirm: vi.fn() }));

const update = { version: "0.2.0", notes: "Faster graph", date: null };

describe("updates", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useUpdates.setState({ phase: "idle" }, true);
    useToasts.setState({ items: [] });
  });

  it("stay quiet when a background check fails", async () => {
    vi.mocked(ipc.updateCheck).mockRejectedValue("offline");
    await checkForUpdates(false);
    expect(useUpdates.getState()).toEqual({ phase: "failed", message: "offline" });
    expect(useToasts.getState().items).toEqual([]);
  });

  it("say so when a manual check finds nothing", async () => {
    vi.mocked(ipc.updateCheck).mockResolvedValue(null);
    await checkForUpdates(true);
    expect(useUpdates.getState().phase).toBe("current");
    expect(useToasts.getState().items[0]?.title).toBe("Git Ronin is up to date");
  });

  it("install only once the user agrees", async () => {
    vi.mocked(ipc.updateCheck).mockResolvedValue(update);
    await checkForUpdates(false);
    expect(useUpdates.getState()).toEqual({ phase: "available", update });

    vi.mocked(confirm).mockResolvedValue(false);
    await installUpdate();
    expect(ipc.updateInstall).not.toHaveBeenCalled();

    const seen: string[] = [];
    vi.mocked(confirm).mockResolvedValue(true);
    vi.mocked(ipc.updateInstall).mockImplementation((onProgress: (p: UpdateProgress) => void) => {
      onProgress({ kind: "downloading", downloaded: 50, total: 100 });
      seen.push(useUpdates.getState().phase);
      onProgress({ kind: "installing" });
      seen.push(useUpdates.getState().phase);
      return Promise.reject(new Error("pkexec was dismissed"));
    });
    await installUpdate();
    expect(vi.mocked(confirm).mock.lastCall?.[0].message).toContain("Faster graph");
    expect(seen).toEqual(["downloading", "installing"]);
    // A failed install leaves the update to try again.
    expect(useUpdates.getState().phase).toBe("available");
    expect(useToasts.getState().items[0]?.title).toBe("Could not install the update");
  });
});
