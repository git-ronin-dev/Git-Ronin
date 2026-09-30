import * as RadixToast from "@radix-ui/react-toast";
import { clsx } from "clsx";
import type { CSSProperties } from "react";

import { X } from "./icons";
import { useToasts, type ToastKind } from "./toast-store";

const accents: Record<ToastKind, string> = {
  info: "border-l-gold",
  success: "border-l-success",
  error: "border-l-danger",
};

/** The ink that splashes behind a new toast, from its coloured edge. */
const splash: Record<ToastKind, string> = {
  info: "var(--rn-gold)",
  success: "var(--rn-success)",
  error: "var(--rn-danger)",
};

export function Toaster() {
  const { items, dismiss } = useToasts();
  return (
    <RadixToast.Provider duration={5000}>
      {items.map((t) => (
        <RadixToast.Root
          key={t.id}
          // Errors stay until dismissed: they often carry git output worth reading.
          duration={t.kind === "error" || t.persist ? Infinity : undefined}
          onOpenChange={(open) => !open && dismiss(t.id)}
          style={{ "--rn-splash": splash[t.kind] } as CSSProperties}
          className={clsx(
            "rn-toast relative flex items-start gap-3 overflow-hidden rounded-md border border-l-4 border-line bg-raised p-3 shadow-xl",
            accents[t.kind],
          )}
        >
          <div className="relative min-w-0 flex-1">
            <RadixToast.Title className="font-medium">{t.title}</RadixToast.Title>
            {t.description && (
              <RadixToast.Description className="mt-1 font-mono text-xs break-words whitespace-pre-wrap text-fg-muted select-text">
                {t.description}
              </RadixToast.Description>
            )}
            {t.action && (
              <RadixToast.Action
                altText={t.action.label}
                onClick={t.action.run}
                className="mt-2 inline-flex h-6 items-center rounded-md border border-line bg-surface px-2 text-xs font-medium hover:bg-hover"
              >
                {t.action.label}
              </RadixToast.Action>
            )}
          </div>
          <RadixToast.Close aria-label="Dismiss" className="relative text-fg-muted hover:text-fg">
            <X className="size-4" />
          </RadixToast.Close>
        </RadixToast.Root>
      ))}
      <RadixToast.Viewport className="fixed right-4 bottom-10 z-50 flex w-96 max-w-[90vw] flex-col gap-2 outline-none" />
    </RadixToast.Provider>
  );
}
