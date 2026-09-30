import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { openUrl } from "../../lib/open";
import MarkdownView from "./MarkdownView";

vi.mock("../../lib/open", () => ({ openUrl: vi.fn() }));

describe("MarkdownView", () => {
  it("renders GitHub-flavoured Markdown", () => {
    const { container } = render(
      <MarkdownView text={"## Plan\n\n- [x] parser\n- [ ] **docs**\n\n`code`"} />,
    );
    expect(screen.getByRole("heading", { name: "Plan" })).toBeInTheDocument();
    expect(screen.getAllByRole("checkbox")).toHaveLength(2);
    expect(container.querySelector("strong")).toHaveTextContent("docs");
    expect(container.querySelector("code")).toHaveTextContent("code");
  });

  it("opens links in the browser, resolving relative ones against the page", () => {
    render(
      <MarkdownView
        text={"[site](https://example.com) and [guide](docs/guide.md)"}
        base="https://github.com/o/r/pull/1"
      />,
    );
    fireEvent.click(screen.getByRole("link", { name: "site" }));
    expect(openUrl).toHaveBeenCalledWith("https://example.com");
    fireEvent.click(screen.getByRole("link", { name: "guide" }));
    expect(openUrl).toHaveBeenLastCalledWith("https://github.com/o/r/pull/docs/guide.md");
  });

  it("never runs raw HTML or javascript: links", () => {
    const { container } = render(
      <MarkdownView text={'<img src=x onerror="alert(1)"> [x](javascript:alert(1))'} />,
    );
    expect(container.querySelector("img")).toBeNull();
    fireEvent.click(screen.getByText("x"));
    expect(openUrl).not.toHaveBeenCalledWith(expect.stringContaining("javascript"));
  });
});
