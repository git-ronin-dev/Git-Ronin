import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import type { AccountView } from "../../bindings/AccountView";
import type { DeviceStart } from "../../bindings/DeviceStart";
import type { ProviderKind } from "../../bindings/ProviderKind";
import type { SignIn } from "../../bindings/SignIn";
import { copyText } from "../../lib/clipboard";
import { ipc } from "../../lib/ipc";
import { openUrl } from "../../lib/open";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { confirm } from "../../ui/confirm-store";
import { Field, Select, TextInput } from "../../ui/Field";
import { Copy, ExternalLink } from "../../ui/icons";
import { toast } from "../../ui/toast-store";
import { activeProfileId } from "../commands/commands";
import { SettingsGroup, SettingsPage } from "../settings/SettingsPage";
import { useConfig, useUiPrefs } from "../workspace/queries";
import { PROVIDER_KINDS, PROVIDERS, shortUrl } from "./providers";
import { accountProfile, invalidateHosting, useAccounts } from "./queries";

/** Services that can sign in through the browser. */
const DEVICE_FLOW: ProviderKind[] = ["github", "gitlab"];

export function AccountsSettings() {
  const config = useConfig().data;
  const accounts = useAccounts();
  const active = activeProfileId(config);
  const mine = (accounts.data ?? []).filter(
    (a) => accountProfile(config, a.account.profile) === active,
  );
  const others = (accounts.data?.length ?? 0) - mine.length;
  const profileName = config?.portable.profiles.find((p) => p.id === active)?.name;
  const [again, setAgain] = useState<AccountView | null>(null);
  // A fresh form after each sign-in.
  const [round, setRound] = useState(0);

  return (
    <SettingsPage
      title="Accounts"
      description="Sign in to see pull requests, issues and checks, clone your repositories and push over HTTPS without typing a password. Accounts belong to the profile in use; tokens stay in your system's keyring."
    >
      {accounts.isError && <p className="text-danger">{String(accounts.error)}</p>}
      {mine.length === 0 && (
        <p className="text-fg-faint">No accounts for {profileName ?? "this profile"} yet.</p>
      )}
      <ul className="space-y-2">
        {mine.map((a) => (
          <AccountRow key={a.account.id} view={a} onSignInAgain={() => setAgain(a)} />
        ))}
      </ul>
      {others > 0 && (
        <p className="text-xs text-fg-faint">
          {others} {others === 1 ? "account belongs" : "accounts belong"} to other profiles.
        </p>
      )}
      <SettingsGroup
        title={again ? `Sign in to ${again.account.username} again` : "Add an account"}
      >
        <AddAccount
          key={again?.account.id ?? `new-${round}`}
          initial={again}
          onDone={() => {
            setAgain(null);
            setRound((r) => r + 1);
          }}
        />
      </SettingsGroup>
    </SettingsPage>
  );
}

function AccountRow({ view, onSignInAgain }: { view: AccountView; onSignInAgain: () => void }) {
  const { account } = view;
  const client = useQueryClient();
  const config = useConfig().data;
  const profiles = config?.portable.profiles ?? [];
  const showAvatars = useUiPrefs()?.showAvatars ?? false;
  const provider = PROVIDERS[account.kind];

  const remove = async () => {
    const ok = await confirm({
      title: "Remove account",
      message: `Remove ${account.username} on ${provider.label}? Its token is deleted from this computer; it stays valid on the service until you revoke it there.`,
      confirmLabel: "Remove",
      danger: true,
    });
    if (!ok) return;
    try {
      await ipc.hostingSignOut(account.id);
      invalidateHosting(client);
    } catch (err) {
      toast.error("Could not remove the account", String(err));
    }
  };
  const move = async (profile: string) => {
    try {
      await ipc.hostingMoveAccount(account.id, profile);
      invalidateHosting(client);
    } catch (err) {
      toast.error("Could not move the account", String(err));
    }
  };

  return (
    <li className="flex items-center gap-3 rounded-md border border-line p-3">
      <Avatar
        name={account.name || account.username}
        email={account.username}
        size={28}
        remote={showAvatars}
        url={account.avatarUrl || null}
      />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="truncate font-medium">{account.name || account.username}</span>
          <span className="truncate text-xs text-fg-faint">{account.username}</span>
        </div>
        <div className="text-xs text-fg-muted">
          {provider.label} · {shortUrl(account.url)}
          {account.oauthClientId ? " · signed in through the browser" : ""}
        </div>
        {!view.signedIn && (
          <div className="text-xs text-warning">Not signed in on this computer.</div>
        )}
      </div>
      {!view.signedIn && (
        <Button variant="primary" onClick={onSignInAgain}>
          Sign in
        </Button>
      )}
      {profiles.length > 1 && (
        <div className="w-36 shrink-0">
          <Select
            aria-label={`Profile of ${account.username}`}
            value={accountProfile(config, account.profile)}
            onChange={(e) => void move(e.target.value)}
          >
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </Select>
        </div>
      )}
      <Button variant="dangerGhost" onClick={() => void remove()}>
        Remove
      </Button>
    </li>
  );
}

function AddAccount({ initial, onDone }: { initial: AccountView | null; onDone: () => void }) {
  const client = useQueryClient();
  const [kind, setKind] = useState<ProviderKind>(initial?.account.kind ?? "github");
  const provider = PROVIDERS[kind];
  const publicUrl = provider.publicUrl;
  const [server, setServer] = useState(
    initial && initial.account.url !== publicUrl ? initial.account.url : "",
  );
  const url = (server.trim() || publicUrl || "").replace(/\/+$/, "");
  const [login, setLogin] = useState(initial?.account.login ?? "");
  const [token, setToken] = useState("");
  const [clientId, setClientId] = useState<string | null>(initial?.account.oauthClientId || null);
  const [builtIn, setBuiltIn] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [device, setDevice] = useState<DeviceStart | null>(null);

  const browserSignIn = DEVICE_FLOW.includes(kind);
  useEffect(() => {
    if (!browserSignIn || !url) return;
    let live = true;
    void ipc.hostingClientId(kind, url).then((id) => live && setBuiltIn(id));
    return () => {
      live = false;
    };
  }, [kind, url, browserSignIn]);
  const shownClientId = clientId ?? builtIn ?? "";
  // Signing in through the browser comes first when it's ready to use.
  const browserFirst = browserSignIn && (!!builtIn || !!initial?.account.oauthClientId);

  const signedIn = (result: SignIn) => {
    const { account, warning } = result;
    toast.success(`Signed in as ${account.username}`, `${PROVIDERS[account.kind].label}`);
    if (warning) toast.error("Token not saved", warning);
    setToken("");
    invalidateHosting(client);
    onDone();
  };

  const signInWithToken = async () => {
    setBusy(true);
    try {
      signedIn(await ipc.hostingSignIn(kind, url, login, token));
    } catch (err) {
      toast.error("Could not sign in", String(err));
    } finally {
      setBusy(false);
    }
  };

  const signInWithBrowser = async () => {
    setBusy(true);
    try {
      const started = await ipc.hostingDeviceStart(kind, url, shownClientId);
      setDevice(started);
      void openUrl(started.code.verificationUriComplete ?? started.code.verificationUri);
      signedIn(await ipc.hostingDeviceWait(started.flow));
    } catch (err) {
      if (!String(err).includes("cancelled")) toast.error("Could not sign in", String(err));
    } finally {
      setDevice(null);
      setBusy(false);
    }
  };

  if (device) {
    return (
      <DeviceCodePanel start={device} onCancel={() => void ipc.hostingDeviceCancel(device.flow)} />
    );
  }

  const browserPanel = (
    <div className="space-y-2 rounded-md border border-line p-3">
      <p className="text-fg-muted">
        Sign in on the {provider.label} website and approve Git Ronin there.
      </p>
      <details open={!builtIn}>
        <summary className="cursor-default text-xs text-fg-muted">OAuth application</summary>
        <div className="mt-2">
          <Field
            label="Client ID"
            hint={
              builtIn
                ? "Git Ronin's own application is filled in."
                : `Register an OAuth application on ${shortUrl(url || provider.serverHint)} with the device flow enabled, and enter its client ID. No secret is needed.`
            }
          >
            <TextInput value={shownClientId} onChange={(e) => setClientId(e.target.value)} />
          </Field>
        </div>
      </details>
      <Button
        variant="primary"
        disabled={busy || !url || !shownClientId.trim()}
        onClick={() => void signInWithBrowser()}
      >
        Sign in with the browser
      </Button>
    </div>
  );
  const tokenForm = (
    <form
      className="space-y-3 rounded-md border border-line p-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (!busy && token.trim() && url) void signInWithToken();
      }}
    >
      <p className="text-fg-muted">
        {browserFirst ? "Or use a personal access token. " : ""}
        {provider.tokenHint}{" "}
        <button
          type="button"
          className="inline-flex items-center gap-1 text-accent hover:underline disabled:opacity-40"
          disabled={!url}
          onClick={() => void openUrl(provider.tokenPage(url))}
        >
          Create one
          <ExternalLink className="size-3" />
        </button>
      </p>
      <div className="grid grid-cols-2 gap-3">
        {provider.login && (
          <Field label={provider.login.label} hint={provider.login.hint}>
            <TextInput value={login} onChange={(e) => setLogin(e.target.value)} />
          </Field>
        )}
        <Field label="Token">
          <TextInput type="password" value={token} onChange={(e) => setToken(e.target.value)} />
        </Field>
      </div>
      <Button type="submit" disabled={busy || !token.trim() || !url}>
        {busy ? "Checking…" : "Sign in with the token"}
      </Button>
    </form>
  );

  const needsServer = !publicUrl;
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <Field label="Service">
          <Select
            value={kind}
            disabled={!!initial}
            onChange={(e) => {
              setKind(e.target.value as ProviderKind);
              setServer("");
              setClientId(null);
            }}
          >
            {PROVIDER_KINDS.map((k) => (
              <option key={k} value={k}>
                {PROVIDERS[k].label}
              </option>
            ))}
          </Select>
        </Field>
        {provider.serverHint && (
          <Field
            label={needsServer ? "Server address" : "Server address (self-hosted only)"}
            hint={needsServer ? undefined : `Leave empty for ${shortUrl(publicUrl ?? "")}.`}
          >
            <TextInput
              value={server}
              disabled={!!initial}
              placeholder={provider.serverHint}
              onChange={(e) => setServer(e.target.value)}
            />
          </Field>
        )}
      </div>

      {browserFirst ? (
        <>
          {browserPanel}
          {tokenForm}
        </>
      ) : (
        <>
          {tokenForm}
          {browserSignIn && (
            <details className="rounded-md border border-line p-3">
              <summary className="cursor-default text-fg-muted">
                Or sign in through the browser with your own OAuth application
              </summary>
              <div className="mt-3">{browserPanel}</div>
            </details>
          )}
        </>
      )}
    </div>
  );
}

function DeviceCodePanel({ start, onCancel }: { start: DeviceStart; onCancel: () => void }) {
  const { code, flow } = start;
  // Leaving the settings stops waiting.
  useEffect(() => () => void ipc.hostingDeviceCancel(flow), [flow]);
  return (
    <div className="space-y-3 rounded-md border border-line p-4" aria-live="polite">
      <p className="text-fg-muted">
        Enter this code on the page that opened in your browser, then approve Git Ronin:
      </p>
      <div className="flex items-center gap-3">
        <span className="font-mono text-2xl font-semibold tracking-widest select-text">
          {code.userCode}
        </span>
        <Button onClick={() => void copyText(code.userCode, "Code copied")}>
          <Copy className="size-3.5" />
          Copy
        </Button>
      </div>
      <div className="flex items-center gap-2">
        <Button onClick={() => void openUrl(code.verificationUriComplete ?? code.verificationUri)}>
          <ExternalLink className="size-3.5" />
          Open {shortUrl(code.verificationUri)}
        </Button>
        <span className="text-xs text-fg-faint">Waiting for approval…</span>
        <Button variant="ghost" className="ml-auto" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
