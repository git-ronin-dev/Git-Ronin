import { describe, expect, it } from "vitest";

import { buildTree, type TreeNode } from "./tree";

const shape = (nodes: TreeNode<string>[]): unknown =>
  nodes.map((n) => (n.kind === "leaf" ? n.name : { [n.name]: shape(n.children) }));

describe("buildTree", () => {
  it("groups by path segments, folders first", () => {
    const tree = buildTree(["main", "feature/b", "feature/a", "fix/deep/x", "dev"], (s) => s);
    expect(shape(tree)).toEqual([
      { feature: ["a", "b"] },
      { fix: [{ deep: ["x"] }] },
      "dev",
      "main",
    ]);
  });

  it("records folder paths", () => {
    const [folder] = buildTree(["a/b/c"], (s) => s);
    expect(folder).toMatchObject({ kind: "folder", path: "a/" });
  });
});
