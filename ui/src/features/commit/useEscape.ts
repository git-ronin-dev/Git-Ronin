import { useEffect } from "react";

/** Calls `onEscape` when Escape is pressed anywhere, while mounted. */
export function useEscape(onEscape: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Escape inside a dialog closes the dialog only.
      if (e.key === "Escape" && !document.querySelector("[role=dialog]")) onEscape();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onEscape]);
}
