import { clsx } from "clsx";
import { useEffect, useState } from "react";
import { Group, Panel, usePanelRef, type PanelImperativeHandle } from "react-resizable-panels";

import { ChangesPanel } from "../features/changes/ChangesPanel";
import { WorkingDiffPanel } from "../features/changes/WorkingDiffPanel";
import { CommitPanel } from "../features/commit/CommitPanel";
import { DiffPanel } from "../features/commit/DiffPanel";
import { ConflictEditor } from "../features/conflict/ConflictEditor";
import { GraphView } from "../features/graph/GraphView";
import { PullRequestView } from "../features/hosting/PullRequestView";
import { BlamePanel } from "../features/history/BlamePanel";
import { FileHistoryPanel } from "../features/history/FileHistoryPanel";
import { OperationBanner } from "../features/ops/OperationBanner";
import { RebaseEditor } from "../features/rebase/RebaseEditor";
import { Sidebar } from "../features/refs/Sidebar";
import { TerminalPanel } from "../features/terminal/TerminalPanel";
import { useWorkspace } from "../features/workspace/store";
import { WORKING_COPY, updateView, useRepoView } from "../features/workspace/view";
import { ResizeHandle } from "../ui/ResizeHandle";
import { EmptyState } from "./EmptyState";
import { useOverlays } from "./overlays";
import { RepoTabs } from "./RepoTabs";
import { StatusBar } from "./StatusBar";
import { Toolbar } from "./Toolbar";

/**
 * A floating panel: rounded, outlined, on the base colour. Inside the
 * Panel, whose own box clips it away when collapsed.
 */
const card = "h-full overflow-hidden rounded-panel border border-line";

function toggle(panel: PanelImperativeHandle | null) {
  if (!panel) return;
  if (panel.isCollapsed()) panel.expand();
  else panel.collapse();
}

export function AppShell() {
  const { tabs, active } = useWorkspace();
  const sidebar = usePanelRef();
  const details = usePanelRef();

  useEffect(() => {
    useOverlays.setState({
      toggleSidebar: () => toggle(sidebar.current),
      toggleDetails: () => toggle(details.current),
    });
  }, [sidebar, details]);

  return (
    <div className="flex h-full flex-col bg-base">
      <RepoTabs />
      <Toolbar
        hasRepo={active !== null}
        onToggleSidebar={() => toggle(sidebar.current)}
        onToggleDetails={() => toggle(details.current)}
        onSearch={() => active && updateView(active, { search: { query: "", byPath: false } })}
      />
      <Group id="main-layout" className="min-h-0 flex-1 px-1.5">
        <Panel
          id="sidebar"
          panelRef={sidebar}
          defaultSize={240}
          minSize={180}
          maxSize={420}
          collapsible
          collapsedSize={0}
        >
          <div className={clsx(card, "bg-surface")}>
            {active && <Sidebar key={active} repo={active} />}
          </div>
        </Panel>
        <ResizeHandle />
        <Panel id="graph" minSize={320}>
          <main className="h-full">
            {tabs.length === 0 && (
              <div className={clsx(card, "bg-canvas")}>
                <EmptyState />
              </div>
            )}
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
        >
          <aside aria-label="Details" className={clsx(card, "bg-surface")}>
            {active && <Details repo={active} />}
          </aside>
        </Panel>
      </Group>
      <StatusBar />
    </div>
  );
}

/** Graph, or an open file (or the interactive rebase editor) on top of it; the terminal below. */
function RepoMain({ repo }: { repo: string }) {
  const { openFile, rebase, terminal, pullRequest } = useRepoView(repo);
  const panel = usePanelRef();
  // The shell starts the first time the terminal is shown and then keeps running.
  const [started, setStarted] = useState(terminal);
  if (terminal && !started) setStarted(true);

  useEffect(() => {
    const p = panel.current;
    if (!p) return;
    if (terminal && p.isCollapsed()) p.expand();
    else if (!terminal && !p.isCollapsed()) p.collapse();
  }, [terminal, started, panel]);

  const main = (
    <div className={clsx(card, "flex flex-col bg-canvas")}>
      <OperationBanner repo={repo} />
      <div className="min-h-0 flex-1">
        <div className={clsx("h-full", (openFile || rebase || pullRequest) && "hidden")}>
          <GraphView repo={repo} />
        </div>
        {/* Kept while one of its files is open, so drafts and tabs survive. */}
        {pullRequest && (
          <div className={clsx("h-full", (openFile || rebase) && "hidden")}>
            <PullRequestView
              key={`${pullRequest.account}:${pullRequest.path}:${pullRequest.number}`}
              repo={repo}
              pr={pullRequest}
            />
          </div>
        )}
        {rebase ? (
          <RebaseEditor key={rebase.base ?? ""} repo={repo} base={rebase.base} />
        ) : (
          <OpenFileView repo={repo} />
        )}
      </div>
    </div>
  );
  if (!started) return main;
  return (
    <Group id={`repo-main-${repo}`} orientation="vertical" className="h-full">
      <Panel id="main" minSize={160}>
        {main}
      </Panel>
      <ResizeHandle horizontal />
      <Panel
        id="terminal"
        panelRef={panel}
        defaultSize={260}
        minSize={100}
        collapsible
        collapsedSize={0}
        onResize={(size) => {
          // Dragged shut.
          if (size.inPixels === 0 && terminal) updateView(repo, { terminal: false });
        }}
      >
        <div className={card}>
          <TerminalPanel repo={repo} visible={terminal} />
        </div>
      </Panel>
    </Group>
  );
}

function OpenFileView({ repo }: { repo: string }) {
  const { openFile } = useRepoView(repo);
  switch (openFile?.kind) {
    case "commit":
      return <DiffPanel repo={repo} oid={openFile.oid} file={openFile.file} base={openFile.base} />;
    case "working":
      return <WorkingDiffPanel repo={repo} path={openFile.path} staged={openFile.staged} />;
    case "conflict":
      return <ConflictEditor key={openFile.path} repo={repo} path={openFile.path} />;
    case "blame":
      return <BlamePanel repo={repo} path={openFile.path} rev={openFile.rev} />;
    case "history":
      return <FileHistoryPanel key={openFile.path} repo={repo} path={openFile.path} />;
    case undefined:
      return null;
  }
}

/** Staging and commit composer for uncommitted changes, else the selected commit. */
function Details({ repo }: { repo: string }) {
  const { selected } = useRepoView(repo);
  return selected === WORKING_COPY ? <ChangesPanel repo={repo} /> : <CommitPanel repo={repo} />;
}
