import { clsx } from "clsx";
import { Group, Panel, usePanelRef, type PanelImperativeHandle } from "react-resizable-panels";

import { ChangesPanel } from "../features/changes/ChangesPanel";
import { WorkingDiffPanel } from "../features/changes/WorkingDiffPanel";
import { CommitPanel } from "../features/commit/CommitPanel";
import { DiffPanel } from "../features/commit/DiffPanel";
import { GraphView } from "../features/graph/GraphView";
import { OperationBanner } from "../features/ops/OperationBanner";
import { Sidebar } from "../features/refs/Sidebar";
import { useWorkspace } from "../features/workspace/store";
import { WORKING_COPY, updateView, useRepoView } from "../features/workspace/view";
import { ResizeHandle } from "../ui/ResizeHandle";
import { EmptyState } from "./EmptyState";
import { RepoTabs } from "./RepoTabs";
import { StatusBar } from "./StatusBar";
import { Toolbar } from "./Toolbar";

function toggle(panel: PanelImperativeHandle | null) {
  if (!panel) return;
  if (panel.isCollapsed()) panel.expand();
  else panel.collapse();
}

export function AppShell() {
  const { tabs, active } = useWorkspace();
  const sidebar = usePanelRef();
  const details = usePanelRef();

  return (
    <div className="flex h-full flex-col">
      <RepoTabs />
      <Toolbar
        hasRepo={active !== null}
        onToggleSidebar={() => toggle(sidebar.current)}
        onToggleDetails={() => toggle(details.current)}
        onSearch={() => active && updateView(active, { search: { query: "", byPath: false } })}
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
          {active && <Sidebar key={active} repo={active} />}
        </Panel>
        <ResizeHandle />
        <Panel id="graph" minSize={320}>
          <main className="h-full">
            {tabs.length === 0 && <EmptyState />}
            {/* Every tab stays mounted so switching keeps scroll positions. */}
            {tabs.map((tab) => (
              <div key={tab.path} className={clsx("h-full", tab.path !== active && "hidden")}>
                <RepoMain repo={tab.path} />
              </div>
            ))}
          </main>
        </Panel>
        <ResizeHandle />
        <Panel
          id="details"
          panelRef={details}
          defaultSize={340}
          minSize={260}
          maxSize={600}
          collapsible
          collapsedSize={0}
          className="bg-surface"
        >
          <aside aria-label="Details" className="h-full">
            {active && <Details repo={active} />}
          </aside>
        </Panel>
      </Group>
      <StatusBar />
    </div>
  );
}

/** Graph, or the diff of an open file on top of it. */
function RepoMain({ repo }: { repo: string }) {
  const { openFile } = useRepoView(repo);
  return (
    <div className="flex h-full flex-col">
      <OperationBanner repo={repo} />
      <div className="min-h-0 flex-1">
        <div className={clsx("h-full", openFile && "hidden")}>
          <GraphView repo={repo} />
        </div>
        {openFile?.kind === "commit" && (
          <DiffPanel repo={repo} oid={openFile.oid} file={openFile.file} />
        )}
        {openFile?.kind === "working" && (
          <WorkingDiffPanel repo={repo} path={openFile.path} staged={openFile.staged} />
        )}
      </div>
    </div>
  );
}

/** Staging and commit composer for uncommitted changes, else the selected commit. */
function Details({ repo }: { repo: string }) {
  const { selected } = useRepoView(repo);
  return selected === WORKING_COPY ? <ChangesPanel repo={repo} /> : <CommitPanel repo={repo} />;
}
