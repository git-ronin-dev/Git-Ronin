import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { ipc } from "../../lib/ipc";

interface ImageDiffProps {
  repo: string;
  path: string;
  /** Commit and path of the previous version, if there is one. */
  before: { oid: string; path: string } | null;
  /** Commit of the new version, or null if the file was deleted. */
  after: string | null;
}

export function ImageDiff({ repo, path, before, after }: ImageDiffProps) {
  return (
    <div className="grid h-full grid-cols-2 gap-4 p-4">
      <Pane label="Before" repo={repo} source={before} />
      <Pane label="After" repo={repo} source={after ? { oid: after, path } : null} />
    </div>
  );
}

function Pane({
  label,
  repo,
  source,
}: {
  label: string;
  repo: string;
  source: { oid: string; path: string } | null;
}) {
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

function useBlobUrl(repo: string, source: { oid: string; path: string } | null) {
  return useQuery({
    queryKey: [repo, "blob", source?.oid, source?.path],
    queryFn: async () => {
      const bytes = await ipc.blob(repo, source!.oid, source!.path);
      // SVG needs its type to render in <img>; browsers sniff the rest.
      const type = source!.path.toLowerCase().endsWith(".svg") ? "image/svg+xml" : "";
      return toDataUrl(new Blob([bytes], { type }));
    },
    enabled: source !== null,
    staleTime: Infinity,
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
