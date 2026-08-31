import { describe, expect, it } from "vitest";

import { clipTypeMetadata, fileDisplayName } from "@/lib/clipPresentation";
import { clipCopyAction, historyKindFilterLabel } from "./clipPresentation";

describe("History clip presentation", () => {
  it("keeps singular item labels distinct from filter labels", () => {
    expect(clipTypeMetadata("image").label).toBe("Image");
    expect(historyKindFilterLabel("image")).toBe("Images");
    expect(clipTypeMetadata("file").label).toBe("File");
    expect(historyKindFilterLabel("file")).toBe("Files");
  });

  it("uses one copy action for every History surface", () => {
    expect(clipCopyAction("image")).toEqual({
      icon: "image",
      label: "Copy image",
    });
    expect(clipCopyAction("text")).toEqual({
      icon: "copy",
      label: "Copy",
    });
  });

  it.each([
    ["/Users/alice/Documents/report.pdf", "report.pdf"],
    [String.raw`C:\Users\alice\Documents\report.pdf`, "report.pdf"],
    ["folder/report.pdf/", "report.pdf"],
    ["", ""],
  ])("formats the display name for %j", (value, expected) => {
    expect(fileDisplayName(value)).toBe(expected);
  });
});
