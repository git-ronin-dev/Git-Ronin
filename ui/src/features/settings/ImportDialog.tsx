import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { useOverlays } from "../../app/overlays";
import type { ImportMode } from "../../bindings/ImportMode";
import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { toast } from "../../ui/toast-store";
import { keys } from "../workspace/queries";

/** Shows what importing a settings file changes, then imports it. */
export function ImportDialog() {
  const path = useOverlays((s) => s.importPath);
  const close = () => useOverlays.setState({ importPath: null });
  return (
    <Dialog open={path !== null} onOpenChange={(open) => !open && close()} title="Import settings">
      {path && <ImportPreview key={path} path={path} onDone={close} />}
    </Dialog>
  );
}

function ImportPreview({ path, onDone }: { path: string; onDone: () => void }) {
  const client = useQueryClient();
  const [mode, setMode] = useState<ImportMode>("merge");
  const [busy, setBusy] = useState(false);
  const preview = useQuery({
    queryKey: ["importPreview", path, mode],
    queryFn: () => ipc.settingsImportPreview(path, mode),
    gcTime: 0,
  });
  const changes = preview.data ?? [];

  const apply = async () => {
    setBusy(true);
    try {
      const warning = await ipc.settingsImport(path, mode);
      if (warning) toast.error("Profile not fully applied", warning);
      toast.success("Settings imported");
      void client.invalidateQueries({ queryKey: keys.config });
      onDone();
    } catch (err) {
      toast.error("Could not import settings", String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <p className="truncate text-xs text-fg-faint" title={path}>
        {path}
      </p>
      <fieldset className="space-y-1">
        <legend className="sr-only">How to import</legend>
        <label className="flex items-start gap-2">
          <input
            type="radio"
            name="import-mode"
            className="mt-1"
            checked={mode === "merge"}
            onChange={() => setMode("merge")}
          />
          <span>
            <span className="text-fg">Merge</span> — take the file’s settings, keep the ones it
            doesn’t have
          </span>
        </label>
        <label className="flex items-start gap-2">
          <input
            type="radio"
            name="import-mode"
            className="mt-1"
            checked={mode === "replace"}
            onChange={() => setMode("replace")}
          />
          <span>
            <span className="text-fg">Replace</span> — use exactly the file’s settings
          </span>
        </label>
      </fieldset>
      {preview.isError ? (
        <p className="text-danger">{String(preview.error)}</p>
      ) : (
        <div className="max-h-64 overflow-y-auto rounded-md border border-line">
          {preview.isPending ? (
            <p className="p-2 text-fg-faint">Reading…</p>
          ) : changes.length === 0 ? (
            <p className="p-2 text-fg-faint">Nothing would change.</p>
          ) : (
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-line text-left text-fg-faint">
                  <th className="p-1.5 font-medium">Setting</th>
                  <th className="p-1.5 font-medium">Now</th>
                  <th className="p-1.5 font-medium">After import</th>
                </tr>
              </thead>
              <tbody className="font-mono">
                {changes.map((c) => (
                  <tr key={c.key} className="border-b border-line/50 align-top">
                    <td className="p-1.5 text-fg">{c.key}</td>
                    <td className="p-1.5 break-all text-danger">{c.current ?? "—"}</td>
                    <td className="p-1.5 break-all text-success">{c.incoming ?? "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
      <div className="flex justify-end gap-2 pt-2">
        <Button onClick={onDone}>Cancel</Button>
        <Button
          variant="primary"
          disabled={busy || preview.isError || changes.length === 0}
          onClick={() => void apply()}
        >
          Import
        </Button>
      </div>
    </div>
  );
}
