import { describe, expect, it } from "vitest";

import {
  eventToShortcut,
  formatShortcut,
  normalizeShortcut,
  parseBinding,
  reachesAppFromTerminal,
} from "./keys";

const press = (code: string, mods: Partial<KeyboardEvent> = {}) => ({
  code,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("shortcuts", () => {
  it("reads physical keys and platform modifiers", () => {
    expect(eventToShortcut(press("KeyZ", { ctrlKey: true, shiftKey: true }), false)).toBe(
      "Mod+Shift+Z",
    );
    expect(eventToShortcut(press("KeyZ", { metaKey: true }), true)).toBe("Mod+Z");
    expect(eventToShortcut(press("KeyZ", { metaKey: true }), false)).toBe("Z");
    expect(eventToShortcut(press("Tab", { ctrlKey: true }), true)).toBe("Ctrl+Tab");
    expect(eventToShortcut(press("Backquote", { ctrlKey: true }), false)).toBe("Mod+`");
    expect(eventToShortcut(press("ShiftLeft", { shiftKey: true }), false)).toBeNull();
  });

  it("normalizes stored shortcuts", () => {
    expect(normalizeShortcut("Shift+Ctrl+z", false)).toBe("Mod+Shift+Z");
    expect(normalizeShortcut("Ctrl+Tab", true)).toBe("Ctrl+Tab");
    expect(normalizeShortcut("Cmd+P", true)).toBe("Mod+P");
    expect(normalizeShortcut("Shift", false)).toBeNull();
    expect(normalizeShortcut("Hyper+X", false)).toBeNull();
    expect(parseBinding(" Mod+Shift+Z  Mod+Y ", false)).toEqual(["Mod+Shift+Z", "Mod+Y"]);
    expect(parseBinding("", false)).toEqual([]);
  });

  it("formats for the platform", () => {
    expect(formatShortcut("Mod+Shift+Z", false)).toBe("Ctrl+Shift+Z");
    expect(formatShortcut("Mod+Shift+Z", true)).toBe("⇧⌘Z");
    expect(formatShortcut("Alt+Up", false)).toBe("Alt+↑");
  });

  it("leaves shell keys to the terminal", () => {
    expect(reachesAppFromTerminal("Mod+W", false)).toBe(false);
    expect(reachesAppFromTerminal("Mod+R", false)).toBe(false);
    expect(reachesAppFromTerminal("Mod+Shift+P", false)).toBe(true);
    expect(reachesAppFromTerminal("Mod+`", false)).toBe(true);
    expect(reachesAppFromTerminal("Mod+W", true)).toBe(true);
    expect(reachesAppFromTerminal("Ctrl+R", true)).toBe(false);
  });
});
