import { clsx } from "clsx";
import { useId, type ReactNode } from "react";

import { brushEnsoPath, ensoPath } from "./enso";

/** A brush ensō that draws itself in once (instantly with reduced motion). */
export function BrushEnso({ className }: { className?: string }) {
  const mask = useId();
  return (
    <svg viewBox="0 0 64 64" className={className} aria-hidden>
      <mask id={mask} maskUnits="userSpaceOnUse" x={0} y={0} width={64} height={64}>
        {/* Longer than the arc (about 134), so the dash covers all of it. */}
        <path
          d={ensoPath(32, 32, 24, 40)}
          stroke="white"
          strokeWidth={12}
          fill="none"
          strokeDasharray="150 150"
          className="animate-rn-draw"
        />
      </mask>
      <path d={brushEnsoPath(32, 32, 24, 6)} fill="currentColor" mask={`url(#${mask})`} />
    </svg>
  );
}

/** An empty panel: an ensō, what is (not) there and what to do about it. */
export function EmptyNote({
  title,
  children,
  className,
}: {
  title: string;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <div className={clsx("flex flex-col items-center gap-2 px-6 py-8 text-center", className)}>
      <BrushEnso className="mb-1 size-16 text-fg-faint opacity-50" />
      <p className="font-medium text-fg-muted">{title}</p>
      {children && <div className="max-w-72 text-xs text-fg-faint">{children}</div>}
    </div>
  );
}
