import { describe, expect, test, vi } from "vitest";

vi.mock("../src/harness/adb.js", () => ({
  sleep: async () => undefined,
}));

import {
  allPrimaryNavigationButtonsReady,
  anyElementRendered,
  NAV,
  NAVIGATION_READY,
} from "../src/harness/ui.js";

describe("primary navigation readiness", () => {
  test("uses enabled semantic controls rather than a product marker", () => {
    expect(NAVIGATION_READY).toBe(
      `${NAV} button:not(:disabled):not([aria-disabled="true"])`,
    );
    expect(NAVIGATION_READY).not.toContain("data-navigation-ready");
  });

  test("requires every rendered navigation button to be actionable", () => {
    expect(allPrimaryNavigationButtonsReady([])).toBe(false);
    expect(
      allPrimaryNavigationButtonsReady([
        { disabled: false, ariaDisabled: null },
        { disabled: false, ariaDisabled: null },
        { disabled: false, ariaDisabled: null },
      ]),
    ).toBe(true);
    expect(
      allPrimaryNavigationButtonsReady([
        { disabled: false, ariaDisabled: null },
        { disabled: true, ariaDisabled: null },
        { disabled: false, ariaDisabled: null },
      ]),
    ).toBe(false);
    expect(
      allPrimaryNavigationButtonsReady([
        { disabled: false, ariaDisabled: "true" },
        { disabled: false, ariaDisabled: null },
      ]),
    ).toBe(false);
  });
});

describe("compact history search readiness", () => {
  test("accepts the visible overlay beside the hidden inline field", () => {
    expect(
      anyElementRendered([
        { width: 0, height: 0 },
        { width: 272, height: 44 },
      ]),
    ).toBe(true);
  });

  test("does not accept a toolbar whose search fields are all hidden", () => {
    expect(
      anyElementRendered([
        { width: 0, height: 0 },
        { width: 272, height: 0 },
      ]),
    ).toBe(false);
  });

  test("accepts the single inline field at wider toolbar widths", () => {
    expect(anyElementRendered([{ width: 320, height: 44 }])).toBe(true);
  });
});
