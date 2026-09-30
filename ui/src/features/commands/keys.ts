/**
 * Keyboard shortcuts as strings such as `Mod+Shift+Z`: modifiers in the
 * order Mod, Ctrl, Alt, Shift, then a key. `Mod` is Cmd on macOS and Ctrl
 * elsewhere; `Ctrl` is the Control key itself (the same as `Mod` except on
 * macOS). Keys are physical (`KeyZ` is `Z` whatever the layout), so
 * shortcuts don't move when Shift changes the character.
 */

export const IS_MAC =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

const MODIFIERS = ["Mod", "Ctrl", "Alt", "Shift"] as const;

const CODES: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Space: "Space",
  Tab: "Tab",
  Enter: "Enter",
  Escape: "Escape",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
};

/** The key part of a shortcut for a `KeyboardEvent.code`, if it can be bound. */
function keyOf(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^Numpad[0-9]$/.test(code)) return code.slice(6);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  return CODES[code] ?? null;
}

/** The shortcut a key press makes, or null for a lone modifier. */
export function eventToShortcut(
  e: Pick<KeyboardEvent, "code" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">,
  mac = IS_MAC,
): string | null {
  const key = keyOf(e.code);
  if (!key) return null;
  const parts: string[] = [];
  if (mac ? e.metaKey : e.ctrlKey) parts.push("Mod");
  if (mac && e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}

/**
 * Canonical form: modifiers de-duplicated and ordered, `Ctrl` read as `Mod`
 * off macOS. Returns null for something that isn't a shortcut.
 */
export function normalizeShortcut(shortcut: string, mac = IS_MAC): string | null {
  const parts = shortcut.split("+").filter(Boolean);
  const key = parts.pop();
  if (!key || (MODIFIERS as readonly string[]).includes(key)) return null;
  const mods = new Set<string>();
  for (const part of parts) {
    const mod = part === "Cmd" || part === "Meta" ? "Mod" : part;
    if (!(MODIFIERS as readonly string[]).includes(mod)) return null;
    mods.add(!mac && mod === "Ctrl" ? "Mod" : mod);
  }
  return [...MODIFIERS.filter((m) => mods.has(m)), key.length === 1 ? key.toUpperCase() : key].join(
    "+",
  );
}

/** Splits a stored binding ("Mod+Shift+Z Mod+Y") into canonical shortcuts. */
export function parseBinding(binding: string, mac = IS_MAC): string[] {
  return binding
    .split(/\s+/)
    .map((s) => normalizeShortcut(s, mac))
    .filter((s): s is string => s !== null);
}

const MAC_SYMBOLS: Record<string, string> = { Mod: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };
const ARROWS: Record<string, string> = { Up: "↑", Down: "↓", Left: "←", Right: "→" };

/** A shortcut as the platform writes it: `Ctrl+Shift+Z`, or `⇧⌘Z` on macOS. */
export function formatShortcut(shortcut: string, mac = IS_MAC): string {
  const parts = shortcut.split("+");
  const key = parts.pop() ?? "";
  const shownKey = ARROWS[key] ?? key;
  if (mac) {
    // macOS orders modifier symbols ⌃⌥⇧⌘.
    const order = ["Ctrl", "Alt", "Shift", "Mod"];
    const mods = order.filter((m) => parts.includes(m)).map((m) => MAC_SYMBOLS[m]);
    return [...mods, shownKey].join("");
  }
  return [...parts.map((m) => (m === "Mod" ? "Ctrl" : m)), shownKey].join("+");
}
