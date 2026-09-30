import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";

import { ipc } from "../../lib/ipc";
import { useConfig } from "../workspace/queries";
import { checkForUpdates } from "./store";

const FIRST_CHECK_MS = 10_000;
const INTERVAL_MS = 12 * 60 * 60_000;

export function useAppInfo() {
  return useQuery({ queryKey: ["appInfo"], queryFn: ipc.appInfo, staleTime: Infinity });
}

/**
 * Looks for a new version shortly after startup and twice a day, when the
 * user allows it and this build can update itself.
 */
export function useUpdateCheck() {
  const enabled = useConfig().data?.local.checkUpdates ?? false;
  const supported = useAppInfo().data?.updatesUnavailable === null;

  useEffect(() => {
    if (!enabled || !supported) return;
    const first = setTimeout(() => void checkForUpdates(false), FIRST_CHECK_MS);
    const every = setInterval(() => void checkForUpdates(false), INTERVAL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  }, [enabled, supported]);
}
