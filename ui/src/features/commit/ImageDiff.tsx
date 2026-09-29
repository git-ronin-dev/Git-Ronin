import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import type { BlobSource } from "../../bindings/BlobSource";
import { ipc } from "../../lib/ipc";
import { keys } from "../workspace/queries";

/** One version of a file: as of a commit, or uncommitted. */
export type BlobRef =
  | { kind: "commit"; oid: string; path: string }
  | { kind: "working"; source: BlobSource; path: string };

interface ImageDiffProps {
  repo: string;
  /** The previous version, if there is one. */
  before: BlobRef | null;
  /** The new version, or null if the file was deleted. */
  after: BlobRef | null;
}

export function ImageDiff({ repo, before, after }: ImageDiffProps) {
  return (
    <div className="grid h-full grid-cols-2 gap-4 p-4">
      <Pane label="Before" repo={repo} source={before} />
      <Pane label="After" repo={repo} source={after} />
    </div>
  );
}

function Pane({ label, repo, source }: { label: string; repo: string; source: BlobRef | null }) {
  const url = useBlobUrl(repo, source);
  const [size, setSize] = useState<string>("");
  return (
    <figure className="flex min-h-0 flex-col gap-2">
      <figcaption className="text-xs text-fg-muted">
        {label} {size && <span className="text-fg-faint">· {size}</span>}
      </figcaption>
      <div className="flex min-h-0 flex-1 items-center justify-center rounded-md border border-line bg-[repeating-conic-gradient(var(--rn-raised)_0_25%,transparent_0_50%)] bg-[length:16px_16px] p-2">
        {source === null ? (
          <span className="text-fg-faint">None</span>
        ) : url ? (
          <img
            src={url}
            alt={label}
            onLoad={(e) =>
              setSize(`${e.currentTarget.naturalWidth}×${e.currentTarget.naturalHeight}`)
            }
            className="max-h-full max-w-full object-contain"
          />
        ) : (
          <span className="text-fg-faint">Loading…</span>
        )}
      </div>
    </figure>
  );
}

function useBlobUrl(repo: string, source: BlobRef | null) {
  const working = source?.kind === "working";
  return useQuery({
    // Uncommitted versions change, and refresh with the working copy.
    queryKey: working
      ? [...keys.working(repo), "blob", source.source, source.path]
      : [repo, "blob", source?.kind === "commit" && source.oid, source?.path],
    queryFn: async () => {
      const bytes =
        source!.kind === "commit"
          ? await ipc.blob(repo, source!.oid, source!.path)
          : await ipc.workingBlob(repo, source!.path, source!.source);
      // SVG needs its type to render in <img>; browsers sniff the rest.
      const type = source!.path.toLowerCase().endsWith(".svg") ? "image/svg+xml" : "";
      return toDataUrl(new Blob([bytes], { type }));
    },
    enabled: source !== null,
    staleTime: working ? 0 : Infinity,
  }).data;
}

function toDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}
