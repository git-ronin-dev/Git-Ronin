import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { clsx } from "clsx";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { GraphRow } from "../../bindings/GraphRow";
import { copyText } from "../../lib/clipboard";
import { ipc } from "../../lib/ipc";
import { formatDate, relativeTime } from "../../lib/time";
import { Avatar } from "../../ui/Avatar";
import { ContextMenu } from "../../ui/ContextMenu";
import { countChanges, useWorkingStatus } from "../changes/queries";
import { useGitActions, type GitActions } from "../ops/actions";
import { commitMenu, type RepoContext } from "../ops/menus";
import { useRepoContext } from "../ops/queries";
import { keys, useRepoInfo, useUiPrefs } from "../workspace/queries";
import { WORKING_COPY, updateView, useRepoView, type Search } from "../workspace/view";
import { LANE_WIDTH, ROW_HEIGHT } from "./geometry";
import { GraphLanes } from "./GraphLanes";
import { RefChips } from "./RefChips";
import { registerScroller, selectRow } from "./reveal";
import { SearchBar } from "./SearchBar";
import { findLoadedIndex, useGraphRows } from "./useGraphRows";
import { WorkingRow } from "./WorkingRow";

/** Wider graphs are clipped; very wide histories stay readable. */
const MAX_VISIBLE_LANES = 12;
/** Room for the column header. */
const MIN_GRAPH_WIDTH = 64;

export function GraphView({ repo }: { repo: string }) {
  const client = useQueryClient();
  const view = useRepoView(repo);
  const showAvatars = useUiPrefs()?.showAvatars ?? false;
  const scrollRef = useRef<HTMLDivElement>(null);
  const [range, setRange] = useState({ first: 0, last: 60 });
  const rows = useGraphRows(repo, range.first, range.last);
  const info = useRepoInfo(repo).data;
  const ctx = useRepoContext(repo);
  const actions = useGitActions(repo);
  const status = useWorkingStatus(repo, info !== undefined && !info.isBare);
  const changes = countChanges(status.data);
  // The uncommitted-changes row sits above the first commit.
  const top = changes > 0 ? ROW_HEIGHT : 0;

  // The React Compiler can't memoise this hook; we don't use the compiler.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.count,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    paddingStart: top,
    overscan: 20,
    onChange: (instance) => {
      const items = instance.getVirtualItems();
      const first = items[0]?.index ?? 0;
      const last = items.at(-1)?.index ?? 0;
      setRange((r) => (r.first === first && r.last === last ? r : { first, last }));
    },
  });

  useEffect(
    () =>
      registerScroller(repo, (index) =>
        virtualizer.scrollToIndex(index, { align: "auto", behavior: "auto" }),
      ),
    [repo, virtualizer],
  );

  const search = useSearch(repo, view.search);
  const [position, setPosition] = useState(0);
  const matchSet = useMemo(() => new Set(search.data ?? []), [search.data]);
  const dimming = view.search !== null && !!view.search.query && !!search.data;

  const step = useCallback(
    (delta: 1 | -1) => {
      const matches = search.data;
      if (!matches?.length) return;
      const next = (position + delta + matches.length) % matches.length;
      setPosition(next);
      void selectRow(client, repo, matches[next]!);
    },
    [client, repo, position, search.data],
  );

  // Jump to the first result whenever a new result set arrives.
  useEffect(() => {
    setPosition(0);
    const first = search.data?.[0];
    if (first !== undefined) void selectRow(client, repo, first);
  }, [client, repo, search.data]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const delta = { ArrowDown: 1, ArrowUp: -1, PageDown: 20, PageUp: -20 }[e.key];
    if (delta === undefined) return;
    e.preventDefault();
    const current =
      view.selected === WORKING_COPY
        ? -1
        : view.selected
          ? findLoadedIndex(client, repo, view.selected)
          : null;
    const target = (current ?? -1) + delta;
    if (target < 0 && changes > 0) {
      selectWorking();
      scrollRef.current?.scrollTo({ top: 0 });
      return;
    }
    if (rows.count > 0) void selectRow(client, repo, Math.max(0, Math.min(rows.count - 1, target)));
  };

  const lanes = Math.min(Math.max(rows.maxLanes, 1), MAX_VISIBLE_LANES);
  const graphWidth = Math.max(lanes * LANE_WIDTH + 8, MIN_GRAPH_WIDTH);
  const select = useCallback(
    (oid: string) => updateView(repo, { selected: oid, openFile: null }),
    [repo],
  );
  const selectWorking = useCallback(() => select(WORKING_COPY), [select]);
  const first = rows.row(0);
  const headLane = first?.refs.some((r) => r.current) ? first.lane : null;

  return (
    <div className="@container flex h-full flex-col">
      {view.search && (
        <SearchBar
          search={view.search}
          onChange={(s) => updateView(repo, { search: s })}
          matches={search.data}
          loading={search.isFetching}
          position={position}
          onStep={step}
        />
      )}
      <div className="flex h-7 shrink-0 items-center border-b border-line text-[11px] font-semibold tracking-wide text-fg-faint uppercase">
        <div style={{ width: graphWidth }} className="shrink-0 px-2">
          Graph
        </div>
        <div className="flex-1 px-2">Description</div>
        <div className="hidden w-44 px-2 @2xl:block">Author</div>
        <div className="hidden w-28 px-2 @xl:block">Date</div>
        <div className="hidden w-20 px-2 @3xl:block">Commit</div>
      </div>
      <div
        ref={scrollRef}
        tabIndex={0}
        onKeyDown={onKeyDown}
        role="grid"
        aria-label="Commit graph"
        aria-rowcount={rows.count}
        className="min-h-0 flex-1 overflow-auto outline-none"
      >
        {rows.complete && rows.count === 0 && changes === 0 ? (
          <p className="p-8 text-center text-fg-muted">No commits yet.</p>
        ) : (
          <div style={{ height: virtualizer.getTotalSize() }} className="relative">
            {changes > 0 && (
              <div style={{ height: ROW_HEIGHT }} className="absolute inset-x-0 top-0 z-10">
                <WorkingRow
                  repo={repo}
                  count={changes}
                  graphWidth={graphWidth}
                  headLane={headLane}
                  selected={view.selected === WORKING_COPY}
                  onSelect={selectWorking}
                />
              </div>
            )}
            {virtualizer.getVirtualItems().map((item) => {
              const row = rows.row(item.index);
              return (
                <div
                  key={item.key}
                  style={{ transform: `translateY(${item.start}px)`, height: ROW_HEIGHT }}
                  className="absolute inset-x-0 top-0"
                >
                  {row ? (
                    <Row
                      row={row}
                      graphWidth={graphWidth}
                      selected={row.oid === view.selected}
                      dimmed={dimming && !matchSet.has(item.index)}
                      showAvatars={showAvatars}
                      onSelect={select}
                      ctx={ctx}
                      actions={actions}
                    />
                  ) : (
                    <Placeholder graphWidth={graphWidth} />
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function useSearch(repo: string, search: Search | null) {
  const query = search?.query.trim() ?? "";
  const byPath = search?.byPath ?? false;
  return useQuery({
    queryKey: keys.search(repo, query, byPath),
    queryFn: () => ipc.graphSearch(repo, query, byPath),
    enabled: query.length > 0,
  });
}

interface RowProps {
  row: GraphRow;
  graphWidth: number;
  selected: boolean;
  dimmed: boolean;
  showAvatars: boolean;
  onSelect: (oid: string) => void;
  ctx: RepoContext;
  actions: GitActions;
}

const Row = memo(function Row({
  row,
  graphWidth,
  selected,
  dimmed,
  showAvatars,
  onSelect,
  ctx,
  actions,
}: RowProps) {
  return (
    <ContextMenu
      items={[
        ...commitMenu(actions, ctx, row.oid),
        "separator",
        { label: "Copy commit SHA", onSelect: () => void copyText(row.oid, "Copied commit SHA") },
        { label: "Copy message", onSelect: () => void copyText(row.summary, "Copied message") },
      ]}
    >
      <div
        role="row"
        aria-selected={selected}
        onMouseDown={() => onSelect(row.oid)}
        className={clsx(
          "flex h-full items-center transition-opacity",
          selected ? "bg-accent/20" : "hover:bg-hover",
          dimmed && "opacity-35",
        )}
      >
        <div style={{ width: graphWidth }} className="h-full shrink-0 overflow-hidden">
          <GraphLanes row={row} width={graphWidth} />
        </div>
        <div className="flex min-w-0 flex-1 items-center gap-2 px-2">
          <RefChips refs={row.refs} lane={row.lane} oid={row.oid} ctx={ctx} actions={actions} />
          <span className="truncate">{row.summary}</span>
        </div>
        <div className="hidden w-44 min-w-0 items-center gap-2 px-2 text-fg-muted @2xl:flex">
          <Avatar name={row.authorName} email={row.authorEmail} size={18} remote={showAvatars} />
          <span className="truncate">{row.authorName}</span>
        </div>
        <div
          title={formatDate(row.time)}
          className="hidden w-28 truncate px-2 text-fg-muted @xl:block"
        >
          {relativeTime(row.time)}
        </div>
        <div className="hidden w-20 px-2 font-mono text-xs text-fg-faint @3xl:block">
          {row.oid.slice(0, 7)}
        </div>
      </div>
    </ContextMenu>
  );
});

function Placeholder({ graphWidth }: { graphWidth: number }) {
  return (
    <div className="flex h-full items-center" aria-hidden>
      <div style={{ width: graphWidth }} className="shrink-0" />
      <div className="mx-2 h-2.5 w-1/3 rounded-sm bg-raised" />
    </div>
  );
}
