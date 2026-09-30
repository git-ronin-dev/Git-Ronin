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
import { EmptyNote } from "../../ui/EmptyNote";
import { countChanges, useWorkingStatus } from "../changes/queries";
import { shortRev, useGitActions, type GitActions } from "../ops/actions";
import { dropProps, useDrag } from "../ops/drag";
import { commitMenu, type RepoContext } from "../ops/menus";
import { useBisect, useRepoContext } from "../ops/queries";
import { useStashActions } from "../stash/actions";
import { keys, useRepoInfo, useSetUiPrefs, useUiPrefs } from "../workspace/queries";
import { WORKING_COPY, updateView, useRepoView, type Search } from "../workspace/view";
import { bisectLabels, type BisectLabel } from "./bisect";
import { rowStash } from "./chips";
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
const MAX_GRAPH_WIDTH = 640;

export function GraphView({ repo }: { repo: string }) {
  const client = useQueryClient();
  const view = useRepoView(repo);
  const prefs = useUiPrefs();
  const setPrefs = useSetUiPrefs();
  const showAvatars = prefs?.showAvatars ?? false;
  // While the column edge is being dragged.
  const [dragWidth, setDragWidth] = useState<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [range, setRange] = useState({ first: 0, last: 60 });
  const rows = useGraphRows(repo, range.first, range.last);
  const info = useRepoInfo(repo).data;
  const ctx = useRepoContext(repo);
  const actions = useGitActions(repo);
  const bisect = useBisect(repo, info?.operation === "bisect").data;
  const marks = useMemo(() => bisectLabels(bisect), [bisect]);
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
  const autoWidth = Math.max(lanes * LANE_WIDTH + 8, MIN_GRAPH_WIDTH);
  const graphWidth = dragWidth ?? (prefs?.graphWidth ? clampWidth(prefs.graphWidth) : autoWidth);

  /** Drags the graph column's right edge; the width is saved on release. */
  const startResize = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || !prefs) return;
    e.preventDefault();
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startWidth = graphWidth;
    let width = startWidth;
    const move = (ev: PointerEvent) => {
      width = clampWidth(startWidth + ev.clientX - startX);
      setDragWidth(width);
    };
    const up = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      handle.removeEventListener("pointercancel", up);
      setDragWidth(null);
      if (width !== startWidth) setPrefs.mutate({ ...prefs, graphWidth: width });
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
    handle.addEventListener("pointercancel", up);
  };
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
        <div style={{ width: graphWidth }} className="relative h-full shrink-0 px-2 leading-7">
          Graph
          <div
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize the graph column"
            title="Drag to resize; double-click to fit the lanes"
            onPointerDown={startResize}
            onDoubleClick={() => prefs && setPrefs.mutate({ ...prefs, graphWidth: 0 })}
            className={clsx(
              "absolute inset-y-0 -right-1 z-10 w-2 cursor-col-resize after:absolute after:inset-y-1 after:left-1 after:w-px hover:after:bg-accent",
              dragWidth !== null && "after:bg-accent",
            )}
          />
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
          <EmptyNote title="No commits yet">
            Stage files in the details panel and write the first commit.
          </EmptyNote>
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
                      mark={marks.get(row.oid)}
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

function clampWidth(width: number): number {
  return Math.round(Math.min(Math.max(width, MIN_GRAPH_WIDTH), MAX_GRAPH_WIDTH));
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
  /** What a running bisect said about the commit. */
  mark: BisectLabel | undefined;
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
  mark,
  dimmed,
  showAvatars,
  onSelect,
  ctx,
  actions,
}: RowProps) {
  const over = useDrag((s) => s.over !== null && s.over === row.oid);
  const stash = rowStash(row);
  const stashActions = useStashActions(ctx.repo);
  return (
    <ContextMenu
      items={[
        ...(stash
          ? [
              { label: "Apply stash", onSelect: () => void stashActions.apply(stash, false) },
              { label: "Pop stash", onSelect: () => void stashActions.apply(stash, true) },
              { label: "Delete stash…", onSelect: () => void stashActions.drop(stash) },
            ]
          : commitMenu(actions, ctx, row.oid)),
        "separator",
        { label: "Copy commit SHA", onSelect: () => void copyText(row.oid, "Copied commit SHA") },
        { label: "Copy message", onSelect: () => void copyText(row.summary, "Copied message") },
      ]}
    >
      <div
        role="row"
        aria-selected={selected}
        onMouseDown={() => onSelect(row.oid)}
        {...dropProps({ kind: "commit", name: shortRev(row.oid), fullName: row.oid, oid: row.oid })}
        className={clsx(
          "flex h-full items-center transition-opacity",
          selected ? "bg-accent/20" : "hover:bg-hover",
          dimmed && "opacity-35",
          over && "bg-accent/20 outline outline-1 -outline-offset-1 outline-accent",
        )}
      >
        <div style={{ width: graphWidth }} className="h-full shrink-0 overflow-hidden">
          <GraphLanes row={row} width={graphWidth} stash={stash !== null} />
        </div>
        <div className="flex min-w-0 flex-1 items-center gap-2 px-2">
          {mark && <BisectChip mark={mark} />}
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

const markStyles: Record<BisectLabel["kind"], string> = {
  bad: "border-danger text-danger",
  good: "border-success text-success",
  skipped: "border-line text-fg-faint",
  found: "border-danger bg-danger text-accent-fg font-semibold",
};

function BisectChip({ mark }: { mark: BisectLabel }) {
  return (
    <span
      title="Marked during the bisect"
      className={clsx(
        "flex h-5 shrink-0 items-center rounded-sm border px-1.5 text-xs",
        markStyles[mark.kind],
      )}
    >
      {mark.label}
    </span>
  );
}

function Placeholder({ graphWidth }: { graphWidth: number }) {
  return (
    <div className="flex h-full items-center" aria-hidden>
      <div style={{ width: graphWidth }} className="shrink-0" />
      <div className="mx-2 h-2.5 w-1/3 rounded-sm bg-raised" />
    </div>
  );
}
