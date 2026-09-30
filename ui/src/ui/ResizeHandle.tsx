import { clsx } from "clsx";
import { Separator } from "react-resizable-panels";

/** Hairline splitter with a wider invisible grab area. `horizontal` splits a column. */
export function ResizeHandle({ horizontal = false }: { horizontal?: boolean }) {
  return (
    <Separator
      className={clsx(
        "relative bg-line transition-colors after:absolute hover:bg-accent data-[separator=active]:bg-accent",
        horizontal
          ? "h-px after:inset-x-0 after:-top-1 after:h-2"
          : "w-px after:inset-y-0 after:-left-1 after:w-2",
      )}
    />
  );
}
