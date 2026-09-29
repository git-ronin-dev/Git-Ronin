import { useQuery } from "@tanstack/react-query";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { useState, type ReactNode } from "react";

import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { Checkbox, Field, Select, TextInput } from "../../ui/Field";
import { toast } from "../../ui/toast-store";
import { useConfig, useRefs } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { useGitActions } from "./actions";
import { useDialogs, type DialogRequest } from "./dialogs";
import { startTask, useTask } from "./tasks";

type Of<K extends DialogRequest["kind"]> = Extract<DialogRequest, { kind: K }>;

/** Hosts the dialog opened with `openDialog()`. Mounted once, in App. */
export function Dialogs() {
  const { request, close } = useDialogs();
  if (!request) return null;
  // Keyed so each request starts with fresh fields.
  const key = JSON.stringify(request);
  switch (request.kind) {
    case "createBranch":
      return <CreateBranch key={key} {...request} onClose={close} />;
    case "renameBranch":
      return <RenameBranch key={key} {...request} onClose={close} />;
    case "createTag":
      return <CreateTag key={key} {...request} onClose={close} />;
    case "remote":
      return <RemoteDialog key={key} {...request} onClose={close} />;
    case "pushNew":
      return <PushNew key={key} {...request} onClose={close} />;
    case "upstream":
      return <UpstreamDialog key={key} {...request} onClose={close} />;
    case "clone":
      return <CloneDialog key={key} onClose={close} />;
  }
}

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

function FormDialog({ title, submitLabel, blocker, onSubmit, onClose, children }: FormDialogProps) {
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

function CreateBranch({
  repo,
  start,
  startLabel,
  onClose,
}: Of<"createBranch"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [name, setName] = useState("");
  const [checkout, setCheckout] = useState(true);
  return (
    <FormDialog
      title="Create branch"
      submitLabel="Create"
      blocker={name.trim() ? null : "Enter a name"}
      onSubmit={() => actions.createBranch(name.trim(), start, checkout)}
      onClose={onClose}
    >
      <Field label="Name" hint={`Starts at ${startLabel}`}>
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
      <Checkbox label="Check it out" checked={checkout} onChange={setCheckout} />
    </FormDialog>
  );
}

function RenameBranch({ repo, name, onClose }: Of<"renameBranch"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [next, setNext] = useState(name);
  return (
    <FormDialog
      title={`Rename ${name}`}
      submitLabel="Rename"
      blocker={!next.trim() || next.trim() === name ? "Enter a new name" : null}
      onSubmit={() => actions.renameBranch(name, next.trim())}
      onClose={onClose}
    >
      <Field label="New name">
        <TextInput
          value={next}
          onChange={(e) => setNext(e.target.value)}
          onFocus={(e) => e.target.select()}
          autoFocus
        />
      </Field>
    </FormDialog>
  );
}

function CreateTag({
  repo,
  target,
  targetLabel,
  onClose,
}: Of<"createTag"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [name, setName] = useState("");
  const [message, setMessage] = useState("");
  return (
    <FormDialog
      title="Create tag"
      submitLabel="Create"
      blocker={name.trim() ? null : "Enter a name"}
      onSubmit={() => actions.createTag(name.trim(), target, message.trim() || null)}
      onClose={onClose}
    >
      <Field label="Name" hint={`Tags ${targetLabel}`}>
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
      <Field
        label="Message (optional)"
        hint="With a message the tag is annotated: it records who tagged, and when."
      >
        <textarea
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          rows={3}
          className="w-full resize-none rounded-md border border-line bg-canvas px-2 py-1 text-fg outline-none focus:border-accent"
        />
      </Field>
    </FormDialog>
  );
}

function RemoteDialog({ repo, remote, onClose }: Of<"remote"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [name, setName] = useState(remote?.name ?? "");
  const [url, setUrl] = useState(remote?.url ?? "");
  const [pushUrl, setPushUrl] = useState(remote?.pushUrl ?? "");
  const [fetchNow, setFetchNow] = useState(true);
  const submit = async () => {
    if (remote) {
      return actions.editRemote(remote.name, name.trim(), url.trim(), pushUrl.trim() || null);
    }
    const added = await actions.addRemote(name.trim(), url.trim());
    if (added && fetchNow) void actions.fetch(name.trim());
    return added;
  };
  return (
    <FormDialog
      title={remote ? `Edit ${remote.name}` : "Add remote"}
      submitLabel={remote ? "Save" : "Add"}
      blocker={!name.trim() ? "Enter a name" : !url.trim() ? "Enter a URL" : null}
      onSubmit={submit}
      onClose={onClose}
    >
      <Field label="Name">
        <TextInput
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="origin"
          autoFocus={!remote}
        />
      </Field>
      <Field label="URL">
        <TextInput
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://github.com/owner/repo.git"
        />
      </Field>
      {remote ? (
        <Field label="Push URL (optional)" hint="Leave empty to push to the URL above.">
          <TextInput value={pushUrl} onChange={(e) => setPushUrl(e.target.value)} />
        </Field>
      ) : (
        <Checkbox label="Fetch it now" checked={fetchNow} onChange={setFetchNow} />
      )}
    </FormDialog>
  );
}

function PushNew({ repo, branch, onClose }: Of<"pushNew"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const remotes = useRefs(repo).data?.remotes ?? [];
  const [remote, setRemote] = useState(remotes[0]?.name ?? "");
  const [name, setName] = useState(branch);
  const chosen = remote || remotes[0]?.name || "";
  return (
    <FormDialog
      title={`Push ${branch}`}
      submitLabel="Push"
      blocker={!chosen ? "Add a remote first" : !name.trim() ? "Enter a branch name" : null}
      onSubmit={() => actions.pushTo(branch, { remote: chosen, branch: name.trim() })}
      onClose={onClose}
    >
      <p>
        {branch} has no upstream yet. Where should it go? It will track that branch from now on.
      </p>
      <Field label="Remote">
        <Select value={chosen} onChange={(e) => setRemote(e.target.value)}>
          {remotes.map((r) => (
            <option key={r.name} value={r.name}>
              {r.name}
            </option>
          ))}
        </Select>
      </Field>
      <Field label="Branch on the remote">
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
    </FormDialog>
  );
}

function UpstreamDialog({
  repo,
  branch,
  upstream,
  onClose,
}: Of<"upstream"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const remotes = useRefs(repo).data?.remotes ?? [];
  const [value, setValue] = useState(upstream ?? "");
  const options = remotes.flatMap((r) => r.branches.map((b) => `${r.name}/${b.name}`));
  return (
    <FormDialog
      title={`Upstream of ${branch}`}
      submitLabel="Save"
      blocker={value === (upstream ?? "") ? "Pick a different branch" : null}
      onSubmit={() => actions.setUpstream(branch, value || null)}
      onClose={onClose}
    >
      <Field
        label="Tracks"
        hint="Pull and push use this branch; ahead/behind counts compare with it."
      >
        <Select value={value} onChange={(e) => setValue(e.target.value)} autoFocus>
          <option value="">None</option>
          {options.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </Select>
      </Field>
    </FormDialog>
  );
}

/** The folder containing `path`, or "". */
function parentOf(path: string | null | undefined): string {
  if (!path) return "";
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : "";
}

function CloneDialog({ onClose }: { onClose: () => void }) {
  const recent = useConfig().data?.local.recentRepos;
  const active = useWorkspace((s) => s.active);
  const [url, setUrl] = useState("");
  // Next to the repository in view, else the most recently opened one.
  const [parent, setParent] = useState(() => parentOf(active ?? recent?.[0]));
  const [name, setName] = useState<string | null>(null);
  const suggested = useQuery({
    queryKey: ["cloneName", url.trim()],
    queryFn: () => ipc.cloneName(url.trim()),
    enabled: url.trim().length > 0,
  }).data;
  const folder = name ?? suggested ?? "";
  const separator = parent.includes("\\") && !parent.includes("/") ? "\\" : "/";
  const dest = parent && folder ? `${parent.replace(/[\\/]+$/, "")}${separator}${folder}` : "";
  const task = useTask(dest);

  const browse = async () => {
    const picked = await pickFolder({
      directory: true,
      title: "Clone into",
      defaultPath: parent || undefined,
    });
    if (picked) setParent(picked);
  };
  const submit = async () => {
    try {
      const info = await ipc.cloneRepo(url.trim(), parent, folder);
      useWorkspace.getState().adopt(info);
      toast.success(`Cloned ${info.name}`);
      return true;
    } catch (err) {
      toast.error("Could not clone", String(err));
      return false;
    }
  };

  return (
    <FormDialog
      title="Clone repository"
      submitLabel={task ? "Cloning…" : "Clone"}
      blocker={
        !url.trim() ? "Enter a URL" : !parent ? "Choose a folder" : !folder ? "Enter a name" : null
      }
      onSubmit={async () => {
        const done = startTask(dest, "Cloning");
        try {
          return await submit();
        } finally {
          done();
        }
      }}
      onClose={onClose}
    >
      <Field label="URL">
        <TextInput
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://github.com/owner/repo.git"
          autoFocus
        />
      </Field>
      <Field label="Into folder">
        <div className="flex gap-2">
          <TextInput value={parent} onChange={(e) => setParent(e.target.value)} />
          <Button type="button" onClick={() => void browse()}>
            Browse…
          </Button>
        </div>
      </Field>
      <Field label="Name" hint={dest && `Creates ${dest}`}>
        <TextInput value={folder} onChange={(e) => setName(e.target.value)} />
      </Field>
      {task && (
        <div className="space-y-1" aria-live="polite">
          <div className="h-1 overflow-hidden rounded-full bg-raised">
            <div
              className="h-full bg-accent transition-[width]"
              style={{ width: `${task.percent ?? 0}%` }}
            />
          </div>
          <p className="truncate text-xs text-fg-faint">{task.message ?? "Starting…"}</p>
        </div>
      )}
    </FormDialog>
  );
}
