import { useQueryClient } from "@tanstack/react-query";
import { Copy, KeyRound } from "lucide-react";
import { useState } from "react";

import type { SshKey } from "../../bindings/SshKey";
import { copyText } from "../../lib/clipboard";
import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { Field, TextInput } from "../../ui/Field";
import { toast } from "../../ui/toast-store";
import { activeProfileId } from "../commands/commands";
import { useConfig } from "../workspace/queries";
import { settingsKeys, useGitIdentity, useSetProfiles, useSshKeys } from "./queries";
import { SettingsGroup, SettingsPage } from "./SettingsPage";

export function SshSettings() {
  const keys = useSshKeys();
  return (
    <SettingsPage
      title="SSH keys"
      description="Keys in your ~/.ssh folder. Add a public key to your account on GitHub, GitLab or another host to push and pull over SSH."
    >
      {keys.isError && <p className="text-danger">{String(keys.error)}</p>}
      {keys.data?.length === 0 && <p className="text-fg-faint">No SSH keys yet.</p>}
      <ul className="space-y-2">
        {keys.data?.map((key) => (
          <KeyRow key={key.path} sshKey={key} />
        ))}
      </ul>
      <SettingsGroup title="Generate a key">
        <GenerateForm existing={keys.data ?? []} />
      </SettingsGroup>
    </SettingsPage>
  );
}

function KeyRow({ sshKey }: { sshKey: SshKey }) {
  const config = useConfig().data;
  const profiles = config?.portable.profiles ?? [];
  const active = profiles.find((p) => p.id === activeProfileId(config));
  const setProfiles = useSetProfiles();
  const inUse = active?.sshKey === sshKey.path;

  return (
    <li className="flex items-start gap-3 rounded-md border border-line p-3">
      <KeyRound className="mt-0.5 size-4 shrink-0 text-fg-muted" />
      <div className="min-w-0 flex-1 space-y-0.5">
        <div className="flex items-baseline gap-2">
          <span className="font-medium">{sshKey.name}</span>
          <span className="text-xs text-fg-faint">{sshKey.algorithm}</span>
          {!sshKey.hasPrivate && <span className="text-xs text-warning">public key only</span>}
          {inUse && <span className="text-xs text-success">used by {active.name}</span>}
        </div>
        {sshKey.comment && <div className="truncate text-fg-muted">{sshKey.comment}</div>}
        <div className="truncate font-mono text-xs text-fg-faint select-text">
          {sshKey.fingerprint}
        </div>
      </div>
      <div className="flex shrink-0 flex-col items-end gap-1">
        <Button onClick={() => void copyText(sshKey.publicKey, "Public key copied")}>
          <Copy className="size-3.5" />
          Copy public key
        </Button>
        {active && sshKey.hasPrivate && !inUse && (
          <Button
            variant="ghost"
            onClick={() =>
              setProfiles.mutate(
                profiles.map((p) => (p.id === active.id ? { ...p, sshKey: sshKey.path } : p)),
              )
            }
          >
            Use for {active.name}
          </Button>
        )}
      </div>
    </li>
  );
}

function freeName(existing: SshKey[]): string {
  const taken = new Set(existing.map((k) => k.name));
  if (!taken.has("id_ed25519")) return "id_ed25519";
  for (let i = 2; ; i++) if (!taken.has(`id_ed25519_${i}`)) return `id_ed25519_${i}`;
}

function GenerateForm({ existing }: { existing: SshKey[] }) {
  const client = useQueryClient();
  const config = useConfig().data;
  const profile = config?.portable.profiles.find((p) => p.id === activeProfileId(config));
  const identity = useGitIdentity().data;
  const [name, setName] = useState<string | null>(null);
  const [comment, setComment] = useState<string | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);

  const shownName = name ?? freeName(existing);
  const shownComment = comment ?? (profile?.userEmail || identity?.email || "");
  const mismatch = passphrase !== again;

  const generate = async () => {
    setBusy(true);
    try {
      const key = await ipc.sshGenerate(shownName.trim(), shownComment, passphrase);
      toast.success(`Generated ${key.name}`, key.fingerprint);
      await copyText(key.publicKey, "Public key copied");
      setName(null);
      setPassphrase("");
      setAgain("");
      void client.invalidateQueries({ queryKey: settingsKeys.sshKeys });
    } catch (err) {
      toast.error("Could not generate a key", String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (!mismatch && shownName.trim()) void generate();
      }}
    >
      <p className="text-fg-muted">
        An Ed25519 key, the kind every major host accepts. Its public key is copied when it’s ready.
      </p>
      <div className="grid grid-cols-2 gap-3">
        <Field label="File name" hint="Saved in ~/.ssh.">
          <TextInput value={shownName} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Comment" hint="Usually your email address.">
          <TextInput value={shownComment} onChange={(e) => setComment(e.target.value)} />
        </Field>
        <Field label="Passphrase" hint="Recommended. Asked for when the key is used.">
          <TextInput
            type="password"
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
          />
        </Field>
        <Field label="Passphrase again" hint={mismatch ? "The passphrases differ." : undefined}>
          <TextInput
            type="password"
            aria-invalid={mismatch}
            value={again}
            onChange={(e) => setAgain(e.target.value)}
          />
        </Field>
      </div>
      <Button type="submit" variant="primary" disabled={busy || mismatch || !shownName.trim()}>
        Generate key
      </Button>
    </form>
  );
}
