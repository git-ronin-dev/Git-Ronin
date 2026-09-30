import { useState, type ReactNode } from "react";

import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";

interface FormDialogProps {
  title: string;
  submitLabel: string;
  /** Why the form can't be submitted yet, if it can't. */
  blocker?: string | null;
  /**
   * Resolves to true when done, closing the dialog. Until then the dialog
   * stays open and can't be dismissed.
   */
  onSubmit: () => Promise<boolean>;
  onClose: () => void;
  children: ReactNode;
}

export function FormDialog({
  title,
  submitLabel,
  blocker,
  onSubmit,
  onClose,
  children,
}: FormDialogProps) {
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    if (busy || blocker) return;
    setBusy(true);
    const done = await onSubmit();
    setBusy(false);
    if (done) onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !busy && onClose()}
      title={title}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={busy || !!blocker}
            title={blocker ?? undefined}
            onClick={() => void submit()}
          >
            {submitLabel}
          </Button>
        </>
      }
    >
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        {children}
        {/* Lets Enter submit from any field. */}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
