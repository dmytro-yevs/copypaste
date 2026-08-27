import { afterEach, describe, expect, test, vi } from "vitest";

import type { AndroidApp } from "../src/harness/app.js";

vi.mock("../src/harness/adb.js", () => ({
  sleep: async () => undefined,
}));

import {
  allPrimaryNavigationButtonsReady,
  anyElementRendered,
  fieldValue,
  firstInteractableElementIndex,
  HISTORY_SEARCH_EXPANDED_ATTRIBUTE,
  NAV,
  NAVIGATION_READY,
  SEARCH,
} from "../src/harness/ui.js";

afterEach(() => {
  delete (globalThis as { document?: Document }).document;
});

function appBackedBySearchInputs(): AndroidApp {
  const hidden = {
    value: "hidden value",
    getBoundingClientRect: () => ({
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      right: 0,
    }),
    matches: () => false,
    getAttribute: () => null,
    contains: () => false,
  };
  const rendered = {
    value: "rendered value",
    getBoundingClientRect: () => ({
      x: 20,
      y: 10,
      width: 272,
      height: 44,
      right: 292,
    }),
    matches: () => false,
    getAttribute: () => null,
    contains: (node: unknown) => node === rendered,
  };
  (globalThis as { document?: unknown }).document = {
    querySelectorAll: () => [hidden, rendered],
    elementFromPoint: () => rendered,
  };
  const page = {
    evaluate: async <T, Args extends unknown[]>(
      fn: (...args: Args) => T,
      ...args: Args
    ) => fn(...args),
  };
  return {
    withPage: async (action: (current: unknown) => unknown) => action(page),
  } as unknown as AndroidApp;
}

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
  test("selects the visible overlay beside the hidden inline field", () => {
    expect(
      firstInteractableElementIndex([
        {
          width: 0,
          height: 0,
          disabled: false,
          ariaDisabled: null,
          ownsCenter: false,
        },
        {
          width: 272,
          height: 44,
          disabled: false,
          ariaDisabled: null,
          ownsCenter: true,
        },
      ]),
    ).toBe(1);
  });

  test("reads the rendered input when the hidden inline input matches first", async () => {
    await expect(fieldValue(appBackedBySearchInputs(), SEARCH)).resolves.toBe(
      "rendered value",
    );
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

  test("reads the authored expanded-state attribute", () => {
    expect(HISTORY_SEARCH_EXPANDED_ATTRIBUTE).toBe("data-search-expanded");
  });
});
