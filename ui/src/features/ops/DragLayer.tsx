import { GitBranch } from "lucide-react";

import { PointMenu } from "../../ui/DropdownMenu";
import { useRepoInfo } from "../workspace/queries";
import { useGitActions } from "./actions";
import { dropActions, useDrag } from "./drag";

/** The dragged branch following the pointer, and the menu after a drop. Mounted once, in App. */
export function DragLayer() {
  const { repo, dragging, over, dropped } = useDrag();
  return (
    <>
      {dragging && (
        <div
          aria-hidden
          style={{ left: dragging.x + 12, top: dragging.y + 8 }}
          className="pointer-events-none fixed z-50 flex items-center gap-1 rounded-sm border border-accent bg-raised px-1.5 py-0.5 text-xs shadow-lg"
        >
          <GitBranch className="size-3" />
          {dragging.source.name}
          {!over && <span className="text-fg-faint">— drop on a branch or commit</span>}
        </div>
      )}
      {dropped && repo && <DropMenu key={`${dropped.x},${dropped.y}`} repo={repo} />}
    </>
  );
}

function DropMenu({ repo }: { repo: string }) {
  const dropped = useDrag((s) => s.dropped)!;
  const info = useRepoInfo(repo).data;
  const actions = useGitActions(repo);
  const head = info?.head.kind === "branch" ? info.head.name : null;
  const choices = dropActions(dropped.source, dropped.target, head);
  const close = () => useDrag.setState({ dropped: null });
  return (
    <PointMenu
      x={dropped.x}
      y={dropped.y}
      title={`${dropped.source.name} → ${dropped.target.name}`}
      items={
        choices.length > 0
          ? choices.map((c) => ({ label: c.label, onSelect: () => void c.run(actions) }))
          : [{ label: "Nothing to do here", disabled: true, onSelect: () => {} }]
      }
      onClose={close}
    />
  );
}
