import { describe, expect, it } from "vitest";

import { quickPasteSearchLabel } from "@/features/quick-paste/screen/QuickPasteScreen";
import { item } from "@/test/harness";

describe("quickPasteSearchLabel", () => {
  it("does not expose unsupported payload text to fuzzy search", () => {
    const unsupported = item({
      content: "https://future.example/raw",
      content_type: "application/x-future",
      content_class: "other",
    });

    expect(quickPasteSearchLabel(unsupported)).toBe("Unsupported clipboard content");
  });
});
