import { describe, expect, it } from "vitest";

import {
  QUICK_PASTE_PREVIEW_LINES,
  previewLineCount,
} from "./previewDensity";

describe("preview density", () => {
  it("keeps history fixed at three lines", () => {
    expect(previewLineCount(1)).toBe(3);
    expect(previewLineCount(3)).toBe(3);
  });

  it("makes Quick Paste a one-line action list", () => {
    expect(previewLineCount(3, "quickPaste")).toBe(QUICK_PASTE_PREVIEW_LINES);
  });
});
