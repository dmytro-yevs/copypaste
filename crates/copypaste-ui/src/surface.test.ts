import { describe, expect, it } from "vitest";

import { isQuickPasteSurface } from "@/surface";

describe("window surface routing", () => {
  it("mounts the compact surface only for the quick-paste route", () => {
    expect(isQuickPasteSurface("?surface=quick-paste")).toBe(true);
    expect(isQuickPasteSurface("")).toBe(false);
    expect(isQuickPasteSurface("?surface=main")).toBe(false);
  });
});
