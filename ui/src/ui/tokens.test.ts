import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/** `--rn-*` colours of the dark (`:root`) and light blocks of tokens.css. */
function themes() {
  // Read from disk: vitest leaves CSS imports (even `?raw`) empty.
  const css = readFileSync(join(import.meta.dirname, "tokens.css"), "utf8");
  const blocks = [...css.matchAll(/^(:root[^{]*)\{([^}]*)\}/gm)];
  const read = (body: string) =>
    Object.fromEntries(
      [...body.matchAll(/--rn-([\w-]+):\s*(#[0-9a-f]{6})\s*;/gi)].map((m) => [m[1]!, m[2]!]),
    );
  const dark = read(blocks.find((b) => b[1]!.trim() === ":root")![2]!);
  const light = { ...dark, ...read(blocks.find((b) => b[1]!.includes("light"))![2]!) };
  return { dark, light };
}

function channels(hex: string) {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
}

function luminance(hex: string) {
  const [r, g, b] = channels(hex).map((c) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
}

function contrast(a: string, b: string) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
}

/** `fg` at `alpha` over `bg`, like Tailwind's `bg-success/12`. */
function over(fg: string, alpha: number, bg: string) {
  const f = channels(fg);
  const b = channels(bg);
  return `#${f
    .map((c, i) =>
      Math.round(c * alpha + b[i]! * (1 - alpha))
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}

const TEXT = ["fg", "fg-muted", "fg-faint", "accent", "gold", "danger", "success", "warning"];
const BACKGROUNDS = ["canvas", "surface", "raised"];
const SYNTAX = [
  "keyword",
  "string",
  "number",
  "comment",
  "type",
  "definition",
  "property",
  "macro",
  "punctuation",
];

describe.each(Object.entries(themes()))("%s theme", (_, t) => {
  it("keeps text at WCAG AA (4.5:1) on every background", () => {
    for (const fg of TEXT)
      for (const bg of BACKGROUNDS)
        expect(contrast(t[fg]!, t[bg]!), `${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
    for (const fg of ["fg", "fg-muted", "fg-faint"])
      expect(contrast(t[fg]!, t.hover!), `${fg} on hover`).toBeGreaterThanOrEqual(4.5);
  });

  it("keeps button labels readable", () => {
    expect(contrast(t["accent-fg"]!, t.accent!)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(t["accent-fg"]!, t.danger!)).toBeGreaterThanOrEqual(4.5);
  });

  it("keeps syntax colours readable on plain, added and removed lines", () => {
    const lines = [
      t.surface!,
      over(t.success!, 0.12, t.surface!),
      over(t.danger!, 0.12, t.surface!),
    ];
    for (const name of SYNTAX)
      for (const bg of lines)
        expect(contrast(t[`syn-${name}`]!, bg), `${name} on ${bg}`).toBeGreaterThanOrEqual(4.5);
  });

  it("keeps graph lanes visible (3:1 for graphics)", () => {
    for (let i = 0; i < 8; i++)
      expect(contrast(t[`lane-${i}`]!, t.canvas!), `lane ${i}`).toBeGreaterThanOrEqual(3);
  });
});
