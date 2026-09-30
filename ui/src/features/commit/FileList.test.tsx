import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { FileChange } from "../../bindings/FileChange";
import { config } from "../../test/fixtures";
import { keys } from "../workspace/queries";
import { FileList } from "./FileList";

vi.mock("../../lib/ipc", () => ({ ipc: { configGet: vi.fn() } }));

const file = (path: string): FileChange => ({
  path,
  oldPath: null,
  status: "modified",
  additions: 1,
  deletions: 0,
});

function renderList(fileTree: boolean, onOpen = vi.fn()) {
  const client = new QueryClient();
  client.setQueryData(keys.config, {
    ...config,
    portable: { ...config.portable, ui: { ...config.portable.ui, fileTree } },
  });
  render(
    <QueryClientProvider client={client}>
      <FileList
        files={[file("src/app/App.tsx"), file("src/lib.ts"), file("README.md")]}
        activePath={null}
        onOpen={onOpen}
      />
    </QueryClientProvider>,
  );
  return onOpen;
}

describe("FileList", () => {
  it("shows full paths as a flat list", () => {
    renderList(false);
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual([
      "Msrc/app/App.tsx+1",
      "Msrc/lib.ts+1",
      "MREADME.md+1",
    ]);
  });

  it("nests files in folders that collapse", async () => {
    const user = userEvent.setup();
    const onOpen = renderList(true);
    expect(screen.getByRole("tree")).toBeInTheDocument();
    const src = screen.getByRole("treeitem", { name: /^src/ });
    expect(src).toHaveAttribute("aria-expanded", "true");
    await user.click(screen.getByRole("treeitem", { name: /App\.tsx/ }));
    expect(onOpen).toHaveBeenCalledWith(file("src/app/App.tsx"));

    await user.click(src);
    expect(src).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("treeitem", { name: /lib\.ts/ })).not.toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /README/ })).toBeInTheDocument();
  });
});
