import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { invalidateRepo } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { updateView, useViews } from "../features/workspace/view";

/** App-wide shortcuts. The command palette (Phase 5) will take these over. */
export function useHotkeys() {
  const client = useQueryClient();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const ws = useWorkspace.getState();
      const active = ws.active;
      const key = e.key.toLowerCase();

      if (key === "o") {
        void ws.pickAndOpen();
      } else if (key === "w" && active) {
        void ws.close(active);
      } else if (key === "tab" && ws.tabs.length > 1) {
        const i = ws.tabs.findIndex((t) => t.path === active);
        const next = (i + (e.shiftKey ? -1 : 1) + ws.tabs.length) % ws.tabs.length;
        ws.activate(ws.tabs[next]!.path);
      } else if (key === "f" && active) {
        const current = useViews.getState().views[active]?.search;
        updateView(active, { search: current ?? { query: "", byPath: false }, openFile: null });
      } else if (key === "r" && active) {
        // Also stops the webview from reloading the whole app.
        void invalidateRepo(client, active);
      } else {
        return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [client]);
}
