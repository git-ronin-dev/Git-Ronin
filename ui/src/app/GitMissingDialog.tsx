import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { ipc } from "../lib/ipc";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";

/** Git Ronin shells out to git for writes and network, so a missing git is fatal. */
export function GitMissingDialog() {
  const git = useQuery({ queryKey: ["gitVersion"], queryFn: ipc.gitVersion });
  const [dismissed, setDismissed] = useState(false);

  return (
    <Dialog
      open={git.isError && !dismissed}
      onOpenChange={(open) => !open && setDismissed(true)}
      title="Git not available"
      footer={
        <Button variant="primary" onClick={() => setDismissed(true)}>
          OK
        </Button>
      }
    >
      <p className="select-text">{String(git.error)}</p>
      <p className="mt-2">
        Install git from your package manager or{" "}
        <span className="select-text">https://git-scm.com</span>, then restart Git Ronin.
      </p>
    </Dialog>
  );
}
