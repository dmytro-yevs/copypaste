/**
 * The toolbar, selection mode and the quick-copy hint, in a real WebView.
 *
 * These are the parts of parity finding 19 that jsdom can assert the structure
 * of but not the *rendering* of: a `<select>` that the engine lays out at zero
 * height, a bulk bar that overlaps the list, a hint strip behind a media query.
 * jsdom has no layout, so it would pass on all three while the screen was
 * unusable.
 *
 * Caveat that applies to the whole suite: this is WebKitGTK on Linux. macOS
 * ships WKWebView and Android the system WebView, so green here is not evidence
 * about either shipping platform.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { ROW, count, rowBoxes, waitForRows } from "../src/harness/ui.js";

const SELECTION_TOOLBAR =
  '[role="toolbar"][aria-label="Selection actions"]';
const ROW_SELECTION = `${ROW} [role="checkbox"]`;

let app: App;

beforeAll(async () => {
  app = await startApp({
    seed: [
      "https://example.com/one",
      "https://example.com/two",
      "just some words",
      "more plain words",
    ],
  });
  await waitForRows(app.browser, 4);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

/** Every control the toolbar offers, sized as the engine laid it out. */
async function controlBoxes(browser: App["browser"]) {
  return (await browser.execute(function () {
    const controls = [
      {
        label: "Search clipboard history, default",
        selector:
          '[role="searchbox"][aria-label="Search clipboard history, default"]',
      },
      {
        label: "Filter by kind, default: All kinds",
        selector:
          'button[aria-label="Filter by kind, default: All kinds"]',
      },
      {
        label: "Sort order, default: Newest first",
        selector:
          '[role="combobox"][aria-label="Sort order, default: Newest first"]',
      },
    ];
    return controls.map(function (control) {
      const el = document.querySelector(control.selector) as HTMLElement | null;
      const rect = el?.getBoundingClientRect();
      return {
        label: control.label,
        present: Boolean(el),
        width: rect ? rect.width : 0,
        height: rect ? rect.height : 0,
      };
    });
  })) as Array<{ label: string; present: boolean; width: number; height: number }>;
}

describe("the toolbar", () => {
  test("lays every control out with a real box", async () => {
    for (const control of await controlBoxes(app.browser)) {
      expect(control.present, control.label).toBe(true);
      expect(control.width, control.label).toBeGreaterThan(0);
      // 24px is well under any of them; the assertion is against a control
      // that collapsed, not a claim about the design.
      expect(control.height, control.label).toBeGreaterThan(24);
    }
  });

  test("filtering by kind removes the rows that do not match", async () => {
    const before = await rowBoxes(app.browser);
    expect(before.length).toBe(4);

    const defaultKindFilter = await app.browser.$(
      'button[aria-label="Filter by kind, default: All kinds"]',
    );
    await defaultKindFilter.click();
    const links = await app.browser.$(
      '//*[@role="menuitemcheckbox" and normalize-space(.)="Links"]',
    );
    await links.waitForClickable({ timeout: 5_000 });
    await links.click();

    await app.browser.waitUntil(
      async () =>
        (await rowBoxes(app.browser)).length === 2 &&
        (await app.browser.$(
          'button[aria-label="Filter by kind, active: Links"]',
        )).isExisting(),
      { timeout: 5_000, timeoutMsg: "the kind filter did not narrow the list" },
    );

    // Put it back for the tests after this one.
    await app.browser
      .$('button[aria-label="Filter by kind, active: Links"]')
      .click();
    const allKinds = await app.browser.$(
      '//*[@role="menuitemcheckbox" and normalize-space(.)="All kinds"]',
    );
    await allKinds.waitForClickable({ timeout: 5_000 });
    await allKinds.click();
    await app.browser.waitUntil(
      async () =>
        (await rowBoxes(app.browser)).length === 4 &&
        (await app.browser.$(
          'button[aria-label="Filter by kind, default: All kinds"]',
        )).isExisting(),
    );
  });
});

describe("selection mode", () => {
  test("shows checkboxes and a bulk bar, and neither overlaps the list", async () => {
    const selectionEntry = await app.browser.$(ROW_SELECTION);
    await selectionEntry.moveTo();
    await selectionEntry.click();

    await app.browser.waitUntil(
      async () => (await app.browser.$(SELECTION_TOOLBAR)).isDisplayed(),
      { timeout: 5_000, timeoutMsg: "selection actions never appeared" },
    );

    const layout = (await app.browser.execute(function (rowSelector: string) {
      const bar = document.querySelector(
        '[role="toolbar"][aria-label="Selection actions"]',
      ) as HTMLElement | null;
      const firstRow = document.querySelector(rowSelector) as HTMLElement | null;
      const box = document.querySelector('[role="checkbox"]') as HTMLElement | null;
      return {
        barBottom: bar ? bar.getBoundingClientRect().bottom : NaN,
        rowTop: firstRow ? firstRow.getBoundingClientRect().top : NaN,
        checkbox: box
          ? {
              width: box.getBoundingClientRect().width,
              height: box.getBoundingClientRect().height,
            }
          : null,
      };
    }, ROW)) as {
      barBottom: number;
      rowTop: number;
      checkbox: { width: number; height: number } | null;
    };

    // The bar sits above the list rather than floating over it: a floating bar
    // covers the rows the user is choosing between.
    expect(layout.barBottom).toBeLessThanOrEqual(layout.rowTop + 1);
    expect(layout.checkbox).not.toBeNull();
    expect(layout.checkbox!.width).toBeGreaterThan(8);
  });

  test("leaves selection mode with row selection entries available", async () => {
    await app.browser.$(`${SELECTION_TOOLBAR} [aria-label="Done"]`).click();
    await app.browser.waitUntil(
      async () => !(await app.browser.$(SELECTION_TOOLBAR)).isExisting(),
      { timeout: 5_000, timeoutMsg: "selection mode stayed active after Done" },
    );
    expect(await count(app.browser, ROW_SELECTION)).toBe(4);
  });
});
