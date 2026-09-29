import * as RadixDialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
}

export function Dialog({ open, onOpenChange, title, children, footer }: DialogProps) {
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="fixed inset-0 z-40 bg-canvas/70" />
        <RadixDialog.Content className="fixed top-1/2 left-1/2 z-50 w-[min(480px,90vw)] -translate-1/2 rounded-lg border border-line bg-surface p-5 shadow-2xl">
          <div className="mb-3 flex items-center justify-between">
            <RadixDialog.Title className="text-base font-semibold">{title}</RadixDialog.Title>
            <RadixDialog.Close aria-label="Close" className="text-fg-muted hover:text-fg">
              <X className="size-4" />
            </RadixDialog.Close>
          </div>
          <RadixDialog.Description asChild>
            <div className="text-fg-muted">{children}</div>
          </RadixDialog.Description>
          {footer && <div className="mt-5 flex justify-end gap-2">{footer}</div>}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
