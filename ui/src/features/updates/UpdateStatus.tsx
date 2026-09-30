import { ArrowDownCircle, LoaderCircle } from "lucide-react";

import { installUpdate, useUpdates } from "./store";

/** Status bar item: a new version to install, or its download. */
export function UpdateStatus() {
  const state = useUpdates();
  if (state.phase === "available")
    return (
      <button
        type="button"
        onClick={() => void installUpdate()}
        className="flex shrink-0 items-center gap-1 text-accent hover:underline"
      >
        <ArrowDownCircle className="size-3" />
        Update to {state.update.version}
      </button>
    );
  if (state.phase === "downloading" || state.phase === "installing")
    return (
      <span role="status" className="flex shrink-0 items-center gap-1 text-fg">
        <LoaderCircle className="size-3 animate-spin" />
        {state.phase === "installing"
          ? "Installing update…"
          : `Downloading ${state.update.version}${
              state.total ? ` ${Math.floor((state.downloaded / state.total) * 100)}%` : "…"
            }`}
      </span>
    );
  return null;
}
