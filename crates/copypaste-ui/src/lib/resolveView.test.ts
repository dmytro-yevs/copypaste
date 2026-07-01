import { describe, expect, it } from "vitest";
import { resolveView } from "./resolveView";

describe("resolveView — defensive view narrowing (task 6.3)", () => {
  it("passes every known production view id through unchanged", () => {
    for (const id of ["history", "devices", "settings"] as const) {
      expect(resolveView(id)).toBe(id);
    }
  });

  it('resolves the dev-only "gallery" value to "history"', () => {
    expect(resolveView("gallery")).toBe("history");
  });

  it("resolves an arbitrary/unknown string to \"history\"", () => {
    expect(resolveView("bogus")).toBe("history");
    expect(resolveView("")).toBe("history");
    expect(resolveView("History")).toBe("history"); // case-sensitive, not fuzzy-matched
  });
});
