import type { QueryClient } from "@tanstack/react-query";

import { openSettings, useOverlays } from "../../app/overlays";
import type { AppInfo } from "../../bindings/AppInfo";
import type { Config } from "../../bindings/Config";
import type { JournalState } from "../../bindings/JournalState";
import type { Refs } from "../../bindings/Refs";
import type { RepoInfo } from "../../bindings/RepoInfo";
import type { RepoLink } from "../../bindings/RepoLink";
import type { SyncStatus } from "../../bindings/SyncStatus";
import type { Theme } from "../../bindings/Theme";
import type { UiPrefs } from "../../bindings/UiPrefs";
import type { WorkingStatus } from "../../bindings/WorkingStatus";
import { ipc } from "../../lib/ipc";
import { toast } from "../../ui/toast-store";
import { countChanges } from "../changes/queries";
import { gitActions, PULL_LABELS } from "../ops/actions";
import { openDialog } from "../ops/dialog-store";
import { describeHead } from "../repo/head";
import { exportSettings, importSettings, syncKeys } from "../settings/queries";
import { checkForUpdates } from "../updates/store";
import { invalidateRepo, keys } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { updateView, useViews, WORKING_COPY } from "../workspace/view";
import { openWorkspace } from "../workspaces/store";
import { parseBinding } from "./keys";

/** What a command runs against. */
export interface CommandContext {
  client: QueryClient;
  /** The active repository tab. */
  repo: string | null;
}

export interface Command {
  /** Stable; keys the user's shortcut overrides. */
  id: string;
  title: string;
  category: string;
  /** Default shortcuts. */
  keys?: string[];
  /** The shortcut also works while typing in a text field. */
  inInput?: boolean;
  /** The shortcut also works while a dialog is open. */
  inDialog?: boolean;
  /** Commands made from current data (tabs, branches, …) can't be bound. */
  dynamic?: boolean;
  enabled?: (ctx: CommandContext) => boolean;
  run: (ctx: CommandContext) => unknown;
}

const hasRepo = (ctx: CommandContext) => ctx.repo !== null;

function data<T>(ctx: CommandContext, key: readonly unknown[]): T | undefined {
  return ctx.client.getQueryData<T>(key);
}

function refs(ctx: CommandContext) {
  return ctx.repo ? data<Refs>(ctx, keys.refs(ctx.repo)) : undefined;
}

function currentBranch(ctx: CommandContext) {
  return refs(ctx)?.local.find((b) => b.isHead);
}

/** A remote of the active repository on a signed-in service that can do `what`. */
function link(ctx: CommandContext, what: "pullRequests" | "issues") {
  if (!ctx.repo) return undefined;
  const cached = ctx.client.getQueriesData<RepoLink[]>({ queryKey: keys.hostingLinks(ctx.repo) });
  return cached.flatMap(([, links]) => links ?? []).find((l) => l.capabilities[what]);
}

type RepoContext = CommandContext & { repo: string };

function repoCommand(
  id: string,
  title: string,
  run: (actions: ReturnType<typeof gitActions>, ctx: RepoContext) => unknown,
  extra: Partial<Command> = {},
): Command {
  return {
    id,
    title,
    category: "Repository",
    ...extra,
    enabled: (ctx) => hasRepo(ctx) && (extra.enabled?.(ctx) ?? true),
    run: (ctx) => ctx.repo && run(gitActions(ctx.client, ctx.repo), { ...ctx, repo: ctx.repo }),
  };
}

function switchTab(step: number) {
  const ws = useWorkspace.getState();
  if (ws.tabs.length < 2) return;
  const i = ws.tabs.findIndex((t) => t.path === ws.active);
  const next = (i + step + ws.tabs.length) % ws.tabs.length;
  ws.activate(ws.tabs[next]!.path);
}

function undoStep(ctx: CommandContext, redo: boolean) {
  if (!ctx.repo) return;
  const journal = data<JournalState>(ctx, keys.journal(ctx.repo));
  const step = redo ? journal?.redo : journal?.undo;
  if (step?.blocked) toast.info(`Can't undo “${step.label}”`, step.blocked);
  else if (step) void gitActions(ctx.client, ctx.repo).undo(redo);
}

const THEMES: Theme[] = ["dark", "light", "system"];

/** Changes UI preferences from a command; resolves to whether they were saved. */
async function updateUi(
  ctx: CommandContext,
  patch: (ui: UiPrefs) => Partial<UiPrefs>,
): Promise<boolean> {
  const config = data<Config>(ctx, keys.config);
  if (!config) return false;
  const ui = { ...config.portable.ui, ...patch(config.portable.ui) };
  ctx.client.setQueryData<Config>(keys.config, {
    ...config,
    portable: { ...config.portable, ui },
  });
  try {
    await ipc.configSetUi(ui);
    return true;
  } catch (err) {
    toast.error("Could not save settings", String(err));
    return false;
  }
}

async function cycleTheme(ctx: CommandContext) {
  let theme: Theme = "system";
  const saved = await updateUi(ctx, (ui) => {
    theme = THEMES[(THEMES.indexOf(ui.theme) + 1) % THEMES.length]!;
    return { theme };
  });
  if (saved) toast.info(`Theme: ${theme}`);
}

/** Every command with a fixed id, in the order the palette lists them. */
export const COMMANDS: Command[] = [
  {
    id: "palette.open",
    title: "Show all commands",
    category: "App",
    keys: ["Mod+P", "Mod+Shift+P"],
    inInput: true,
    inDialog: true,
    run: () => useOverlays.setState((s) => ({ palette: !s.palette })),
  },
  {
    id: "settings.open",
    title: "Settings",
    category: "App",
    keys: ["Mod+,"],
    inInput: true,
    run: () => openSettings("general"),
  },
  {
    id: "settings.keyboard",
    title: "Keyboard shortcuts",
    category: "App",
    run: () => openSettings("keyboard"),
  },
  {
    id: "settings.profiles",
    title: "Manage profiles…",
    category: "App",
    run: () => openSettings("profiles"),
  },
  { id: "settings.ssh", title: "SSH keys…", category: "App", run: () => openSettings("ssh") },
  {
    id: "app.checkUpdates",
    title: "Check for updates",
    category: "App",
    enabled: (ctx) => data<AppInfo>(ctx, ["appInfo"])?.updatesUnavailable === null,
    run: () => checkForUpdates(true),
  },
  {
    id: "settings.export",
    title: "Export settings…",
    category: "App",
    run: () => void exportSettings(),
  },
  {
    id: "settings.import",
    title: "Import settings…",
    category: "App",
    run: () => void importSettings(),
  },
  {
    id: "sync.now",
    title: "Sync settings now",
    category: "App",
    enabled: (ctx) => !!data<SyncStatus>(ctx, syncKeys.status)?.settings,
    run: async (ctx) => {
      try {
        const status = await ipc.syncNow();
        ctx.client.setQueryData(syncKeys.status, status);
        if (status.error) toast.error("Could not sync settings", status.error);
        else if (status.conflicts.length > 0) openSettings("sync");
        else toast.success("Settings synced");
      } catch (err) {
        toast.error("Could not sync settings", String(err));
      }
    },
  },
  { id: "theme.cycle", title: "Change theme", category: "App", run: (ctx) => cycleTheme(ctx) },

  {
    id: "repo.open",
    title: "Open repository…",
    category: "Tabs",
    keys: ["Mod+O"],
    inInput: true,
    run: () => void useWorkspace.getState().pickAndOpen(),
  },
  {
    id: "repo.clone",
    title: "Clone repository…",
    category: "Tabs",
    run: () => openDialog({ kind: "clone" }),
  },
  {
    id: "repo.init",
    title: "New repository…",
    category: "Tabs",
    run: () => void useWorkspace.getState().pickAndInit(),
  },
  {
    id: "tab.close",
    title: "Close tab",
    category: "Tabs",
    keys: ["Mod+W"],
    inInput: true,
    enabled: hasRepo,
    run: (ctx) => ctx.repo && void useWorkspace.getState().close(ctx.repo),
  },
  {
    id: "tab.next",
    title: "Next tab",
    category: "Tabs",
    keys: ["Ctrl+Tab"],
    inInput: true,
    run: () => switchTab(1),
  },
  {
    id: "tab.previous",
    title: "Previous tab",
    category: "Tabs",
    keys: ["Ctrl+Shift+Tab"],
    inInput: true,
    run: () => switchTab(-1),
  },
  {
    id: "workspaces.open",
    title: "Workspaces…",
    category: "Tabs",
    run: () => useOverlays.setState({ workspaces: true, palette: false }),
  },

  {
    id: "hosting.launchpad",
    title: "Launchpad: my pull requests and issues",
    category: "Hosting",
    keys: ["Mod+Shift+L"],
    inInput: true,
    run: () => useOverlays.setState({ launchpad: true, palette: false }),
  },
  {
    id: "settings.accounts",
    title: "Accounts…",
    category: "Hosting",
    run: () => openSettings("accounts"),
  },
  repoCommand(
    "hosting.createPullRequest",
    "Create pull request…",
    (_, ctx) =>
      openDialog({
        kind: "createPullRequest",
        repo: ctx.repo,
        branch: currentBranch(ctx)?.name ?? null,
      }),
    { category: "Hosting", enabled: (ctx) => !!link(ctx, "pullRequests") },
  ),
  repoCommand(
    "hosting.createIssue",
    "New issue…",
    (_, ctx) => openDialog({ kind: "createIssue", repo: ctx.repo }),
    { category: "Hosting", enabled: (ctx) => !!link(ctx, "issues") },
  ),

  {
    id: "graph.search",
    title: "Search commits",
    category: "View",
    keys: ["Mod+F"],
    inInput: true,
    enabled: hasRepo,
    run: ({ repo }) => {
      if (!repo) return;
      const current = useViews.getState().views[repo]?.search;
      updateView(repo, { search: current ?? { query: "", byPath: false }, openFile: null });
    },
  },
  {
    id: "view.changes",
    title: "Show uncommitted changes",
    category: "View",
    enabled: hasRepo,
    run: ({ repo }) => repo && updateView(repo, { selected: WORKING_COPY, openFile: null }),
  },
  {
    id: "view.sidebar",
    title: "Toggle sidebar",
    category: "View",
    keys: ["Mod+B"],
    run: () => useOverlays.getState().toggleSidebar(),
  },
  {
    id: "view.details",
    title: "Toggle details",
    category: "View",
    run: () => useOverlays.getState().toggleDetails(),
  },
  {
    id: "view.fileTree",
    title: "Toggle folder tree for changed files",
    category: "View",
    run: (ctx) => void updateUi(ctx, (ui) => ({ fileTree: !ui.fileTree })),
  },
  {
    id: "terminal.toggle",
    title: "Toggle terminal",
    category: "View",
    keys: ["Ctrl+`"],
    inInput: true,
    enabled: hasRepo,
    run: ({ repo }) => {
      if (!repo) return;
      const open = useViews.getState().views[repo]?.terminal ?? false;
      updateView(repo, { terminal: !open });
    },
  },

  repoCommand("repo.refresh", "Refresh", (_, { client, repo }) => invalidateRepo(client, repo), {
    keys: ["Mod+R"],
    inInput: true,
  }),
  repoCommand("repo.undo", "Undo", (_, ctx) => undoStep(ctx, false), { keys: ["Mod+Z"] }),
  repoCommand("repo.redo", "Redo", (_, ctx) => undoStep(ctx, true), {
    keys: ["Mod+Shift+Z", "Mod+Y"],
  }),
  repoCommand("repo.fetchAll", "Fetch all remotes", (a) => a.fetch(null), {
    enabled: (ctx) => (refs(ctx)?.remotes.length ?? 0) > 0,
  }),
  ...(["default", "fastForwardOnly", "merge", "rebase"] as const).map((mode) =>
    repoCommand(
      mode === "default" ? "repo.pull" : `repo.pull.${mode}`,
      PULL_LABELS[mode],
      (a) => a.pull(mode),
      { enabled: (ctx) => !!currentBranch(ctx)?.upstream },
    ),
  ),
  repoCommand(
    "repo.push",
    "Push",
    (a, ctx) => {
      const branch = currentBranch(ctx);
      return branch && a.push(branch.name, branch.upstream?.name ?? null);
    },
    { enabled: (ctx) => !!currentBranch(ctx) && (refs(ctx)?.remotes.length ?? 0) > 0 },
  ),
  repoCommand(
    "branch.create",
    "Create branch…",
    (_, ctx) => {
      const info = data<RepoInfo>(ctx, keys.info(ctx.repo));
      if (info)
        openDialog({
          kind: "createBranch",
          repo: ctx.repo,
          start: "HEAD",
          startLabel: describeHead(info.head),
        });
    },
    { category: "Branch" },
  ),
  repoCommand(
    "stash.save",
    "Stash changes…",
    (_, { repo }) => useOverlays.setState({ stash: repo }),
    {
      enabled: (ctx) =>
        !!ctx.repo && countChanges(data<WorkingStatus>(ctx, keys.status(ctx.repo))) > 0,
    },
  ),
  repoCommand(
    "stash.pop",
    "Pop latest stash",
    async (_, ctx) => {
      const latest = refs(ctx)?.stashes[0];
      if (!latest) return;
      try {
        await ipc.stashApply(ctx.repo, latest.index, latest.oid, true);
      } catch (err) {
        toast.error("Could not pop stash", String(err));
      } finally {
        void invalidateRepo(ctx.client, ctx.repo);
      }
    },
    { enabled: (ctx) => (refs(ctx)?.stashes.length ?? 0) > 0 },
  ),
  ...(
    [
      ["operation.continue", "Continue merge, rebase, …", "continueOperation"],
      ["operation.skip", "Skip commit", "skipOperation"],
      ["operation.abort", "Abort merge, rebase, …", "abortOperation"],
    ] as const
  ).map(([id, title, action]) =>
    repoCommand(id, title, (a) => a[action](), {
      enabled: (ctx) => !!ctx.repo && !!data<RepoInfo>(ctx, keys.info(ctx.repo))?.operation,
    }),
  ),
];

/** Commands made from what is open now: tabs, recent repositories, workspaces, profiles, branches. */
export function dynamicCommands(ctx: CommandContext): Command[] {
  const out: Command[] = [];
  const ws = useWorkspace.getState();
  const config = data<Config>(ctx, keys.config);
  for (const tab of ws.tabs) {
    if (tab.path === ws.active) continue;
    out.push({
      id: `tab:${tab.path}`,
      title: `Switch to ${tab.name}`,
      category: "Tabs",
      dynamic: true,
      run: () => ws.activate(tab.path),
    });
  }
  for (const path of config?.local.recentRepos ?? []) {
    if (ws.tabs.some((t) => t.path === path)) continue;
    out.push({
      id: `recent:${path}`,
      title: `Open recent ${path}`,
      category: "Tabs",
      dynamic: true,
      run: () => void ws.open(path),
    });
  }
  for (const workspace of config?.local.workspaces ?? []) {
    out.push({
      id: `workspace:${workspace.name}`,
      title: `Open workspace ${workspace.name}`,
      category: "Tabs",
      dynamic: true,
      run: () => void openWorkspace(workspace),
    });
  }
  const active = activeProfileId(config);
  for (const profile of config?.portable.profiles ?? []) {
    if (profile.id === active) continue;
    out.push({
      id: `profile:${profile.id}`,
      title: `Switch to profile ${profile.name}`,
      category: "Profiles",
      dynamic: true,
      run: () => void ws.switchProfile(profile.id),
    });
  }
  if (ctx.repo) {
    const repo = ctx.repo;
    for (const branch of refs(ctx)?.local ?? []) {
      if (branch.isHead || branch.worktree) continue;
      out.push({
        id: `checkout:${branch.name}`,
        title: `Check out ${branch.name}`,
        category: "Branch",
        dynamic: true,
        run: () => void gitActions(ctx.client, repo).checkout(branch.name),
      });
    }
  }
  return out;
}

/** The profile in use: the saved one if it still exists, else the first. */
export function activeProfileId(config: Config | undefined): string | undefined {
  const profiles = config?.portable.profiles ?? [];
  const saved = config?.local.activeProfile;
  return profiles.some((p) => p.id === saved) ? saved : profiles[0]?.id;
}

/** Each command's shortcuts: the user's override, else the defaults. */
export function effectiveKeys(
  overrides: Record<string, string> | undefined,
  mac?: boolean,
): Map<string, string[]> {
  const out = new Map<string, string[]>();
  for (const command of COMMANDS) {
    const override = overrides?.[command.id];
    out.set(
      command.id,
      override !== undefined
        ? parseBinding(override, mac)
        : (command.keys ?? []).flatMap((k) => parseBinding(k, mac)),
    );
  }
  return out;
}

/** Shortcut → command, for dispatching key presses. The first command wins. */
export function shortcutMap(bindings: Map<string, string[]>): Map<string, Command> {
  const out = new Map<string, Command>();
  for (const command of COMMANDS) {
    for (const shortcut of bindings.get(command.id) ?? []) {
      if (!out.has(shortcut)) out.set(shortcut, command);
    }
  }
  return out;
}

/** Commands other than `id` bound to `shortcut`. */
export function conflictsWith(
  bindings: Map<string, string[]>,
  id: string,
  shortcut: string,
): Command[] {
  return COMMANDS.filter((c) => c.id !== id && bindings.get(c.id)?.includes(shortcut));
}
