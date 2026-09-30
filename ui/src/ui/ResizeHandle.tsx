import { clsx } from "clsx";
import { Separator } from "react-resizable-panels";

/**
 * The gap between two floating panels, which drags to resize them. A thin
 * vermilion line shows in it while hovered or dragged. `horizontal` splits
 * a column.
 */
export function ResizeHandle({ horizontal = false }: { horizontal?: boolean }) {
  return (
    <Separator
      className={clsx(
        "group relative shrink-0 outline-none after:absolute after:rounded-full after:transition-colors",
        "hover:after:bg-accent data-[separator=active]:after:bg-accent",
        horizontal
          ? "h-1.5 after:inset-x-2 after:top-1/2 after:h-0.5 after:-translate-y-1/2"
          : "w-1.5 after:inset-y-2 after:left-1/2 after:w-0.5 after:-translate-x-1/2",
      )}
    />
  );
}
