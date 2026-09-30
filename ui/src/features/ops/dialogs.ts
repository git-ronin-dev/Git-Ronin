import { create } from "zustand";

import type { FlowKind } from "../../bindings/FlowKind";
import type { Remote } from "../../bindings/Remote";

/** Dialogs any menu can open; hosted once by `Dialogs`. */
export type DialogRequest =
  | { kind: "createBranch"; repo: string; start: string; startLabel: string }
  | { kind: "renameBranch"; repo: string; name: string }
  | { kind: "createTag"; repo: string; target: string; targetLabel: string }
  | { kind: "remote"; repo: string; remote: Remote | null }
  | { kind: "pushNew"; repo: string; branch: string }
  | { kind: "upstream"; repo: string; branch: string; upstream: string | null }
  | { kind: "clone" }
  | { kind: "addWorktree"; repo: string; branch: string | null }
  | { kind: "addSubmodule"; repo: string }
  | { kind: "lfsTrack"; repo: string; pattern: string }
  | { kind: "flowInit"; repo: string }
  | { kind: "flowStart"; repo: string; flow: FlowKind }
  | { kind: "flowFinish"; repo: string; flow: FlowKind; branch: string };

interface DialogState {
  request: DialogRequest | null;
  close: () => void;
}

export const useDialogs = create<DialogState>()((set) => ({
  request: null,
  close: () => set({ request: null }),
}));

export function openDialog(request: DialogRequest) {
  useDialogs.setState({ request });
}
