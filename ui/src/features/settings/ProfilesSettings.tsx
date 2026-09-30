import { clsx } from "clsx";
import { useState } from "react";

import type { Profile } from "../../bindings/Profile";
import type { SigningFormat } from "../../bindings/SigningFormat";
import { Button } from "../../ui/Button";
import { confirm } from "../../ui/confirm-store";
import { Checkbox, Field, Select, TextInput } from "../../ui/Field";
import { Plus } from "../../ui/icons";
import { activeProfileId } from "../commands/commands";
import { useConfig } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { newProfileId, useGitIdentity, useSetProfiles, useSshKeys } from "./queries";
import { SettingsPage } from "./SettingsPage";

const BLANK: Omit<Profile, "id" | "name"> = {
  userName: "",
  userEmail: "",
  signingKey: "",
  signingFormat: "openpgp",
  signCommits: false,
  sshKey: "",
};

export function ProfilesSettings() {
  const config = useConfig().data;
  const profiles = config?.portable.profiles ?? [];
  const active = activeProfileId(config);
  const [selected, setSelected] = useState<string | null>(null);
  const current = profiles.find((p) => p.id === selected) ?? profiles[0];
  const save = useSetProfiles();

  const add = () => {
    const id = newProfileId(profiles);
    save.mutate([...profiles, { ...BLANK, id, name: `Profile ${profiles.length + 1}` }]);
    setSelected(id);
  };

  return (
    <SettingsPage
      title="Profiles"
      description="Each profile has its own author, signing key, SSH key, accounts and open tabs. Empty fields leave your git configuration in charge, and a repository's own settings still win."
    >
      <div className="flex gap-4">
        <div className="w-44 shrink-0 space-y-1">
          <ul aria-label="Profiles" className="space-y-0.5">
            {profiles.map((p) => (
              <li key={p.id}>
                <button
                  type="button"
                  aria-current={p.id === current?.id ? "true" : undefined}
                  onClick={() => setSelected(p.id)}
                  className={clsx(
                    "flex h-8 w-full items-center gap-2 rounded-md px-2 text-left",
                    p.id === current?.id ? "bg-hover text-fg" : "text-fg-muted hover:bg-hover",
                  )}
                >
                  <span className="min-w-0 flex-1 truncate">{p.name || "Unnamed"}</span>
                  {p.id === active && <span className="text-xs text-success">in use</span>}
                </button>
              </li>
            ))}
          </ul>
          <Button variant="ghost" onClick={add}>
            <Plus className="size-3.5" />
            Add profile
          </Button>
        </div>
        {current && (
          <ProfileForm
            key={current.id}
            profile={current}
            active={current.id === active}
            fallback={profiles.find((p) => p.id !== current.id)?.id ?? null}
            onSave={(p) => save.mutate(profiles.map((q) => (q.id === p.id ? p : q)))}
            onDelete={() => {
              save.mutate(profiles.filter((q) => q.id !== current.id));
              setSelected(null);
            }}
          />
        )}
      </div>
    </SettingsPage>
  );
}

function ProfileForm({
  profile,
  active,
  fallback,
  onSave,
  onDelete,
}: {
  profile: Profile;
  active: boolean;
  /** The profile to use instead if this one is deleted; null if it's the only one. */
  fallback: string | null;
  onSave: (profile: Profile) => void;
  onDelete: () => void;
}) {
  const [draft, setDraft] = useState(profile);
  const identity = useGitIdentity().data;
  const keys = useSshKeys().data ?? [];
  const switchProfile = useWorkspace((s) => s.switchProfile);
  const dirty = JSON.stringify(draft) !== JSON.stringify(profile);
  const set = (patch: Partial<Profile>) => setDraft((d) => ({ ...d, ...patch }));
  const fromGit = (value: string | null | undefined) =>
    value ? `From git config: ${value}` : "Not set in git config";

  const remove = async () => {
    const ok = await confirm({
      title: "Delete profile",
      message: `Delete the profile “${profile.name}”? Its tabs are forgotten; nothing else changes.`,
      confirmLabel: "Delete",
      danger: true,
    });
    if (!ok) return;
    // Hand over to another profile (and its tabs) first.
    if (active && fallback) await switchProfile(fallback);
    onDelete();
  };

  return (
    <form
      aria-label={`Profile ${profile.name}`}
      className="min-w-0 flex-1 space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (draft.name.trim()) onSave({ ...draft, name: draft.name.trim() });
      }}
    >
      <Field label="Profile name">
        <TextInput value={draft.name} onChange={(e) => set({ name: e.target.value })} />
      </Field>
      <div className="grid grid-cols-2 gap-3">
        <Field label="Author name">
          <TextInput
            value={draft.userName}
            placeholder={fromGit(identity?.name)}
            onChange={(e) => set({ userName: e.target.value })}
          />
        </Field>
        <Field label="Author email">
          <TextInput
            type="email"
            value={draft.userEmail}
            placeholder={fromGit(identity?.email)}
            onChange={(e) => set({ userEmail: e.target.value })}
          />
        </Field>
      </div>
      <div className="grid grid-cols-[10rem_1fr] gap-3">
        <Field label="Signing format">
          <Select
            value={draft.signingFormat}
            onChange={(e) => set({ signingFormat: e.target.value as SigningFormat })}
          >
            <option value="openpgp">OpenPGP (gpg)</option>
            <option value="ssh">SSH</option>
            <option value="x509">X.509 (gpgsm)</option>
          </Select>
        </Field>
        <Field
          label="Signing key"
          hint={draft.signingFormat === "ssh" ? "A public key file." : "A key id or fingerprint."}
        >
          <TextInput
            list="profile-public-keys"
            value={draft.signingKey}
            placeholder={fromGit(identity?.signingKey)}
            onChange={(e) => set({ signingKey: e.target.value })}
          />
        </Field>
      </div>
      <Checkbox
        label="Sign every commit and tag"
        checked={draft.signCommits}
        onChange={(signCommits) => set({ signCommits })}
      />
      <Field
        label="SSH key for remotes"
        hint="Used instead of ssh's defaults for this profile's pushes, pulls and fetches."
      >
        <TextInput
          list="profile-private-keys"
          value={draft.sshKey}
          placeholder="ssh's own choice"
          onChange={(e) => set({ sshKey: e.target.value })}
        />
      </Field>
      <datalist id="profile-public-keys">
        {keys.map((k) => (
          <option key={k.path} value={`${k.path}.pub`} />
        ))}
      </datalist>
      <datalist id="profile-private-keys">
        {keys
          .filter((k) => k.hasPrivate)
          .map((k) => (
            <option key={k.path} value={k.path} />
          ))}
      </datalist>

      <div className="flex flex-wrap items-center gap-2 pt-2">
        <Button type="submit" variant="primary" disabled={!dirty || !draft.name.trim()}>
          Save
        </Button>
        <Button disabled={!dirty} onClick={() => setDraft(profile)}>
          Revert
        </Button>
        {!active && (
          <Button disabled={dirty} onClick={() => void switchProfile(profile.id)}>
            Use this profile
          </Button>
        )}
        <Button
          variant="dangerGhost"
          className="ml-auto"
          disabled={!fallback}
          onClick={() => void remove()}
        >
          Delete
        </Button>
      </div>
    </form>
  );
}
