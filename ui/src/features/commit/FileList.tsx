import { clsx } from "clsx";

import type { FileChange } from "../../bindings/FileChange";
import type { FileStatus } from "../../bindings/FileStatus";
import { splitPath } from "./lines";

const badges: Record<FileStatus, [string, string]> = {
  added: ["A", "text-success"],
  modified: ["M", "text-accent"],
  deleted: ["D", "text-danger"],
  renamed: ["R", "text-warning"],
  copied: ["C", "text-warning"],
  typeChanged: ["T", "text-warning"],
  unknown: ["?", "text-fg-muted"],
};

interface FileListProps {
  files: FileChange[];
  activePath: string | null;
  onOpen: (file: FileChange) => void;
}

export function FileList({ files, activePath, onOpen }: FileListProps) {
  return (
    <ul role="listbox" aria-label="Changed files">
      {files.map((file) => {
        const [dir, name] = splitPath(file.path);
        const [letter, color] = badges[file.status];
        return (
          <li key={file.path}>
            <button
              type="button"
              role="option"
              aria-selected={file.path === activePath}
              title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
              onClick={() => onOpen(file)}
              className={clsx(
                "flex h-7 w-full items-center gap-2 px-3 text-left",
                file.path === activePath ? "bg-accent/20" : "hover:bg-hover",
              )}
            >
              <span className={clsx("w-3 shrink-0 font-mono text-xs font-bold", color)}>
                {letter}
              </span>
              <span className="min-w-0 flex-1 truncate">
                <span className="text-fg-faint">{dir}</span>
                {name}
              </span>
              <Stats file={file} />
            </button>
          </li>
        );
      })}
    </ul>
  );
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
