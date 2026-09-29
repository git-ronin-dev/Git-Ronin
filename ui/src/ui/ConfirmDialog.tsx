import { Button } from "./Button";
import { useConfirm } from "./confirm-store";
import { Dialog } from "./Dialog";

/** Hosts the dialog behind `confirm()`. Mounted once, in App. */
export function ConfirmDialog() {
  const { request, settle } = useConfirm();
  return (
    <Dialog
      open={request !== null}
      onOpenChange={(open) => !open && settle(false)}
      title={request?.title ?? ""}
      footer={
        <>
          <Button onClick={() => settle(false)}>Cancel</Button>
          <Button
            variant={request?.danger ? "danger" : "primary"}
            onClick={() => settle(true)}
            autoFocus
          >
            {request?.confirmLabel}
          </Button>
        </>
      }
    >
      <p className="whitespace-pre-line">{request?.message}</p>
    </Dialog>
  );
}
