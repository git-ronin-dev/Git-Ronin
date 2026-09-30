import { describe, expect, it } from "vitest";

import { closingText, issueBranchName } from "./issues";

describe("issue branches", () => {
  it("names branches after the issue", () => {
    expect(issueBranchName({ key: "#12", title: "Fix the login page!" })).toBe(
      "12-fix-the-login-page",
    );
    expect(
      issueBranchName({ key: "PROJ-7", title: "Crash when opening a very very long file name" }),
    ).toBe("PROJ-7-crash-when-opening-a-very-very-long-file");
    expect(issueBranchName({ key: "#3", title: "…" })).toBe("3");
    expect(issueBranchName({ key: "#4", title: "Größe ändern" })).toBe("4-größe-ändern");
  });

  it("closes issues from the description", () => {
    expect(closingText("github", "#12")).toBe("Closes #12");
    expect(closingText("azureDevops", "#12")).toBe("Fixes AB#12");
    expect(closingText("github", "PROJ-7")).toBe("");
  });
});
