import * as RadixToast from "@radix-ui/react-toast";
import { clsx } from "clsx";
import { X } from "lucide-react";

import { useToasts, type ToastKind } from "./toast-store";

const accents: Record<ToastKind, string> = {
  info: "border-l-accent",
  success: "border-l-success",
  error: "border-l-danger",
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
          className={clsx(
            "flex items-start gap-3 rounded-md border border-l-4 border-line bg-raised p-3 shadow-xl",
            accents[t.kind],
          )}
        >
          <div className="min-w-0 flex-1">
            <RadixToast.Title className="font-medium">{t.title}</RadixToast.Title>
            {t.description && (
              <RadixToast.Description className="mt-1 font-mono text-xs break-words whitespace-pre-wrap text-fg-muted select-text">
                {t.description}
              </RadixToast.Description>
            )}
          </div>
          <RadixToast.Close aria-label="Dismiss" className="text-fg-muted hover:text-fg">
            <X className="size-4" />
          </RadixToast.Close>
        </RadixToast.Root>
      ))}
      <RadixToast.Viewport className="fixed right-4 bottom-10 z-50 flex w-96 max-w-[90vw] flex-col gap-2 outline-none" />
    </RadixToast.Provider>
  );
}
