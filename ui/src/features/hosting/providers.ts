import type { ProviderKind } from "../../bindings/ProviderKind";

export interface ProviderInfo {
  label: string;
  /** Address of the public service; null when there is none (self-hosted only). */
  publicUrl: string | null;
  /** Placeholder for a self-hosted address. */
  serverHint: string;
  /** The service takes a user name or email along with the token. */
  login: { label: string; hint: string; required: boolean } | null;
  /** Where to create a token, and what it needs. */
  tokenPage: (url: string) => string;
  tokenHint: string;
  /** Offered in the add-account list. */
  order: number;
}

export const PROVIDERS: Record<ProviderKind, ProviderInfo> = {
  github: {
    label: "GitHub",
    publicUrl: "https://github.com",
    serverHint: "https://github.example.com (Enterprise Server)",
    login: null,
    tokenPage: (url) =>
      `${url}/settings/tokens/new?scopes=repo,read:org,read:user,write:public_key&description=Git%20Ronin`,
    tokenHint:
      "A classic token with the repo, read:org, read:user and write:public_key scopes, or a fine-grained token with access to your repositories (contents, pull requests, issues, commit statuses).",
    order: 0,
  },
  gitlab: {
    label: "GitLab",
    publicUrl: "https://gitlab.com",
    serverHint: "https://gitlab.example.com (self-managed)",
    login: null,
    tokenPage: (url) => `${url}/-/user_settings/personal_access_tokens?name=Git%20Ronin&scopes=api`,
    tokenHint: "A personal access token with the api scope.",
    order: 1,
  },
  bitbucket: {
    label: "Bitbucket Cloud",
    publicUrl: "https://bitbucket.org",
    serverHint: "",
    login: {
      label: "Atlassian account email",
      hint: "Leave empty for a workspace or repository access token.",
      required: false,
    },
    tokenPage: () => "https://id.atlassian.com/manage-profile/security/api-tokens",
    tokenHint:
      "An Atlassian API token with Bitbucket scopes (account, repositories, pull requests, issues, pipelines and SSH keys).",
    order: 2,
  },
  bitbucketServer: {
    label: "Bitbucket Data Center",
    publicUrl: null,
    serverHint: "https://bitbucket.example.com",
    login: null,
    tokenPage: (url) => `${url}/plugins/servlet/access-tokens/users/manage`,
    tokenHint: "An HTTP access token with project and repository write permission.",
    order: 3,
  },
  azureDevops: {
    label: "Azure DevOps",
    publicUrl: null,
    serverHint: "https://dev.azure.com/your-organization",
    login: null,
    tokenPage: (url) => `${url}/_usersSettings/tokens`,
    tokenHint:
      "A personal access token with Code (read & write), Work Items (read & write) and Build (read) scopes.",
    order: 4,
  },
  jira: {
    label: "Jira",
    publicUrl: null,
    serverHint: "https://your-site.atlassian.net or https://jira.example.com",
    login: {
      label: "Atlassian account email",
      hint: "For Jira Cloud. Leave empty for a Data Center personal access token.",
      required: false,
    },
    tokenPage: (url) =>
      url.includes(".atlassian.net")
        ? "https://id.atlassian.com/manage-profile/security/api-tokens"
        : `${url}/secure/ViewProfile.jspa?selectedTab=com.atlassian.pats.pats-plugin:jira-user-personal-access-tokens`,
    tokenHint: "An API token (Cloud) or a personal access token (Data Center).",
    order: 5,
  },
};

export const PROVIDER_KINDS = (Object.keys(PROVIDERS) as ProviderKind[]).sort(
  (a, b) => PROVIDERS[a].order - PROVIDERS[b].order,
);

/** "Pull request" or GitLab's "merge request". */
export function prNoun(kind: ProviderKind | undefined): string {
  return kind === "gitlab" ? "merge request" : "pull request";
}

/** `!12` on GitLab, `#12` elsewhere. */
export function prRef(kind: ProviderKind | undefined, number: number): string {
  return `${kind === "gitlab" ? "!" : "#"}${number}`;
}

/** The web address with its scheme and a trailing slash removed, for labels. */
export function shortUrl(url: string): string {
  return url.replace(/^https?:\/\//, "").replace(/\/+$/, "");
}
