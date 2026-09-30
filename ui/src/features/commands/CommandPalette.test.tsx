import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useOverlays } from "../../app/overlays";
import { config } from "../../test/fixtures";
import { keys } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { CommandPalette } from "./CommandPalette";

vi.mock("../../lib/ipc", () => ({ ipc: { configGet: vi.fn() } }));

function renderPalette() {
  const client = new QueryClient();
  client.setQueryData(keys.config, config);
  return render(
    <QueryClientProvider client={client}>
      <CommandPalette />
    </QueryClientProvider>,
  );
}

describe("CommandPalette", () => {
  beforeEach(() => {
    useWorkspace.setState({ tabs: [], active: null });
    useOverlays.setState({ palette: true, settings: null });
  });

  it("filters commands and runs the chosen one", async () => {
    const user = userEvent.setup();
    renderPalette();
    const input = screen.getByRole("combobox", { name: /search commands/i });
    // Repository commands are hidden without a repository.
    expect(screen.queryByText("Fetch all remotes")).not.toBeInTheDocument();
    expect(screen.getByText("Show all commands")).toBeInTheDocument();

    await user.type(input, "keyboard");
    const options = screen.getAllByRole("option");
    expect(options[0]).toHaveTextContent("Keyboard shortcuts");
    await user.keyboard("{Enter}");
    await vi.waitFor(() => expect(useOverlays.getState().settings).toBe("keyboard"));
    expect(useOverlays.getState().palette).toBe(false);
  });

  it("says when nothing matches", async () => {
    const user = userEvent.setup();
    renderPalette();
    await user.type(screen.getByRole("combobox"), "zzzzqq");
    expect(screen.getByText("No matching commands")).toBeInTheDocument();
  });
});
