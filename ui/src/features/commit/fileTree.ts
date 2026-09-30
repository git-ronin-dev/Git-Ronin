import { useState } from "react";

/** A row of a changed-files list shown as a folder tree. */
export type TreeRow<T> =
  | {
      kind: "dir";
      /** Full path without a trailing slash, e.g. `src/app`. */
      path: string;
      /** What the row shows: chains of single folders are joined (`src/app`). */
      name: string;
      depth: number;
      /** Every item under the folder, for folder-wide actions. */
      items: T[];
      collapsed: boolean;
    }
  | { kind: "file"; item: T; name: string; depth: number };

interface Dir<T> {
  dirs: Map<string, Dir<T>>;
  files: { name: string; item: T }[];
  items: T[];
}

const newDir = <T>(): Dir<T> => ({ dirs: new Map(), files: [], items: [] });
const byName = (a: string, b: string) => a.localeCompare(b, undefined, { numeric: true });

/**
 * Flattens `items` into tree rows: folders first, then files, each sorted by
 * name. Rows inside a folder listed in `collapsed` are left out.
 */
export function treeRows<T>(
  items: T[],
  pathOf: (item: T) => string,
  collapsed: ReadonlySet<string>,
): TreeRow<T>[] {
  const root = newDir<T>();
  for (const item of items) {
    const parts = pathOf(item).split("/");
    const name = parts.pop()!;
    let dir = root;
    for (const part of parts) {
      let next = dir.dirs.get(part);
      if (!next) dir.dirs.set(part, (next = newDir()));
      next.items.push(item);
      dir = next;
    }
    dir.files.push({ name, item });
  }

  const rows: TreeRow<T>[] = [];
  const walk = (dir: Dir<T>, prefix: string, depth: number) => {
    for (const key of [...dir.dirs.keys()].sort(byName)) {
      let child = dir.dirs.get(key)!;
      let name = key;
      // Join folders that only hold one folder.
      while (child.files.length === 0 && child.dirs.size === 1) {
        const [only, next] = [...child.dirs][0]!;
        name = `${name}/${only}`;
        child = next;
      }
      const path = prefix + name;
      const isCollapsed = collapsed.has(path);
      rows.push({ kind: "dir", path, name, depth, items: child.items, collapsed: isCollapsed });
      if (!isCollapsed) walk(child, `${path}/`, depth + 1);
    }
    for (const file of [...dir.files].sort((a, b) => byName(a.name, b.name))) {
      rows.push({ kind: "file", item: file.item, name: file.name, depth });
    }
  };
  walk(root, "", 0);
  return rows;
}

/** Indentation for a tree row. */
export function indent(depth: number): number {
  return 20 + depth * 14;
}

/** Folders the user closed in one list. */
export function useCollapsed(): [ReadonlySet<string>, (path: string) => void] {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const toggle = (path: string) =>
    setCollapsed((c) => {
      const next = new Set(c);
      if (!next.delete(path)) next.add(path);
      return next;
    });
  return [collapsed, toggle];
}
