import { useQueryClient } from "@tanstack/react-query";
import { Download, LoaderCircle, RefreshCw, Upload } from "lucide-react";
import { useState } from "react";

import type { SyncStatus } from "../../bindings/SyncStatus";
import { ipc } from "../../lib/ipc";
import { relativeTime } from "../../lib/time";
import { Button } from "../../ui/Button";
import { confirm } from "../../ui/confirm-store";
import { Field, TextInput } from "../../ui/Field";
import { toast } from "../../ui/toast-store";
import { exportSettings, importSettings, syncKeys, useSyncStatus } from "./queries";
import { SettingsGroup, SettingsPage } from "./SettingsPage";

export function SyncSettings() {
  const status = useSyncStatus().data;
  return (
    <SettingsPage
      title="Sync & backup"
      description="Carry your theme, diff options, shortcuts and profiles to another machine. Repository paths, tabs and workspaces stay on this one."
    >
      <div className="flex gap-2">
        <Button onClick={() => void exportSettings()}>
          <Upload className="size-3.5" />
          Export settings…
        </Button>
        <Button onClick={() => void importSettings()}>
          <Download className="size-3.5" />
          Import settings…
        </Button>
      </div>
      <SettingsGroup title="Sync through a git repository">
        {status?.settings ? <SyncOn status={status} /> : <SyncOff />}
      </SettingsGroup>
    </SettingsPage>
  );
}

function useSyncAction() {
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const run = async (title: string, action: () => Promise<SyncStatus>) => {
    setBusy(true);
    try {
      const status = await action();
      client.setQueryData(syncKeys.status, status);
      if (status.error) toast.error(title, status.error);
      return status;
    } catch (err) {
      toast.error(title, String(err));
      return null;
    } finally {
      setBusy(false);
    }
  };
  return { busy, run };
}

function SyncOff() {
  const [remote, setRemote] = useState("");
  const [branch, setBranch] = useState("main");
  const [minutes, setMinutes] = useState("5");
  const { busy, run } = useSyncAction();
  const valid = remote.trim() !== "" && branch.trim() !== "" && /^\d{1,4}$/.test(minutes);

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (!valid) return;
        void run("Could not sync settings", () =>
          ipc.syncEnable(remote.trim(), branch.trim(), Number(minutes)),
        ).then((s) => s?.settings && !s.error && toast.success("Settings sync is on"));
      }}
    >
      <p className="text-fg-muted">
        Point Git Ronin at an empty private repository you own. Changes are committed as you make
        them, pushed every few minutes and when the app closes, and pulled when it starts. Two
        machines changing different settings merge cleanly; settings that look like passwords or
        tokens are never committed.
      </p>
      <Field label="Repository URL">
        <TextInput
          value={remote}
          placeholder="git@github.com:you/ronin-settings.git"
          onChange={(e) => setRemote(e.target.value)}
        />
      </Field>
      <div className="grid grid-cols-2 gap-3">
        <Field label="Branch">
          <TextInput value={branch} onChange={(e) => setBranch(e.target.value)} />
        </Field>
        <Field label="Push every (minutes)" hint="0: only on start, exit and on request.">
          <TextInput
            inputMode="numeric"
            value={minutes}
            aria-invalid={!/^\d{1,4}$/.test(minutes)}
            onChange={(e) => setMinutes(e.target.value)}
          />
        </Field>
      </div>
      <Button type="submit" variant="primary" disabled={busy || !valid}>
        {busy && <LoaderCircle className="size-3.5 animate-spin" />}
        Turn on sync
      </Button>
    </form>
  );
}

function SyncOn({ status }: { status: SyncStatus }) {
  const { busy, run } = useSyncAction();
  const settings = status.settings!;
  const running = busy || status.running;

  const disable = async () => {
    const ok = await confirm({
      title: "Turn off sync",
      message:
        "Stop syncing settings from this machine? Your settings stay as they are, and so does the repository.",
      confirmLabel: "Turn off",
    });
    if (ok) void run("Could not turn off sync", ipc.syncDisable);
  };

  return (
    <div className="space-y-3">
      <dl className="grid grid-cols-[8rem_1fr] gap-y-1">
        <dt className="text-fg-muted">Repository</dt>
        <dd className="truncate select-text">{settings.remote}</dd>
        <dt className="text-fg-muted">Branch</dt>
        <dd>{settings.branch}</dd>
        <dt className="text-fg-muted">Pushes</dt>
        <dd>
          {settings.pushMinutes > 0
            ? `every ${settings.pushMinutes} min, and on exit`
            : "on start, exit and on request"}
        </dd>
        <dt className="text-fg-muted">Last synced</dt>
        <dd role="status">
          {running ? "Syncing…" : status.lastSync ? relativeTime(status.lastSync) : "Not yet"}
        </dd>
      </dl>
      {status.error && !running && <p className="text-danger select-text">{status.error}</p>}
      {status.conflicts.length > 0 && <Conflicts status={status} />}
      <div className="flex gap-2">
        <Button disabled={running} onClick={() => void run("Could not sync settings", ipc.syncNow)}>
          <RefreshCw className={running ? "size-3.5 animate-spin" : "size-3.5"} />
          Sync now
        </Button>
        <Button variant="ghost" disabled={running} onClick={() => void disable()}>
          Turn off sync
        </Button>
      </div>
    </div>
  );
}

/** Settings changed differently on two machines: pick a side for each. */
function Conflicts({ status }: { status: SyncStatus }) {
  const { busy, run } = useSyncAction();
  const [theirs, setTheirs] = useState<boolean[]>(() => status.conflicts.map(() => false));

  return (
    <div className="space-y-2 rounded-md border border-warning/60 p-3">
      <p>
        These settings were changed differently here and on another machine. Choose which to keep:
      </p>
      <ul className="space-y-2">
        {status.conflicts.map((c, i) => (
          <li key={c.key}>
            <fieldset className="space-y-0.5">
              <legend className="font-mono text-xs">{c.key}</legend>
              {[false, true].map((other) => (
                <label key={String(other)} className="flex items-center gap-2">
                  <input
                    type="radio"
                    name={`conflict-${i}`}
                    checked={theirs[i] === other}
                    onChange={() => setTheirs((t) => t.map((v, j) => (j === i ? other : v)))}
                  />
                  <span className="text-fg-muted">
                    {other ? "Other machine:" : "This machine:"}
                  </span>
                  <code className="truncate">
                    {(other ? c.incoming : c.current) ?? "(not set)"}
                  </code>
                </label>
              ))}
            </fieldset>
          </li>
        ))}
      </ul>
      <Button
        variant="primary"
        disabled={busy}
        onClick={() =>
          void run("Could not merge settings", () => ipc.syncResolve(theirs)).then(
            (s) => s && !s.error && toast.success("Settings merged"),
          )
        }
      >
        Keep these and sync
      </Button>
    </div>
  );
}
