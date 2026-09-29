import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useRepoStore } from "../features/repo/store";
import { ipc } from "../lib/ipc";
import { TooltipProvider } from "../ui/Tooltip";
import { AppShell } from "./AppShell";

vi.mock("../lib/ipc", () => ({ ipc: { openRepo: vi.fn(), gitVersion: vi.fn() } }));

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
    useRepoStore.setState({ repo: null });
  });

  it("invites the user to open a repository", async () => {
    renderShell();
    expect(screen.getByRole("button", { name: /open repository…/i })).toBeInTheDocument();
    expect(await screen.findByText("git 2.55.0")).toBeInTheDocument();
  });

  it("shows the open repository and its branch", () => {
    useRepoStore.setState({
      repo: {
        path: "/work/ronin",
        name: "ronin",
        isBare: false,
        head: { kind: "branch", name: "feature/x", unborn: false },
      },
    });
    renderShell();
    expect(screen.getByRole("navigation", { name: "Repository" })).toHaveTextContent("feature/x");
    expect(screen.getByText("/work/ronin")).toBeInTheDocument();
  });
});
