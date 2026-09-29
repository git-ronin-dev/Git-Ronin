import { clsx } from "clsx";
import { ChevronRight } from "lucide-react";
import { useState, type ReactNode } from "react";

interface SectionProps {
  title: string;
  icon?: ReactNode;
  count?: number;
  defaultOpen?: boolean;
  children?: ReactNode;
}

/** Collapsible sidebar group, e.g. "Local", "Remote", "Tags". */
export function Section({ title, icon, count, defaultOpen = true, children }: SectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="flex h-7 w-full items-center gap-1.5 px-2 text-xs font-semibold tracking-wide text-fg-muted uppercase hover:text-fg"
      >
        <ChevronRight className={clsx("size-3.5 transition-transform", open && "rotate-90")} />
        {icon}
        <span>{title}</span>
        {count !== undefined && <span className="ml-auto font-normal text-fg-faint">{count}</span>}
      </button>
      {open && children && <div className="pb-2">{children}</div>}
    </section>
  );
}
