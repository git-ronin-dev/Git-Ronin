import { clsx } from "clsx";
import { Lock } from "lucide-react";
import { useState } from "react";

import type { HostedRepo } from "../../bindings/HostedRepo";
import { Checkbox, Select, TextInput } from "../../ui/Field";
import { PROVIDERS } from "./providers";
import { useHostedRepos, useProfileAccounts } from "./queries";

/** Repositories of the profile's accounts, to clone one; `onPick` gets its clone URL. */
export function RepoPicker({ url, onPick }: { url: string; onPick: (url: string) => void }) {
  const accounts = useProfileAccounts().filter((a) => a.capabilities.repos && a.signedIn);
  const [chosen, setChosen] = useState<string | null>(null);
  const account = accounts.find((a) => a.account.id === chosen) ?? accounts[0];
  const repos = useHostedRepos(account?.account.id ?? null);
  const [filter, setFilter] = useState("");
  const [ssh, setSsh] = useState(false);
  if (!account) return null;

  const cloneUrl = (r: HostedRepo) => (ssh && r.cloneSsh ? r.cloneSsh : r.cloneHttps);
  const needle = filter.trim().toLowerCase();
  const shown = (repos.data ?? []).filter(
    (r) => r.path.toLowerCase().includes(needle) || r.description?.toLowerCase().includes(needle),
  );
  return (
    <div className="space-y-2 rounded-md border border-line p-2">
      <div className="flex gap-2">
        {accounts.length > 1 && (
          <div className="w-44 shrink-0">
            <Select
              aria-label="Account"
              value={account.account.id}
              onChange={(e) => setChosen(e.target.value)}
            >
              {accounts.map((a) => (
                <option key={a.account.id} value={a.account.id}>
                  {a.account.username} · {PROVIDERS[a.account.kind].label}
                </option>
              ))}
            </Select>
          </div>
        )}
        <TextInput
          aria-label="Filter repositories"
          placeholder={`Your repositories on ${PROVIDERS[account.account.kind].label}`}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      <ul className="max-h-44 overflow-y-auto" aria-label="Repositories">
        {repos.isError && <li className="p-2 text-xs text-danger">{String(repos.error)}</li>}
        {!repos.data && !repos.isError && <li className="p-2 text-xs text-fg-faint">Loading…</li>}
        {repos.data && shown.length === 0 && (
          <li className="p-2 text-xs text-fg-faint">No repositories match.</li>
        )}
        {shown.map((r) => (
          <li key={r.path}>
            <button
              type="button"
              title={r.description ?? r.path}
              aria-selected={cloneUrl(r) === url}
              onClick={() => onPick(cloneUrl(r))}
              className={clsx(
                "flex h-7 w-full items-center gap-2 rounded-sm px-2 text-left",
                cloneUrl(r) === url ? "bg-accent/20" : "hover:bg-hover",
              )}
            >
              <span className="truncate text-fg">{r.path}</span>
              {r.private && <Lock aria-label="private" className="size-3 shrink-0 text-fg-faint" />}
              {r.fork && <span className="shrink-0 text-[10px] text-fg-faint">fork</span>}
            </button>
          </li>
        ))}
      </ul>
      <Checkbox label="Clone over SSH" checked={ssh} onChange={setSsh} />
    </div>
  );
}
