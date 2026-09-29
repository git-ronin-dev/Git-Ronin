import { useEffect, useState } from "react";

import { ipc, onCredentialRequest, type CredentialRequest } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { TextInput } from "../../ui/Field";

/**
 * Answers git's and ssh's credential prompts when no credential helper
 * could. Prompts queue up and are shown one at a time. Mounted once, in App.
 */
export function CredentialDialog() {
  const [queue, setQueue] = useState<CredentialRequest[]>([]);
  const [answer, setAnswer] = useState("");

  useEffect(() => {
    const unlisten = onCredentialRequest((request) => setQueue((q) => [...q, request]));
    return () => void unlisten.then((stop) => stop());
  }, []);

  const current = queue[0];
  if (!current) return null;

  const respond = (value: string | null) => {
    void ipc.credentialRespond(current.id, value);
    setAnswer("");
    setQueue((q) => q.slice(1));
  };
  // ssh asks yes/no questions (e.g. about unknown host keys) the same way.
  const question = /\(yes\/no/.test(current.prompt);

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && respond(null)}
      title={question ? "Confirm" : "Sign in"}
      footer={
        question ? (
          <>
            <Button onClick={() => respond("no")}>No</Button>
            <Button variant="primary" onClick={() => respond("yes")} autoFocus>
              Yes
            </Button>
          </>
        ) : (
          <>
            <Button onClick={() => respond(null)}>Cancel</Button>
            <Button variant="primary" onClick={() => respond(answer)}>
              OK
            </Button>
          </>
        )
      }
    >
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          respond(answer);
        }}
      >
        <p className="break-words whitespace-pre-wrap select-text">{current.prompt}</p>
        {!question && (
          <TextInput
            key={current.id}
            type={current.secret ? "password" : "text"}
            aria-label={current.prompt}
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
            autoFocus
          />
        )}
      </form>
    </Dialog>
  );
}
