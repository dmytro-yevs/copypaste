import { describe, expect, test, vi } from "vitest";

vi.mock("../src/harness/adb.js", () => ({
  sleep: async () => undefined,
}));

import {
  allPrimaryNavigationButtonsReady,
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
