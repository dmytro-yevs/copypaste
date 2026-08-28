import { describe, expect, it } from "vitest";

import { settings } from "./settings";

describe("screenshot preference copy", () => {
  it("keeps pairing outside the screenshot opt-in", () => {
    expect(settings.list.allowScreenshots.description).toContain("shell and Quick Paste");
    expect(settings.list.allowScreenshots.description).toContain("Pairing prompts stay protected");
    expect(settings.list.allowScreenshots.warning).toContain("Pairing prompts stay protected");
  });
});
