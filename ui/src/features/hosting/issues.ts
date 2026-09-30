import { useQueryClient } from "@tanstack/react-query";

import type { Issue } from "../../bindings/Issue";
import type { ProviderKind } from "../../bindings/ProviderKind";
import { ipc } from "../../lib/ipc";
import { toast } from "../../ui/toast-store";
import { gitActions } from "../ops/actions";

/**
 * A branch name for working on an issue, `12-fix-the-login-page` or
 * `PROJ-12-fix-the-login-page` (Jira recognises its keys in branch names).
 */
export function issueBranchName(issue: Pick<Issue, "key" | "title">): string {
  const key = issue.key.replace(/^#/, "");
  let slug = "";
  for (const word of issue.title.toLowerCase().split(/[^\p{L}\p{N}]+/u)) {
    if (!word) continue;
    if (slug.length + word.length > 40) break;
    slug += (slug ? "-" : "") + word;
  }
  return slug ? `${key}-${slug}` : key;
}

/** Starts a branch for an issue at HEAD, checks it out and remembers the issue. */
export function useStartIssue(repo: string) {
  const client = useQueryClient();
  return async (issue: Pick<Issue, "key" | "title">) => {
    const name = issueBranchName(issue);
    const created = await gitActions(client, repo).createBranch(name, "HEAD", true);
    if (!created) return;
    try {
      await ipc.setBranchIssue(repo, name, issue.key);
    } catch (err) {
      toast.error("Could not link the branch to the issue", String(err));
    }
  };
}

/**
 * Words in a pull request description that close the issue when it's
 * merged, or for Jira keys (which the title carries) nothing.
 */
export function closingText(kind: ProviderKind | undefined, issue: string): string {
  if (!issue.startsWith("#")) return "";
  return kind === "azureDevops" ? `Fixes AB${issue}` : `Closes ${issue}`;
}
