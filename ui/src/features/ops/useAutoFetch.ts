import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { ipc } from "../../lib/ipc";
import { invalidateRepo, useConfig } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";

/** How often to look for repositories that are due. */
const TICK_MS = 30_000;

/**
 * Fetches every open repository's remotes every `autoFetchMinutes`, one
 * repository at a time, starting shortly after it is opened. Background
 * fetches never prompt for credentials and fail silently: a manual fetch
 * reports the problem.
 */
export function useAutoFetch() {
  const client = useQueryClient();
  const minutes = useConfig().data?.portable.git.autoFetchMinutes ?? 0;

  useEffect(() => {
    if (minutes <= 0) return;
    const interval = minutes * 60_000;
    const last = new Map<string, number>();
    let running = false;
    let stopped = false;

    const tick = async () => {
      if (running) return;
      running = true;
      try {
        for (const { path } of useWorkspace.getState().tabs) {
          if (stopped) return;
          const now = Date.now();
          if (now - (last.get(path) ?? 0) < interval) continue;
          last.set(path, now);
          try {
            await ipc.fetch(path, null, true);
            void invalidateRepo(client, path);
          } catch (err) {
            console.warn(`auto-fetch of ${path} failed:`, err);
          }
        }
      } finally {
        running = false;
      }
    };

    const first = setTimeout(() => void tick(), 5_000);
    const timer = setInterval(() => void tick(), TICK_MS);
    return () => {
      stopped = true;
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [client, minutes]);
}
