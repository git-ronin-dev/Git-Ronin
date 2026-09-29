import { create } from "zustand";

/** The commit message being written in a repository tab. */
export interface CommitDraft {
  summary: string;
  body: string;
  amend: boolean;
  signoff: boolean;
}

export const emptyDraft: CommitDraft = { summary: "", body: "", amend: false, signoff: false };

interface DraftState {
  drafts: Record<string, CommitDraft>;
  update: (repo: string, patch: Partial<CommitDraft>) => void;
  clear: (repo: string) => void;
}

/** Kept apart from the view state so typing doesn't re-render the graph. */
export const useDrafts = create<DraftState>()((set) => ({
  drafts: {},
  update: (repo, patch) =>
    set((s) => ({
      drafts: { ...s.drafts, [repo]: { ...(s.drafts[repo] ?? emptyDraft), ...patch } },
    })),
  clear: (repo) =>
    set((s) => ({
      // Sign-off is a habit, not part of one message.
      drafts: { ...s.drafts, [repo]: { ...emptyDraft, signoff: s.drafts[repo]?.signoff ?? false } },
    })),
}));

export function useDraft(repo: string): CommitDraft {
  return useDrafts((s) => s.drafts[repo] ?? emptyDraft);
}

/** Summary and body as one message, or "" if there is no summary. */
export function composeMessage({ summary, body }: Pick<CommitDraft, "summary" | "body">): string {
  const head = summary.trim();
  if (!head) return "";
  const rest = body.replace(/\s+$/, "").replace(/^\s*\n/, "");
  return rest ? `${head}\n\n${rest}\n` : `${head}\n`;
}

/** Splits a commit message into summary and body. */
export function splitMessage(message: string): Pick<CommitDraft, "summary" | "body"> {
  const newline = message.indexOf("\n");
  if (newline < 0) return { summary: message.trim(), body: "" };
  return {
    summary: message.slice(0, newline).trim(),
    body: message
      .slice(newline + 1)
      .replace(/^\s*\n/, "")
      .trimEnd(),
  };
}
