export type TreeNode<T> =
  | { kind: "folder"; name: string; path: string; children: TreeNode<T>[] }
  | { kind: "leaf"; name: string; item: T };

/**
 * Groups items by the slash-separated segments of their names, so
 * `feature/login` and `feature/signup` share a `feature` folder.
 * Folders sort before leaves; both alphabetically.
 */
export function buildTree<T>(items: T[], nameOf: (item: T) => string): TreeNode<T>[] {
  const root: TreeNode<T>[] = [];
  for (const item of items) {
    const parts = nameOf(item).split("/");
    let level = root;
    let path = "";
    for (const part of parts.slice(0, -1)) {
      path += `${part}/`;
      let folder = level.find(
        (n): n is Extract<TreeNode<T>, { kind: "folder" }> =>
          n.kind === "folder" && n.name === part,
      );
      if (!folder) {
        folder = { kind: "folder", name: part, path, children: [] };
        level.push(folder);
      }
      level = folder.children;
    }
    level.push({ kind: "leaf", name: parts.at(-1)!, item });
  }
  sortTree(root);
  return root;
}

function sortTree<T>(nodes: TreeNode<T>[]) {
  nodes.sort((a, b) =>
    a.kind !== b.kind ? (a.kind === "folder" ? -1 : 1) : a.name.localeCompare(b.name),
  );
  for (const n of nodes) if (n.kind === "folder") sortTree(n.children);
}
