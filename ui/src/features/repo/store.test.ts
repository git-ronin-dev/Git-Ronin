import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RepoInfo } from "../../bindings/RepoInfo";
import { ipc } from "../../lib/ipc";
import { useToasts } from "../../ui/toast-store";
import { useRepoStore } from "./store";

vi.mock("../../lib/ipc", () => ({ ipc: { openRepo: vi.fn(), gitVersion: vi.fn() } }));

const repo: RepoInfo = {
  path: "/work/ronin",
  name: "ronin",
  isBare: false,
  head: { kind: "branch", name: "main", unborn: false },
};

describe("useRepoStore.open", () => {
  beforeEach(() => {
    useRepoStore.setState({ repo: null, opening: false });
    useToasts.setState({ items: [] });
  });

  it("stores the opened repository", async () => {
    vi.mocked(ipc.openRepo).mockResolvedValue(repo);
    await useRepoStore.getState().open("/work/ronin");
    expect(useRepoStore.getState()).toMatchObject({ repo, opening: false });
  });

  it("reports failures as an error toast", async () => {
    vi.mocked(ipc.openRepo).mockRejectedValue("not a git repository: /tmp");
    await useRepoStore.getState().open("/tmp");
    expect(useRepoStore.getState()).toMatchObject({ repo: null, opening: false });
    expect(useToasts.getState().items).toEqual([
      expect.objectContaining({ kind: "error", description: "not a git repository: /tmp" }),
    ]);
  });
});
