import { describe, expect, it } from "vitest";

import { treeRows } from "./fileTree";

const paths = [
  "src/app/App.tsx",
  "src/app/main.tsx",
  "README.md",
  "docs/guide/intro.md",
  "src/lib.ts",
];
const show = (collapsed: string[] = []) =>
  treeRows(paths, (p) => p, new Set(collapsed)).map((r) =>
    r.kind === "dir"
      ? `${"  ".repeat(r.depth)}${r.name}/ (${r.items.length})`
      : `${"  ".repeat(r.depth)}${r.name}`,
  );

describe("treeRows", () => {
  it("nests folders first, joins single-folder chains and sorts by name", () => {
    expect(show()).toEqual([
      "docs/guide/ (1)",
      "  intro.md",
      "src/ (3)",
      "  app/ (2)",
      "    App.tsx",
      "    main.tsx",
      "  lib.ts",
      "README.md",
    ]);
  });

  it("hides what is inside collapsed folders", () => {
    expect(show(["src/app"])).toEqual([
      "docs/guide/ (1)",
      "  intro.md",
      "src/ (3)",
      "  app/ (2)",
      "  lib.ts",
      "README.md",
    ]);
    expect(show(["src"])).toEqual(["docs/guide/ (1)", "  intro.md", "src/ (3)", "README.md"]);
  });
});
