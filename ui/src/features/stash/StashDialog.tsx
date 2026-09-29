import { useState } from "react";

import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { useStashActions } from "./actions";

interface StashDialogProps {
  repo: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function StashDialog({ repo, open, onOpenChange }: StashDialogProps) {
  const actions = useStashActions(repo);
  const [message, setMessage] = useState("");
  const [includeUntracked, setIncludeUntracked] = useState(true);
  const [keepIndex, setKeepIndex] = useState(false);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    const ok = await actions.push({ message: message.trim() || null, includeUntracked, keepIndex });
    setBusy(false);
    if (ok) {
      setMessage("");
      onOpenChange(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Stash changes"
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button variant="primary" disabled={busy} onClick={() => void submit()}>
            Stash
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
        <input
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="Message (optional)"
          aria-label="Stash message"
          autoFocus
          className="h-8 w-full rounded-md border border-line bg-canvas px-2 text-fg outline-none focus:border-accent"
        />
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={includeUntracked}
            onChange={(e) => setIncludeUntracked(e.target.checked)}
          />
          Include untracked files
        </label>
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={keepIndex}
            onChange={(e) => setKeepIndex(e.target.checked)}
          />
          Keep staged changes in the working copy
        </label>
      </form>
    </Dialog>
  );
}
