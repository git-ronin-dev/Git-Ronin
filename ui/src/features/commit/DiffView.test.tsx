import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { FileDiff } from "../../bindings/FileDiff";
import { DiffView } from "./DiffView";

const diff: FileDiff = {
  binary: false,
  hunks: [
    {
      header: "@@ -1,2 +1,3 @@",
      oldStart: 1,
      oldLines: 2,
      newStart: 1,
      newLines: 3,
      lines: [
        { kind: "context", oldLine: 1, newLine: 1, text: "keep\r", noNewline: false },
        { kind: "added", oldLine: null, newLine: 2, text: "first", noNewline: false },
        { kind: "added", oldLine: null, newLine: 3, text: "second", noNewline: false },
      ],
    },
  ],
};

function renderDiff(onApply = vi.fn()) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <DiffView
        diff={diff}
        mode="unified"
        path="notes.unknown-language"
        staging={{ actions: [{ verb: "Stage", target: "stage" }], onApply, busy: false }}
      />
    </QueryClientProvider>,
  );
  return onApply;
}

describe("DiffView staging", () => {
  it("stages a whole hunk when no line is selected", () => {
    const onApply = renderDiff();
    fireEvent.click(screen.getByRole("button", { name: "Stage hunk" }));
    expect(onApply).toHaveBeenCalledWith("stage", [{ hunk: 0, lines: [0, 1, 2] }]);
  });

  it("stages the lines picked by clicking them", () => {
    const onApply = renderDiff();
    fireEvent.click(screen.getByText("second"));
    // Context lines can't be picked.
    fireEvent.click(screen.getByText("keep"));
    fireEvent.click(screen.getByRole("button", { name: "Stage 1 line" }));
    expect(onApply).toHaveBeenCalledWith("stage", [{ hunk: 0, lines: [2] }]);
  });
});
