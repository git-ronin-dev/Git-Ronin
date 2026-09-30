import { clsx } from "clsx";

import type { FileChange } from "../../bindings/FileChange";
import type { FileStatus } from "../../bindings/FileStatus";
import { ContextMenu, type MenuItem } from "../../ui/ContextMenu";
import { useUiPrefs } from "../workspace/queries";
import { indent, treeRows, useCollapsed, type TreeRow } from "./fileTree";
import { FolderRow } from "./FileTreeParts";
import { splitPath } from "./lines";

const badges: Record<FileStatus, [string, string]> = {
  added: ["A", "text-success"],
  modified: ["M", "text-accent"],
  deleted: ["D", "text-danger"],
  renamed: ["R", "text-warning"],
  copied: ["C", "text-warning"],
  typeChanged: ["T", "text-warning"],
  untracked: ["U", "text-success"],
  conflicted: ["!", "text-danger"],
  unknown: ["?", "text-fg-muted"],
};

interface FileListProps {
  files: FileChange[];
  activePath: string | null;
  onOpen: (file: FileChange) => void;
  /** Right-click menu for a file. */
  menu?: (file: FileChange) => MenuItem[];
}

export function FileList({ files, activePath, onOpen, menu }: FileListProps) {
  const tree = useUiPrefs()?.fileTree ?? false;
  const [collapsed, toggle] = useCollapsed();
  const rows: TreeRow<FileChange>[] = tree
    ? treeRows(files, (f) => f.path, collapsed)
    : files.map((item) => ({ kind: "file", item, name: item.path, depth: 0 }));

  return (
    <ul role={tree ? "tree" : "listbox"} aria-label="Changed files">
      {rows.map((row) => {
        if (row.kind === "dir")
          return (
            <li key={`dir:${row.path}`}>
              <FolderRow row={row} onToggle={() => toggle(row.path)} />
            </li>
          );
        const file = row.item;
        const [dir, name] = tree ? ["", row.name] : splitPath(file.path);
        const button = (
          <button
            type="button"
            role={tree ? "treeitem" : "option"}
            aria-selected={file.path === activePath}
            title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
            onClick={() => onOpen(file)}
            style={tree ? { paddingLeft: indent(row.depth) + 6 } : undefined}
            className={clsx(
              "flex h-7 w-full items-center gap-2 px-3 text-left",
              file.path === activePath ? "bg-accent/20" : "hover:bg-hover",
            )}
          >
            <Badge status={file.status} />
            <span className="min-w-0 flex-1 truncate">
              <span className="text-fg-faint">{dir}</span>
              {name}
            </span>
            <Stats file={file} />
          </button>
        );
        return (
          <li key={file.path}>
            {menu ? <ContextMenu items={menu(file)}>{button}</ContextMenu> : button}
          </li>
        );
      })}
    </ul>
  );
}

export function Badge({ status }: { status: FileStatus }) {
  const [letter, color] = badges[status];
  return <span className={clsx("w-3 shrink-0 font-mono text-xs font-bold", color)}>{letter}</span>;
}

export function Stats({ file }: { file: Pick<FileChange, "additions" | "deletions"> }) {
  if (file.additions === null) return <span className="text-xs text-fg-faint">binary</span>;
  return (
    <span className="shrink-0 font-mono text-xs">
      {file.additions > 0 && <span className="text-success">+{file.additions}</span>}
      {file.additions > 0 && !!file.deletions && " "}
      {!!file.deletions && <span className="text-danger">−{file.deletions}</span>}
    </span>
  );
}
