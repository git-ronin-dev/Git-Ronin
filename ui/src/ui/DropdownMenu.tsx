import * as RadixDropdown from "@radix-ui/react-dropdown-menu";
import type { ReactElement } from "react";

import type { MenuItem } from "./ContextMenu";

const content = "rn-pop z-50 min-w-44 rounded-md border border-line bg-raised p-1 shadow-xl";
const item =
  "flex h-7 cursor-default items-center rounded-sm px-2 outline-none data-[disabled]:opacity-40 data-[highlighted]:bg-hover data-[highlighted]:text-fg data-[highlighted]:shadow-[inset_2px_0_0_var(--rn-accent)]";

function Items({ items }: { items: MenuItem[] }) {
  return items.map((it, i) =>
    it === "separator" ? (
      <RadixDropdown.Separator key={i} className="my-1 h-px bg-line" />
    ) : (
      <RadixDropdown.Item key={i} disabled={it.disabled} onSelect={it.onSelect} className={item}>
        {it.label}
      </RadixDropdown.Item>
    ),
  );
}

/** Menu that opens when `children` (a button) is clicked. */
export function DropdownMenu({
  items,
  children,
  align = "start",
}: {
  items: MenuItem[];
  children: ReactElement;
  align?: "start" | "center" | "end";
}) {
  return (
    <RadixDropdown.Root modal={false}>
      <RadixDropdown.Trigger asChild>{children}</RadixDropdown.Trigger>
      <RadixDropdown.Portal>
        <RadixDropdown.Content align={align} sideOffset={4} className={content}>
          <Items items={items} />
        </RadixDropdown.Content>
      </RadixDropdown.Portal>
    </RadixDropdown.Root>
  );
}

/** Menu at a point on screen, e.g. where something was dropped. */
export function PointMenu({
  x,
  y,
  title,
  items,
  onClose,
}: {
  x: number;
  y: number;
  title?: string;
  items: MenuItem[];
  onClose: () => void;
}) {
  return (
    <RadixDropdown.Root open onOpenChange={(open) => !open && onClose()}>
      <RadixDropdown.Trigger asChild>
        <span aria-hidden style={{ position: "fixed", left: x, top: y, width: 0, height: 0 }} />
      </RadixDropdown.Trigger>
      <RadixDropdown.Portal>
        <RadixDropdown.Content align="start" className={content}>
          {title && (
            <RadixDropdown.Label className="px-2 py-1 text-xs text-fg-faint">
              {title}
            </RadixDropdown.Label>
          )}
          <Items items={items} />
        </RadixDropdown.Content>
      </RadixDropdown.Portal>
    </RadixDropdown.Root>
  );
}
