import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { config, refs } from "../../test/fixtures";
import { keys } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import {
  activeProfileId,
  COMMANDS,
  conflictsWith,
  dynamicCommands,
  effectiveKeys,
  shortcutMap,
} from "./commands";

vi.mock("../../lib/ipc", () => ({ ipc: {} }));

describe("commands", () => {
  beforeEach(() => {
    useWorkspace.setState({
      tabs: [
        { path: "/work/ronin", name: "ronin" },
        { path: "/work/other", name: "other" },
      ],
      active: "/work/ronin",
    });
  });

  it("have unique ids", () => {
    const ids = COMMANDS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("take the user's shortcuts over the defaults", () => {
    const defaults = effectiveKeys({}, false);
    expect(defaults.get("repo.redo")).toEqual(["Mod+Shift+Z", "Mod+Y"]);
    expect(defaults.get("tab.next")).toEqual(["Mod+Tab"]);

    const custom = effectiveKeys({ "repo.redo": "Mod+Alt+Z", "graph.search": "" }, false);
    expect(custom.get("repo.redo")).toEqual(["Mod+Alt+Z"]);
    expect(custom.get("graph.search")).toEqual([]);

    const map = shortcutMap(custom);
    expect(map.get("Mod+P")?.id).toBe("palette.open");
    expect(map.get("Mod+Alt+Z")?.id).toBe("repo.redo");
    expect(map.has("Mod+F")).toBe(false);
  });

  it("find other commands on the same shortcut", () => {
    const bindings = effectiveKeys({}, false);
    expect(conflictsWith(bindings, "repo.fetchAll", "Mod+Z").map((c) => c.id)).toEqual([
      "repo.undo",
    ]);
    expect(conflictsWith(bindings, "repo.undo", "Mod+Z")).toEqual([]);
  });

  it("offer tabs, recent repositories, profiles and branches", () => {
    const client = new QueryClient();
    client.setQueryData(keys.config, {
      ...config,
      portable: {
        ...config.portable,
        profiles: [
          ...config.portable.profiles,
          { ...config.portable.profiles[0]!, id: "w", name: "Work" },
        ],
      },
      local: { ...config.local, recentRepos: ["/work/ronin", "/old"] },
    });
    client.setQueryData(keys.refs("/work/ronin"), refs);
    const titles = dynamicCommands({ client, repo: "/work/ronin" }).map((c) => c.title);
    expect(titles).toEqual([
      "Switch to other",
      "Open recent /old",
      "Switch to profile Work",
      "Check out feature/login",
    ]);
    expect(activeProfileId(client.getQueryData(keys.config))).toBe("default");
  });
});
