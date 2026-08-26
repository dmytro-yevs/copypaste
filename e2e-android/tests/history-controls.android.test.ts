/**
 * The history toolbar as the Android engine lays it out.
 *
 * These are the parts of parity finding 19 that jsdom can assert the structure
 * of but not the rendering of: a `<select>` the engine gives zero height, a
 * control pushed off a 412px-wide screen, a filter that narrows nothing. jsdom
 * has no layout and would pass on all three while the screen was unusable.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems } from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import { rowBoxes } from "../src/harness/list.js";
import {
  SEARCH,
  clearField,
  count,
  gotoView,
  reloadHistoryWith,
  tapButton,
  waitFor,
  waitForRows,
} from "../src/harness/ui.js";

/** Every control the toolbar offers. Named by what a screen reader reads, so
 *  a control that loses its name fails here too. */
const CONTROLS = [
  "Search clipboard history",
  "Filter by kind",
  "Sort order",
  "Select multiple items",
  "Clear clipboard history",
];

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "Library");

  marker = fixtureMarker("controls");
  seeded = await addItems(app, [
    `https://example.com/one?${marker}`,
    `https://example.com/two?${marker}`,
    `${marker} just some words`,
    `${marker} more plain words`,
  ]);
  await reloadHistoryWith(app, `${marker} more plain words`);
  await waitForRows(app, 4);
}, 300_000);

afterAll(async () => {
  await clearField(app, SEARCH).catch(() => undefined);
  await cleanUpItems(app, seeded);
  await app?.detach();
});

async function controlBoxes() {
  return app.withPage((page) =>
    page.evaluate(
      (labels: string[]) =>
        labels.map((label) => {
          const el = document.querySelector(
            `[aria-label="${label}"]`,
          ) as HTMLElement | null;
          const rect = el?.getBoundingClientRect();
          return {
            label,
            present: Boolean(el),
            width: rect ? rect.width : 0,
            height: rect ? rect.height : 0,
            right: rect ? rect.right : 0,
          };
        }),
      CONTROLS,
    ),
  );
}

/** Use the authored listbox rather than reaching through the control. */
async function chooseKind(value: string): Promise<void> {
  await app.withPage((page) =>
    page.evaluate(async (kind: string) => {
      const trigger = document.querySelector(
        '[aria-label="Filter by kind"]',
      ) as HTMLButtonElement | null;
      trigger?.click();
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      );
      (
        document.querySelector(
          `[role="option"][data-value="${kind}"]`,
        ) as HTMLElement | null
      )?.click();
    }, value),
  );
}

describe("the toolbar", () => {
  test("lays every control out with a real box, inside the screen", async () => {
    const width = await app.withPage((page) =>
      page.evaluate(() => document.documentElement.clientWidth),
    );

    for (const control of await controlBoxes()) {
      expect(control.present, control.label).toBe(true);
      expect(control.width, control.label).toBeGreaterThan(0);
      // 24px is well under any of them; the assertion is against a control
      // that collapsed, not a claim about the design.
      expect(control.height, control.label).toBeGreaterThan(24);
      expect(control.right, control.label).toBeLessThanOrEqual(width + 1);
    }
  });

  test("search replaces the control row at full width", async () => {
    await tapButton(app, "Search clipboard history");
    const state = await app.withPage((page) =>
      page.evaluate((searchSelector) => {
        const toolbar = document.querySelector(
          '[data-slot="history-toolbar"]',
        ) as HTMLElement;
        const search = document.querySelector(searchSelector) as HTMLElement;
        const toolbarRect = toolbar.getBoundingClientRect();
        const searchRect = search.getBoundingClientRect();
        return {
          expanded: toolbar.dataset.searchOpen,
          searchWidth: searchRect.width,
          toolbarWidth: toolbarRect.width,
          filterPresent: Boolean(
            document.querySelector('[aria-label="Filter by kind"]'),
          ),
        };
      }, SEARCH),
    );

    expect(state.expanded).toBe("true");
    expect(state.filterPresent).toBe(false);
    expect(state.searchWidth).toBeGreaterThan(state.toolbarWidth - 80);
    await tapButton(app, "Close search");
  });

  test("every control meets the touch target the tokens promise", async () => {
    // `--tap-min`, 44px. A pointer-sized control is the failure this catches
    // on a phone and cannot catch on a desktop engine.
    for (const control of await controlBoxes()) {
      expect(control.height, control.label).toBeGreaterThanOrEqual(44);
    }
  });

  test("filtering by kind removes the rows that do not match", async () => {
    const before = await rowBoxes(app);
    expect(before.length).toBeGreaterThanOrEqual(4);

    await chooseKind("url");
    await waitFor(async () => {
      const rows = await rowBoxes(app);
      return (
        rows.length > 0 &&
        rows.every((row) => row.text.includes("https://example.com"))
      );
    }, "the kind filter left rows that are not links on screen");

    await chooseKind("all");
    await waitFor(
      async () => (await rowBoxes(app)).length >= before.length,
      "clearing the kind filter never restored the list",
    );
  });
});

describe("selection mode", () => {
  test("leaves the mode again without stranding the checkboxes", async () => {
    await tapButton(app, "Select multiple items");
    await waitFor(
      async () => (await count(app, '[role="checkbox"]')) > 0,
      "no checkboxes appeared",
    );

    await tapButton(app, "Done", {
      within: '[role="region"][aria-label="Selection actions"]',
    });
    await waitFor(
      async () => (await count(app, '[role="checkbox"]')) === 0,
      "the checkboxes stayed after leaving",
    );
  });
});
