import { clsx } from "clsx";
import { useState, type ReactNode } from "react";

import { ChevronRight } from "./icons";

interface SectionProps {
  title: string;
  icon?: ReactNode;
  count?: number;
  defaultOpen?: boolean;
  /** A small button shown in the header, e.g. "add". */
  action?: ReactNode;
  children?: ReactNode;
}

/** Collapsible sidebar group, e.g. "Local", "Remote", "Tags". */
export function Section({
  title,
  icon,
  count,
  defaultOpen = true,
  action,
  children,
}: SectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section>
      <div className="group flex h-7 items-center pr-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="flex h-full min-w-0 flex-1 items-center gap-1.5 px-2 text-xs font-semibold tracking-wide text-fg-muted uppercase hover:text-fg"
        >
          <ChevronRight className={clsx("size-3.5 transition-transform", open && "rotate-90")} />
          {icon}
          <span>{title}</span>
          {count !== undefined && (
            <span className="ml-auto font-normal text-fg-faint">{count}</span>
          )}
        </button>
        {action}
      </div>
      {open && children && <div className="pb-2">{children}</div>}
    </section>
  );
}
