import { clsx } from "clsx";
import { Separator } from "react-resizable-panels";

/**
 * Hairline splitter with a wider invisible grab area, drawn as a katana edge
 * (a glint along the line). `horizontal` splits a column.
 */
export function ResizeHandle({ horizontal = false }: { horizontal?: boolean }) {
  return (
    <Separator
      className={clsx(
        "relative after:absolute hover:bg-accent hover:bg-none data-[separator=active]:bg-accent data-[separator=active]:bg-none",
        horizontal
          ? "rn-edge-x h-px after:inset-x-0 after:-top-1 after:h-2"
          : "rn-edge-y w-px after:inset-y-0 after:-left-1 after:w-2",
      )}
    />
  );
}
