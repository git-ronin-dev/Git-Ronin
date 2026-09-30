import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RepoLink } from "../../bindings/RepoLink";
import { ipc } from "../../lib/ipc";
import { refs, repoInfo } from "../../test/fixtures";
import type { MenuItem } from "../../ui/ContextMenu";
import { useConfirm } from "../../ui/confirm-store";
import { useToasts } from "../../ui/toast-store";
import { keys } from "../workspace/queries";
import { useViews, WORKING_COPY } from "../workspace/view";
import { gitActions, shortRev, type GitActions } from "./actions";
import { useDialogs } from "./dialog-store";
import { dropActions, type DragRef } from "./drag";
import { commitMenu, localBranchMenu, repoContext, tagMenu } from "./menus";
import { startTask, useTasks } from "./tasks";

vi.mock("../../lib/ipc", () => ({
  ipc: {
    pushBranch: vi.fn(),
    merge: vi.fn(),
    interactiveRebase: vi.fn(),
    deleteBranch: vi.fn(),
    checkoutBranch: vi.fn(),
  },
}));

const oid = "c".repeat(40);
const local = (name: string): DragRef => ({
  kind: "local",
  name,
  fullName: `refs/heads/${name}`,
  oid,
});
const remote: DragRef = {
  kind: "remote",
  name: "origin/main",
  fullName: "refs/remotes/origin/main",
  oid,
  remote: "origin",
  branch: "main",
};

/** Every action as a spy resolving to true. */
function fakeActions(): GitActions {
  return new Proxy({} as Record<string, unknown>, {
    get: (target, key: string) => (target[key] ??= vi.fn().mockResolvedValue(true)),
  }) as unknown as GitActions;
}

const labels = (items: MenuItem[]) => items.map((i) => (i === "separator" ? "—" : i.label));
const find = (items: MenuItem[], label: string) =>
  items.find((i): i is Exclude<MenuItem, "separator"> => i !== "separator" && i.label === label)!;

/** Answers the next confirm dialog. */
async function answerConfirm(ok: boolean) {
  await vi.waitFor(() => expect(useConfirm.getState().request).not.toBeNull());
  useConfirm.getState().settle(ok);
}

describe("dropActions", () => {
  it("merges into or rebases the checked-out branch", () => {
    const a = fakeActions();
    const onHead = dropActions(local("feature"), local("main"), "main");
    expect(onHead.map((x) => x.label)).toEqual([
      "Merge feature into main",
      "Rebase main onto feature",
      "Reset main to feature",
    ]);
    void onHead[1]!.run(a);
    expect(a.rebase).toHaveBeenCalledWith("feature", "main");

    const fromHead = dropActions(local("main"), local("feature"), "main");
    expect(fromHead.map((x) => x.label)).toEqual([
      "Merge feature into main",
      "Rebase main onto feature",
      "Reset main to feature",
    ]);
    void fromHead[2]!.run(a);
    expect(a.reset).toHaveBeenCalledWith(oid, "mixed", "main");
  });

  it("checks out first when neither branch is checked out", async () => {
    const a = fakeActions();
    const actions = dropActions(local("a"), local("b"), "main");
    expect(actions.map((x) => x.label)).toEqual([
      "Check out b and merge a into it",
      "Check out a and rebase it onto b",
      "Move b to a",
    ]);
    await actions[0]!.run(a);
    expect(a.checkout).toHaveBeenCalledWith("b");
    expect(a.merge).toHaveBeenCalledWith("a", "b");

    // A remote branch can be merged in, but not checked out and rebased.
    expect(dropActions(remote, local("b"), null).map((x) => x.label)).toEqual([
      "Check out b and merge origin/main into it",
      "Move b to origin/main",
    ]);
  });

  it("moves or rebases a branch dropped on a commit", async () => {
    const a = fakeActions();
    const commit = (id: string): DragRef => ({
      kind: "commit",
      name: id.slice(0, 7),
      fullName: id,
      oid: id,
    });
    const target = commit("d".repeat(40));
    const onHead = dropActions(local("main"), target, "main");
    expect(onHead.map((x) => x.label)).toEqual([
      "Rebase main onto ddddddd",
      "Reset main to ddddddd",
    ]);
    void onHead[0]!.run(a);
    expect(a.rebase).toHaveBeenCalledWith(target.oid, "main");

    const other = dropActions(local("topic"), target, "main");
    expect(other.map((x) => x.label)).toEqual([
      "Move topic to ddddddd",
      "Check out topic and rebase it onto ddddddd",
    ]);
    await other[1]!.run(a);
    expect(a.checkout).toHaveBeenCalledWith("topic");
    // Its own commit, or a remote branch: nothing to do.
    expect(dropActions(local("topic"), commit(oid), "main")).toEqual([]);
    expect(dropActions(remote, target, "main")).toEqual([]);
  });

  it("pushes a local branch dropped on a remote one", () => {
    const a = fakeActions();
    const [push] = dropActions(local("feature"), remote, "main");
    expect(push!.label).toBe("Push feature to origin/main");
    void push!.run(a);
    expect(a.pushTo).toHaveBeenCalledWith("feature", { remote: "origin", branch: "main" });
    expect(dropActions(remote, remote, "main")).toEqual([]);
    expect(dropActions(local("x"), local("x"), "main")).toEqual([]);
  });
});

describe("menus", () => {
  const ctx = repoContext("/work/ronin", repoInfo, refs);

  it("offers branch actions relative to the checked-out branch", () => {
    const a = fakeActions();
    const [main, feature] = refs.local;
    const mainMenu = localBranchMenu(a, ctx, main!);
    expect(find(mainMenu, "Check out main").disabled).toBe(true);
    expect(find(mainMenu, "Delete…").disabled).toBe(true);
    expect(labels(mainMenu)).toContain("Push to origin/main");
    expect(labels(mainMenu)).not.toContain("Merge main into main");

    const featureMenu = localBranchMenu(a, ctx, feature!);
    expect(labels(featureMenu)).toContain("Merge feature/login into main");
    expect(labels(featureMenu)).toContain("Push…");
    find(featureMenu, "Rebase main onto feature/login").onSelect();
    expect(a.rebase).toHaveBeenCalledWith("feature/login", "main");
    find(featureMenu, "Rename…").onSelect();
    expect(useDialogs.getState().request).toEqual({
      kind: "renameBranch",
      repo: "/work/ronin",
      name: "feature/login",
    });
  });

  it("offers pull requests only when a remote is on a signed-in service", () => {
    const a = fakeActions();
    const [main] = refs.local;
    expect(labels(localBranchMenu(a, ctx, main!))).not.toContain("Create pull request…");
    const hosted = repoContext("/work/ronin", repoInfo, refs, true);
    find(localBranchMenu(a, hosted, main!), "Create pull request…").onSelect();
    expect(useDialogs.getState().request).toEqual({
      kind: "createPullRequest",
      repo: "/work/ronin",
      branch: "main",
    });
  });

  it("offers history actions on commits and disables them mid-merge", () => {
    const a = fakeActions();
    const menu = commitMenu(a, ctx, oid);
    expect(labels(menu)).toContain("Cherry-pick onto main");
    find(menu, "Reset main here: discard changes…").onSelect();
    expect(a.reset).toHaveBeenCalledWith(oid, "hard", "main");

    const merging = repoContext("/work/ronin", { ...repoInfo, operation: "merge" }, refs);
    expect(find(commitMenu(a, merging, oid), "Revert").disabled).toBe(true);

    const detached = repoContext(
      "/work/ronin",
      { ...repoInfo, head: { kind: "detached", oid } },
      refs,
    );
    expect(labels(commitMenu(a, detached, oid))).toContain("Cherry-pick onto HEAD");
  });

  it("rebases interactively and bisects from commits", () => {
    const a = fakeActions();
    const menu = commitMenu(a, ctx, oid);
    find(menu, "Interactive rebase main onto this commit…").onSelect();
    expect(useViews.getState().views["/work/ronin"]?.rebase).toEqual({ base: oid });
    find(menu, "Start bisect: this commit is bad").onSelect();
    expect(a.bisect).toHaveBeenCalledWith("bad", oid);

    const bisecting = repoContext("/work/ronin", { ...repoInfo, operation: "bisect" }, refs);
    const during = commitMenu(a, bisecting, oid);
    expect(labels(during)).not.toContain("Start bisect: this commit is bad");
    find(during, "Bisect: skip").onSelect();
    expect(a.bisect).toHaveBeenCalledWith("skip", oid);
  });

  it("pushes and deletes tags per remote", () => {
    const a = fakeActions();
    const tag = { name: "v1", fullName: "refs/tags/v1", oid };
    const menu = tagMenu(a, ctx, tag);
    find(menu, "Push to origin").onSelect();
    expect(a.pushTag).toHaveBeenCalledWith("origin", "v1");
    find(menu, "Delete from origin…").onSelect();
    expect(a.deleteRemoteRef).toHaveBeenCalledWith("origin", "refs/tags/v1", "v1");
  });
});

describe("gitActions", () => {
  const client = new QueryClient();
  const actions = gitActions(client, "/work/ronin");

  beforeEach(() => {
    vi.clearAllMocks();
    useToasts.setState({ items: [] });
    useConfirm.setState({ request: null });
  });

  it("offers to force a rejected push over the remote commit shown", async () => {
    client.setQueryData(keys.refs("/work/ronin"), refs);
    vi.mocked(ipc.pushBranch).mockResolvedValueOnce("rejected").mockResolvedValueOnce("pushed");
    const pushed = actions.push("main", "origin/main");
    await answerConfirm(true);
    expect(await pushed).toBe(true);
    expect(ipc.pushBranch).toHaveBeenNthCalledWith(1, "/work/ronin", "main", null, null);
    // origin/main's commit in the fixture.
    expect(ipc.pushBranch).toHaveBeenNthCalledWith(2, "/work/ronin", "main", null, "a".repeat(40));
  });

  it("offers a pull request after pushing a topic branch to a hosted repository", async () => {
    const caps = { pullRequests: true } as RepoLink["capabilities"];
    const link = {
      account: "a",
      kind: "github",
      remote: "origin",
      path: "o/r",
      capabilities: caps,
    };
    client.setQueryData([...keys.hostingLinks("/work/ronin"), "a"], [link]);
    vi.mocked(ipc.pushBranch).mockResolvedValue("pushed");

    await actions.push("topic", "origin/topic");
    const [pushed] = useToasts.getState().items;
    expect(pushed).toMatchObject({ kind: "success", title: "Pushed topic" });
    pushed!.action!.run();
    expect(useDialogs.getState().request).toEqual({
      kind: "createPullRequest",
      repo: "/work/ronin",
      branch: "topic",
    });

    useToasts.setState({ items: [] });
    await actions.push("main", "origin/main");
    expect(useToasts.getState().items[0]!.action).toBeUndefined();
    client.removeQueries({ queryKey: keys.hostingLinks("/work/ronin") });
  });

  it("asks where to push a branch without an upstream", async () => {
    expect(await actions.push("topic", null)).toBe(false);
    expect(ipc.pushBranch).not.toHaveBeenCalled();
    expect(useDialogs.getState().request).toEqual({
      kind: "pushNew",
      repo: "/work/ronin",
      branch: "topic",
    });
  });

  it("asks again before deleting an unmerged branch", async () => {
    vi.mocked(ipc.deleteBranch)
      .mockRejectedValueOnce("branch topic is not fully merged")
      .mockResolvedValueOnce();
    const deleted = actions.deleteBranch("topic");
    await answerConfirm(true);
    await answerConfirm(true);
    expect(await deleted).toBe(true);
    expect(ipc.deleteBranch).toHaveBeenLastCalledWith("/work/ronin", "topic", true);
  });

  it("shows the changes when a merge stops on conflicts", async () => {
    vi.mocked(ipc.merge).mockResolvedValue("conflicts");
    expect(await actions.merge("topic", "main")).toBe(true);
    expect(useViews.getState().views["/work/ronin"]?.selected).toBe(WORKING_COPY);
    expect(useToasts.getState().items[0]).toMatchObject({
      kind: "info",
      title: "Merge topic into main stopped on conflicts",
    });
  });

  it("says what to do when a rebase stops to edit a commit", async () => {
    vi.mocked(ipc.interactiveRebase).mockResolvedValue("stopped");
    expect(await actions.interactiveRebase(oid, oid, [])).toBe(true);
    expect(useViews.getState().views["/work/ronin"]?.selected).toBe(WORKING_COPY);
    expect(useToasts.getState().items[0]).toMatchObject({
      kind: "info",
      title: "Interactive rebase stopped",
    });
  });

  it("reports failures and resolves to false", async () => {
    vi.mocked(ipc.checkoutBranch).mockRejectedValue("error: local changes would be overwritten");
    expect(await actions.checkout("topic")).toBe(false);
    expect(useToasts.getState().items[0]).toMatchObject({
      kind: "error",
      title: "Could not check out topic",
    });
  });
});

describe("tasks", () => {
  it("ends only the task that was started", () => {
    const first = startTask("/r", "Fetching");
    const second = startTask("/r", "Pushing");
    useTasks.getState().progress("/r", "Writing objects", 50);
    first();
    expect(useTasks.getState().tasks["/r"]).toMatchObject({
      label: "Pushing",
      message: "Writing objects",
      percent: 50,
    });
    second();
    expect(useTasks.getState().tasks["/r"]).toBeUndefined();
  });

  it("abbreviates object ids", () => {
    expect(shortRev(oid)).toBe("ccccccc");
    expect(shortRev("origin/main")).toBe("origin/main");
  });
});
