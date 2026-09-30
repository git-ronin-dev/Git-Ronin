import type { Config } from "../bindings/Config";
import type { GraphRow } from "../bindings/GraphRow";
import type { Refs } from "../bindings/Refs";
import type { RepoInfo } from "../bindings/RepoInfo";

export const repoInfo: RepoInfo = {
  path: "/work/ronin",
  name: "ronin",
  isBare: false,
  head: { kind: "branch", name: "main", unborn: false },
  operation: null,
  rebase: null,
};

export const config: Config = {
  portable: {
    ui: {
      theme: "dark",
      showAvatars: false,
      diffView: "unified",
      ignoreWhitespace: false,
      fileTree: false,
      graphWidth: 0,
      terminalFontSize: 13,
    },
    git: { autoFetchMinutes: 0 },
    keybindings: {},
    profiles: [
      {
        id: "default",
        name: "Default",
        userName: "",
        userEmail: "",
        signingKey: "",
        signingFormat: "openpgp",
        signCommits: false,
        sshKey: "",
      },
    ],
  },
  local: {
    recentRepos: ["/work/ronin"],
    openTabs: [],
    activeTab: null,
    repos: {},
    activeProfile: "default",
    profileTabs: {},
    workspaces: [],
    sync: null,
    terminalShell: "",
    accounts: [],
    checkUpdates: true,
  },
};

export const refs: Refs = {
  local: [
    {
      name: "main",
      fullName: "refs/heads/main",
      oid: "a".repeat(40),
      isHead: true,
      upstream: { name: "origin/main", ahead: 2, behind: 0, gone: false },
      worktree: null,
    },
    {
      name: "feature/login",
      fullName: "refs/heads/feature/login",
      oid: "b".repeat(40),
      isHead: false,
      upstream: null,
      worktree: null,
    },
  ],
  remotes: [
    {
      name: "origin",
      url: "https://example.com/ronin.git",
      pushUrl: null,
      branches: [{ name: "main", fullName: "refs/remotes/origin/main", oid: "a".repeat(40) }],
    },
  ],
  tags: [],
  stashes: [],
  submodules: [],
};

export function row(oid: string, summary: string, patch: Partial<GraphRow> = {}): GraphRow {
  return {
    oid: oid.padEnd(40, "0"),
    parents: [],
    summary,
    authorName: "Ronin Test",
    authorEmail: "test@ronin.invalid",
    time: 1_700_000_000,
    refs: [],
    lane: 0,
    edges: [],
    ...patch,
  };
}
