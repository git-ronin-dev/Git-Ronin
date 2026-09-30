import { beforeEach, describe, expect, it, vi } from "vitest";

import { ipc } from "../../lib/ipc";
import { config, repoInfo } from "../../test/fixtures";
import { useToasts } from "../../ui/toast-store";
import { useWorkspace } from "./store";

vi.mock("../../lib/ipc", () => ({
  ipc: {
    openRepo: vi.fn(),
    closeRepo: vi.fn(),
    setActiveTab: vi.fn(),
    restoreTabs: vi.fn(),
    configGet: vi.fn(),
    takeLaunchPaths: vi.fn(),
  },
}));

const other = { ...repoInfo, path: "/work/other", name: "other" };
const third = { ...repoInfo, path: "/work/third", name: "third" };

describe("useWorkspace", () => {
  beforeEach(() => {
    vi.mocked(ipc.closeRepo).mockResolvedValue();
    vi.mocked(ipc.setActiveTab).mockResolvedValue();
    vi.mocked(ipc.takeLaunchPaths).mockResolvedValue([]);
    useWorkspace.setState({ tabs: [], active: null, opening: false });
    useToasts.setState({ items: [] });
  });

  it("opens a repository as the active tab, once", async () => {
    vi.mocked(ipc.openRepo).mockResolvedValue(repoInfo);
    await useWorkspace.getState().open("/work/ronin/src");
    await useWorkspace.getState().open("/work/ronin");
    expect(useWorkspace.getState()).toMatchObject({
      tabs: [{ path: "/work/ronin", name: "ronin" }],
      active: "/work/ronin",
      opening: false,
    });
  });

  it("reports failures as an error toast", async () => {
    vi.mocked(ipc.openRepo).mockRejectedValue("not a git repository: /tmp");
    await useWorkspace.getState().open("/tmp");
    expect(useWorkspace.getState().tabs).toEqual([]);
    expect(useToasts.getState().items).toEqual([
      expect.objectContaining({ kind: "error", description: "not a git repository: /tmp" }),
    ]);
  });

  it("activates the neighbour when closing the active tab", async () => {
    const tabs = [repoInfo, other, third].map(({ path, name }) => ({ path, name }));
    useWorkspace.setState({ tabs, active: "/work/other" });
    await useWorkspace.getState().close("/work/other");
    expect(useWorkspace.getState().active).toBe("/work/third");
    await useWorkspace.getState().close("/work/third");
    expect(useWorkspace.getState().active).toBe("/work/ronin");
    expect(ipc.closeRepo).toHaveBeenCalledWith("/work/third");
  });

  it("restores tabs and the saved active tab", async () => {
    vi.mocked(ipc.restoreTabs).mockResolvedValue([repoInfo, other]);
    vi.mocked(ipc.configGet).mockResolvedValue({
      ...config,
      local: { ...config.local, activeTab: "/work/other" },
    });
    await useWorkspace.getState().restore();
    expect(useWorkspace.getState().tabs.map((t) => t.name)).toEqual(["ronin", "other"]);
    expect(useWorkspace.getState().active).toBe("/work/other");
  });

  it("opens the paths the app was started with after restoring", async () => {
    vi.mocked(ipc.restoreTabs).mockResolvedValue([repoInfo]);
    vi.mocked(ipc.configGet).mockResolvedValue(config);
    vi.mocked(ipc.takeLaunchPaths).mockResolvedValue(["/work/third/src"]);
    vi.mocked(ipc.openRepo).mockResolvedValue(third);
    await useWorkspace.getState().restore();
    expect(ipc.openRepo).toHaveBeenCalledWith("/work/third/src");
    expect(useWorkspace.getState().tabs.map((t) => t.name)).toEqual(["ronin", "third"]);
    expect(useWorkspace.getState().active).toBe("/work/third");
  });
});
