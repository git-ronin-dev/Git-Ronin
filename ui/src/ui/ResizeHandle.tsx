import { Separator } from "react-resizable-panels";

/** Hairline splitter with a wider invisible grab area. */
export function ResizeHandle() {
  return (
    <Separator className="relative w-px bg-line transition-colors after:absolute after:inset-y-0 after:-left-1 after:w-2 hover:bg-accent data-[separator=active]:bg-accent" />
  );
}
