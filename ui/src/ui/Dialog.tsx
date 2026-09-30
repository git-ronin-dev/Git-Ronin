import * as RadixDialog from "@radix-ui/react-dialog";
import { clsx } from "clsx";
import type { ReactNode } from "react";

import { X } from "./icons";

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  /** `large`: a fixed-size window whose body lays itself out (settings). */
  size?: "default" | "large";
}

export function Dialog({
  open,
  onOpenChange,
  title,
  children,
  footer,
  size = "default",
}: DialogProps) {
  const large = size === "large";
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="fixed inset-0 z-40 animate-rn-fade bg-canvas/70" />
        <RadixDialog.Content
          // A large dialog has no single description.
          {...(large && { "aria-describedby": undefined })}
          onOpenAutoFocus={(e) => {
            // Radix would focus the first tabbable element: the close
            // button, which Enter would then press.
            const content = e.currentTarget as HTMLElement;
            if (content.contains(document.activeElement)) return;
            const target = content.querySelector<HTMLElement>(
              "input:not([type=hidden]):not([type=checkbox]), select, textarea, [data-primary]",
            );
            if (target) {
              e.preventDefault();
              target.focus();
            }
          }}
          className={clsx(
            "fixed top-1/2 left-1/2 z-50 -translate-1/2 animate-rn-rise rounded-lg border border-line bg-surface p-5 shadow-2xl outline-none",
            large ? "flex h-[min(640px,90vh)] w-[min(900px,94vw)] flex-col" : "w-[min(480px,90vw)]",
          )}
        >
          <div className="mb-3 flex items-center justify-between">
            <RadixDialog.Title className="text-base font-semibold">{title}</RadixDialog.Title>
            <RadixDialog.Close aria-label="Close" className="text-fg-muted hover:text-fg">
              <X className="size-4" />
            </RadixDialog.Close>
          </div>
          {large ? (
            <div className="min-h-0 flex-1">{children}</div>
          ) : (
            <RadixDialog.Description asChild>
              <div className="text-fg-muted">{children}</div>
            </RadixDialog.Description>
          )}
          {footer && <div className="mt-5 flex justify-end gap-2">{footer}</div>}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
