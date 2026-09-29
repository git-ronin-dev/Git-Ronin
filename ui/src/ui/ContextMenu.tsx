import * as RadixContextMenu from "@radix-ui/react-context-menu";
import type { ReactElement } from "react";

export type MenuItem = { label: string; onSelect: () => void; disabled?: boolean } | "separator";

/** Right-click menu around `children`. */
export function ContextMenu({ items, children }: { items: MenuItem[]; children: ReactElement }) {
  return (
    <RadixContextMenu.Root>
      <RadixContextMenu.Trigger asChild>{children}</RadixContextMenu.Trigger>
      <RadixContextMenu.Portal>
        <RadixContextMenu.Content className="z-50 min-w-44 rounded-md border border-line bg-raised p-1 shadow-xl">
          {items.map((item, i) =>
            item === "separator" ? (
              <RadixContextMenu.Separator key={i} className="my-1 h-px bg-line" />
            ) : (
              <RadixContextMenu.Item
                key={i}
                disabled={item.disabled}
                onSelect={item.onSelect}
                className="flex h-7 cursor-default items-center rounded-sm px-2 outline-none data-[disabled]:opacity-40 data-[highlighted]:bg-accent data-[highlighted]:text-accent-fg"
              >
                {item.label}
              </RadixContextMenu.Item>
            ),
          )}
        </RadixContextMenu.Content>
      </RadixContextMenu.Portal>
    </RadixContextMenu.Root>
  );
}
