import { clsx } from "clsx";
import { ChevronRight } from "lucide-react";
import { useState, type ComponentProps, type ReactNode } from "react";

import { ContextMenu, type MenuItem } from "../../ui/ContextMenu";

const indent = (depth: number) => ({ paddingLeft: 8 + depth * 14 });

export function Folder({
  name,
  depth,
  icon: folderIcon,
  title,
  menu,
  defaultOpen = true,
  children,
}: {
  name: string;
  depth: number;
  icon: ReactNode;
  title?: string;
  menu?: MenuItem[];
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const button = (
    <button
      type="button"
      aria-expanded={open}
      title={title}
      onClick={() => setOpen(!open)}
      style={indent(depth - 1)}
      className="flex h-7 w-full items-center gap-1.5 pr-3 text-fg-muted hover:bg-hover hover:text-fg"
    >
      <ChevronRight
        className={clsx("size-3.5 shrink-0 transition-transform", open && "rotate-90")}
      />
      {folderIcon}
      <span className="truncate">{name}</span>
    </button>
  );
  return (
    <div>
      {menu ? <ContextMenu items={menu}>{button}</ContextMenu> : button}
      {open && children}
    </div>
  );
}

/** Extra props (and ref) are passed through so it can be a context menu trigger. */
export function Leaf({
  depth,
  title,
  onClick,
  dimmed,
  highlighted,
  trailing,
  children,
  ...rest
}: {
  depth: number;
  title?: string;
  onClick: () => void;
  dimmed?: boolean;
  /** A dragged branch is over it. */
  highlighted?: boolean;
  trailing?: ReactNode;
  children: ReactNode;
} & Omit<ComponentProps<"div">, "onClick" | "title" | "children">) {
  return (
    <div
      {...rest}
      role="button"
      tabIndex={0}
      title={title}
      onClick={onClick}
      onKeyDown={(e) => e.key === "Enter" && onClick()}
      style={indent(depth)}
      className={clsx(
        "group flex h-7 cursor-default items-center gap-2 pr-3 hover:bg-hover",
        dimmed && "opacity-45",
        highlighted && "bg-accent/20 outline outline-1 -outline-offset-1 outline-accent",
      )}
    >
      {children}
      <span className="ml-auto" />
      {trailing}
    </div>
  );
}
