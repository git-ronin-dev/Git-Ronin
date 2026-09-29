import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useWorkspace } from "../features/workspace/store";
import { ipc } from "../lib/ipc";
import { config, refs, repoInfo, row } from "../test/fixtures";
import { TooltipProvider } from "../ui/Tooltip";
import { AppShell } from "./AppShell";

vi.mock("../lib/ipc", () => ({
  ipc: {
    gitVersion: vi.fn(),
    configGet: vi.fn(),
    repoInfo: vi.fn(),
    listRefs: vi.fn(),
    graphPage: vi.fn(),
    graphSearch: vi.fn(),
  },
}));

function renderShell() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <TooltipProvider>
        <AppShell />
      </TooltipProvider>
    </QueryClientProvider>,
  );
}

describe("AppShell", () => {
  beforeEach(() => {
    vi.mocked(ipc.gitVersion).mockResolvedValue({ major: 2, minor: 55, patch: 0 });
    vi.mocked(ipc.configGet).mockResolvedValue(config);
    vi.mocked(ipc.repoInfo).mockResolvedValue(repoInfo);
    vi.mocked(ipc.listRefs).mockResolvedValue(refs);
    vi.mocked(ipc.graphPage).mockResolvedValue({
      start: 0,
      rows: [row("a", "Add parser"), row("b", "Initial commit")],
      loaded: 2,
      complete: true,
      maxLanes: 1,
    });
    useWorkspace.setState({ tabs: [], active: null });
  });

  it("offers recent repositories when nothing is open", async () => {
    renderShell();
    expect(screen.getByRole("button", { name: /open repository…/i })).toBeInTheDocument();
    expect(await screen.findByText("/work/ronin")).toBeInTheDocument();
    expect(await screen.findByText("git 2.55.0")).toBeInTheDocument();
  });

  it("shows tabs, refs and the commit graph of the open repository", async () => {
    useWorkspace.setState({
      tabs: [{ path: "/work/ronin", name: "ronin" }],
      active: "/work/ronin",
    });
    renderShell();

    expect(screen.getByRole("tab", { name: /ronin/ })).toHaveAttribute("aria-selected", "true");
    const sidebar = await screen.findByRole("navigation", { name: "Repository" });
    // Local main and origin/main.
    expect(within(sidebar).getAllByText("main")).toHaveLength(2);
    expect(within(sidebar).getByLabelText("current branch")).toBeInTheDocument();
    // Branch folders: feature/login is grouped under "feature".
    expect(within(sidebar).getByText("feature")).toBeInTheDocument();
    expect(within(sidebar).getByText("login")).toBeInTheDocument();
    expect(within(sidebar).getByText("↑2")).toBeInTheDocument();
    expect(await screen.findByText("/work/ronin")).toBeInTheDocument();
  });
});
