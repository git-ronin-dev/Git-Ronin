import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ipc } from "../../lib/ipc";
import { config } from "../../test/fixtures";
import { keys } from "../workspace/queries";
import { KeyboardSettings } from "./KeyboardSettings";

vi.mock("../../lib/ipc", () => ({
  ipc: { configGet: vi.fn(), configSetKeybindings: vi.fn() },
}));

/** The shortcut overrides last saved. */
const saved = () => vi.mocked(ipc.configSetKeybindings).mock.lastCall?.[0];

function renderEditor(keybindings: Record<string, string> = {}) {
  const client = new QueryClient();
  client.setQueryData(keys.config, {
    ...config,
    portable: { ...config.portable, keybindings },
  });
  render(
    <QueryClientProvider client={client}>
      <KeyboardSettings />
    </QueryClientProvider>,
  );
}

describe("KeyboardSettings", () => {
  beforeEach(() => {
    vi.mocked(ipc.configSetKeybindings).mockResolvedValue();
  });

  it("records a shortcut and takes it from the command that had it", async () => {
    const user = userEvent.setup();
    renderEditor();
    await user.click(screen.getByRole("button", { name: "Change shortcut for Fetch all remotes" }));
    expect(screen.getByRole("status")).toHaveTextContent(/press a shortcut/i);

    // A bare letter isn't accepted.
    fireEvent.keyDown(window, { code: "KeyF", key: "f" });
    expect(screen.getByRole("status")).toHaveTextContent(/add ctrl/i);

    fireEvent.keyDown(window, { code: "KeyZ", key: "z", ctrlKey: true });
    await vi.waitFor(() => expect(saved()).toEqual({ "repo.fetchAll": "Mod+Z", "repo.undo": "" }));
  });

  it("resets an override and removes a shortcut", async () => {
    const user = userEvent.setup();
    renderEditor({ "repo.fetchAll": "Mod+Alt+F" });
    await user.click(screen.getByRole("button", { name: "Reset shortcut for Fetch all remotes" }));
    await vi.waitFor(() => expect(saved()).toEqual({}));
    await user.click(screen.getByRole("button", { name: "Remove shortcut for Search commits" }));
    await vi.waitFor(() => expect(saved()).toEqual({ "graph.search": "" }));
  });
});
