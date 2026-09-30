import type { Workspace } from "../../bindings/Workspace";
import { useWorkspace } from "../workspace/store";

/** Opens every repository of a workspace as a tab, the first one active. */
export async function openWorkspace(workspace: Workspace) {
  const ws = useWorkspace.getState();
  for (const path of workspace.repos) {
    if (!useWorkspace.getState().tabs.some((t) => t.path === path)) await ws.open(path);
  }
  const first = workspace.repos.find((p) => useWorkspace.getState().tabs.some((t) => t.path === p));
  if (first) useWorkspace.getState().activate(first);
}
