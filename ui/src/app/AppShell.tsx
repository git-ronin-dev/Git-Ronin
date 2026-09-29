import { useEffect } from "react";
import { Group, Panel, usePanelRef, type PanelImperativeHandle } from "react-resizable-panels";

import { describeHead } from "../features/repo/head";
import { useRepoStore } from "../features/repo/store";
import { ResizeHandle } from "../ui/ResizeHandle";
import { EmptyState } from "./EmptyState";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { Toolbar } from "./Toolbar";

function toggle(panel: PanelImperativeHandle | null) {
  if (!panel) return;
  if (panel.isCollapsed()) panel.expand();
  else panel.collapse();
}

export function AppShell() {
  const repo = useRepoStore((s) => s.repo);
  const pickAndOpen = useRepoStore((s) => s.pickAndOpen);
  const sidebar = usePanelRef();
  const details = usePanelRef();

  // Minimal global shortcuts until the command palette lands (Phase 5).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        void pickAndOpen();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pickAndOpen]);

  return (
    <div className="flex h-full flex-col">
      <Toolbar
        onToggleSidebar={() => toggle(sidebar.current)}
        onToggleDetails={() => toggle(details.current)}
      />
      <Group id="main-layout" className="min-h-0 flex-1">
        <Panel
          id="sidebar"
          panelRef={sidebar}
          defaultSize={240}
          minSize={180}
          maxSize={420}
          collapsible
          collapsedSize={0}
          className="bg-surface"
        >
          <Sidebar />
        </Panel>
        <ResizeHandle />
        <Panel id="graph" minSize={320}>
          <main className="h-full">
            {repo ? (
              <div className="flex h-full flex-col items-center justify-center gap-1 text-fg-muted">
                <p className="text-base text-fg">{repo.name}</p>
                <p>{describeHead(repo.head)}</p>
                <p className="mt-2 text-xs text-fg-faint">Commit graph arrives in Phase 1.</p>
              </div>
            ) : (
              <EmptyState />
            )}
          </main>
        </Panel>
        <ResizeHandle />
        <Panel
          id="details"
          panelRef={details}
          defaultSize={320}
          minSize={240}
          maxSize={560}
          collapsible
          collapsedSize={0}
          className="bg-surface"
        >
          <aside
            aria-label="Details"
            className="flex h-full items-center justify-center p-4 text-fg-faint"
          >
            {repo && "Select a commit to see its details."}
          </aside>
        </Panel>
      </Group>
      <StatusBar />
    </div>
  );
}
