import { useEffect } from "react";

/** Whether Escape there belongs to the field (clearing, closing a picker). */
function inField(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
  );
}

/** Calls `onEscape` when Escape is pressed outside dialogs and text fields, while mounted. */
export function useEscape(onEscape: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Escape inside a dialog closes the dialog only.
      if (e.key !== "Escape" || document.querySelector("[role=dialog]") || inField(e.target)) {
        return;
      }
      onEscape();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onEscape]);
}
