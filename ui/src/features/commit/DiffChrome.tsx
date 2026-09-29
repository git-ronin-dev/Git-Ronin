import { clsx } from "clsx";
import { ArrowLeft } from "lucide-react";
import type { ReactNode } from "react";

import { useSetUiPrefs, useUiPrefs } from "../workspace/queries";
import { splitPath } from "./lines";

/** Top bar of a diff that replaces the graph: back button, file name, controls. */
export function DiffHeader({
  path,
  oldPath,
  onClose,
  children,
}: {
  path: string;
  oldPath: string | null;
  onClose: () => void;
  children?: ReactNode;
}) {
  const [dir, name] = splitPath(path);
  return (
    <header className="flex h-10 shrink-0 items-center gap-2 border-b border-line bg-surface px-2">
      <button
        type="button"
        onClick={onClose}
        aria-label="Back to graph"
        title="Back to graph (Esc)"
        className="flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-hover hover:text-fg"
      >
        <ArrowLeft className="size-4" />
      </button>
      <span className="min-w-0 flex-1 truncate select-text" title={path}>
        {oldPath && <span className="text-fg-faint">{oldPath} → </span>}
        <span className="text-fg-faint">{dir}</span>
        <span className="font-medium">{name}</span>
      </span>
      {children}
    </header>
  );
}

/** Ignore-whitespace toggle and unified/split switch, saved as preferences. */
export function DiffViewControls() {
  const prefs = useUiPrefs();
  const setPrefs = useSetUiPrefs();
  if (!prefs) return null;
  const mode = prefs.diffView;
  return (
    <>
      <label className="ml-2 flex items-center gap-1.5 text-xs text-fg-muted">
        <input
          type="checkbox"
          checked={prefs.ignoreWhitespace}
          onChange={(e) => setPrefs.mutate({ ...prefs, ignoreWhitespace: e.target.checked })}
        />
        Ignore whitespace
      </label>
      <div className="flex rounded-md border border-line text-xs">
        {(["unified", "split"] as const).map((m) => (
          <button
            key={m}
            type="button"
            onClick={() => setPrefs.mutate({ ...prefs, diffView: m })}
            className={clsx(
              "h-6 px-2 capitalize first:rounded-l-md last:rounded-r-md",
              mode === m ? "bg-raised text-fg" : "text-fg-muted hover:text-fg",
            )}
          >
            {m}
          </button>
        ))}
      </div>
    </>
  );
}
