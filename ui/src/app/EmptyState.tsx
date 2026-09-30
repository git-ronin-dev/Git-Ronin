import { clsx } from "clsx";
import type { ReactNode } from "react";

import logo from "../assets/logo.png";
import { activeProfileId } from "../features/commands/commands";
import { useShortcutLabel } from "../features/commands/useShortcuts";
import { useProfileAccounts } from "../features/hosting/queries";
import { openDialog } from "../features/ops/dialog-store";
import { useGitIdentity } from "../features/settings/queries";
import { useConfig } from "../features/workspace/queries";
import { useWorkspace } from "../features/workspace/store";
import { Button } from "../ui/Button";
import { Check, Download, FolderOpen, FolderPlus } from "../ui/icons";
import { openSettings, useOverlays } from "./overlays";

/** No repository open: the welcome screen, with first steps until one has been opened. */
export function EmptyState() {
  const { pickAndOpen, pickAndInit, open, opening } = useWorkspace();
  const config = useConfig().data;
  const recent = config?.local.recentRepos ?? [];
  const openKey = useShortcutLabel("repo.open");
  const paletteKey = useShortcutLabel("palette.open");

  return (
    <div className="flex h-full flex-col items-center overflow-y-auto p-8 text-center">
      <div className="my-auto flex w-full flex-col items-center gap-4 py-4">
        <img
          src={logo}
          alt=""
          width={208}
          height={208}
          className="animate-rn-rise drop-shadow-[0_6px_24px_rgb(0_0_0/0.35)]"
        />
        <div className="flex items-center gap-3">
          <h1 className="font-display text-3xl tracking-wide">Git Ronin</h1>
          <Hanko />
        </div>
        <p className="text-fg-muted">A masterless Git client. No account, no server.</p>
        <div className="flex flex-wrap justify-center gap-2">
          <Button variant="primary" onClick={() => void pickAndOpen()} disabled={opening}>
            <FolderOpen className="size-4" />
            Open repository…
          </Button>
          <Button onClick={() => openDialog({ kind: "clone" })} disabled={opening}>
            <Download className="size-4" />
            Clone…
          </Button>
          <Button onClick={() => void pickAndInit()} disabled={opening}>
            <FolderPlus className="size-4" />
            New repository…
          </Button>
        </div>
        <p className="text-xs text-fg-faint">
          {openKey && `${openKey} opens a repository · `}
          {paletteKey && `${paletteKey} shows every command`}
        </p>

        {recent.length > 0 ? (
          <section className="mt-6 w-full max-w-md text-left">
            <h2 className="mb-2 px-3 text-xs font-semibold tracking-wide text-fg-faint uppercase">
              Recent
            </h2>
            <ul>
              {recent.map((path) => (
                <li key={path}>
                  <button
                    type="button"
                    disabled={opening}
                    onClick={() => void open(path)}
                    className="flex w-full flex-col rounded-md px-3 py-1.5 text-left hover:bg-hover"
                  >
                    <span>{path.split(/[\\/]/).filter(Boolean).at(-1)}</span>
                    <span className="truncate text-xs text-fg-faint">{path}</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ) : (
          config && <FirstSteps paletteKey={paletteKey} />
        )}
      </div>
    </div>
  );
}

/** The seal next to the name: 浪人, "ronin", in vermilion. */
function Hanko() {
  return (
    <span
      lang="ja"
      aria-label="Ronin"
      className="rounded-sm bg-accent px-1 py-1.5 font-display text-sm leading-none text-accent-fg [writing-mode:vertical-rl]"
    >
      浪人
    </span>
  );
}

/** Setup a new user may not know about, ticked off as it gets done. */
function FirstSteps({ paletteKey }: { paletteKey?: string }) {
  const config = useConfig().data;
  const identity = useGitIdentity().data;
  const accounts = useProfileAccounts();
  const profile = config?.portable.profiles.find((p) => p.id === activeProfileId(config));
  const named =
    !!(profile?.userName || identity?.name) && !!(profile?.userEmail || identity?.email);

  return (
    <section className="mt-6 w-full max-w-md text-left" aria-label="First steps">
      <h2 className="mb-2 px-3 text-xs font-semibold tracking-wide text-fg-faint uppercase">
        First steps
      </h2>
      <ol className="space-y-1">
        <Step
          done={named}
          title="Say who you commit as"
          action={<StepButton onClick={() => openSettings("profiles")}>Profiles</StepButton>}
        >
          {named
            ? `Commits are by ${profile?.userName || identity?.name}.`
            : "Git has no name or email for your commits yet."}
        </Step>
        <Step
          done={accounts.length > 0}
          title="Connect your Git hosting"
          action={<StepButton onClick={() => openSettings("accounts")}>Accounts</StepButton>}
        >
          GitHub, GitLab, Bitbucket or Azure DevOps: pull requests, issues, checks, and no password
          prompts.
        </Step>
        <Step
          title="Everything is a command away"
          action={
            <StepButton onClick={() => useOverlays.setState({ palette: true })}>
              {paletteKey ?? "Palette"}
            </StepButton>
          }
        >
          Every action and its shortcut is in the command palette.
        </Step>
      </ol>
    </section>
  );
}

function Step({
  done = false,
  title,
  action,
  children,
}: {
  done?: boolean;
  title: string;
  action: ReactNode;
  children: ReactNode;
}) {
  return (
    <li className="flex items-start gap-3 rounded-md px-3 py-2 hover:bg-hover">
      <span
        className={clsx(
          "mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border",
          done ? "border-success bg-success text-canvas" : "border-fg-faint",
        )}
      >
        {done && <Check className="size-3" strokeWidth={3} />}
      </span>
      <span className="min-w-0 flex-1">
        <span className={clsx("block", done && "text-fg-muted")}>{title}</span>
        <span className="block text-xs text-fg-faint">{children}</span>
      </span>
      {action}
    </li>
  );
}

function StepButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <Button variant="ghost" className="h-6 shrink-0 px-2 text-xs" onClick={onClick}>
      {children}
    </Button>
  );
}
