/**
 * The history toolbar as the Android engine lays it out.
 *
 * These are the parts of parity finding 19 that jsdom can assert the structure
 * of but not the rendering of: a `<select>` the engine gives zero height, a
 * control pushed off a 412px-wide screen, a filter that narrows nothing. jsdom
 * has no layout and would pass on all three while the screen was unusable.
 */
import { afterAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems } from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import { rowBoxes } from "../src/harness/list.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import {
  ROW_SELECTION,
  SEARCH,
  SEARCH_DEFAULT_LABEL,
  HISTORY_SEARCH_EXPANDED_ATTRIBUTE,
  clearField,
  count,
  gotoView,
  interactableElementBox,
  openHistorySearch,
  reloadHistoryWith,
  tapButton,
  tapElement,
  waitFor,
  waitForRows,
} from "../src/harness/ui.js";

/** Every control the toolbar offers. Named by what a screen reader reads, so
 *  a control that loses its name fails here too. */
const CONTROLS = [
  SEARCH_DEFAULT_LABEL,
  "Filter by kind, default: All kinds",
  "Sort order, default: Newest first",
];

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

beforeAllWithEvidence("history-controls", async () => {
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
  return Promise.all(
    CONTROLS.map(async (label) => {
      const box = await interactableElementBox(app, `[aria-label="${label}"]`);
      return {
        label,
        present: box !== null,
        width: box?.width ?? 0,
        height: box?.height ?? 0,
        right: box?.right ?? 0,
      };
    }),
  );
}

/** Use the authored listbox rather than reaching through the control. */
async function chooseKind(value: string): Promise<void> {
  await tapElement(app, 'button[aria-label^="Filter by kind,"]');
  await tapElement(
    app,
    '[role="menuitemcheckbox"]',
    value === "url" ? "Links" : "All kinds",
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
    await openHistorySearch(app);
    const search = await interactableElementBox(app, SEARCH);
    const filter = await interactableElementBox(
      app,
      '[aria-label^="Filter by kind,"]',
    );
    const state = await app.withPage((page) =>
      page.evaluate((expandedAttribute) => {
        const toolbar = document.querySelector(
          '[data-slot="history-toolbar"]',
        ) as HTMLElement;
        const siblings = toolbar.querySelector(
          ':scope > [aria-hidden="true"]',
        ) as HTMLElement | null;
        const toolbarRect = toolbar.getBoundingClientRect();
        return {
          expanded: toolbar.hasAttribute(expandedAttribute),
          toolbarWidth: toolbarRect.width,
          filterMounted: Boolean(
            toolbar.querySelector('[aria-label^="Filter by kind,"]'),
          ),
          siblingsHidden: siblings?.getAttribute("aria-hidden"),
          siblingsVisibility: siblings
            ? getComputedStyle(siblings).visibility
            : null,
        };
      }, HISTORY_SEARCH_EXPANDED_ATTRIBUTE),
    );

    expect(state.expanded).toBe(true);
    expect(state.filterMounted).toBe(true);
    expect(state.siblingsHidden).toBe("true");
    expect(state.siblingsVisibility).toBe("hidden");
    expect(filter).toBeNull();
    expect(search?.width ?? 0).toBeGreaterThan(state.toolbarWidth - 80);
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
    const before = await count(app, ROW_SELECTION);
    expect(before).toBeGreaterThan(0);
    await tapElement(app, ROW_SELECTION);
    await waitFor(
      async () =>
        (await count(
          app,
          '[role="toolbar"][aria-label="Selection actions"]',
        )) === 1,
      "selection actions never appeared",
    );

    await tapButton(app, "Done", {
      within: '[role="toolbar"][aria-label="Selection actions"]',
    });
    await waitFor(
      async () =>
        (await count(
          app,
          '[role="toolbar"][aria-label="Selection actions"]',
        )) === 0,
      "selection mode stayed active after Done",
    );
    expect(await count(app, ROW_SELECTION)).toBe(before);
  });
});
