/**
 * The history toolbar as the Android engine lays it out.
 *
 * These are the parts of parity finding 19 that jsdom can assert the structure
 * of but not the rendering of: a `<select>` the engine gives zero height, a
 * control pushed off a 412px-wide screen, a filter that narrows nothing. jsdom
 * has no layout and would pass on all three while the screen was unusable.
 */
import {
  afterAll,
  beforeEach,
  describe,
  expect,
  test,
} from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems } from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import {
  controlledMenuReached,
  sameSortedItemIds,
  sortedItemIds,
  withCleanupPreservingPrimary,
} from "../src/harness/history-controls.js";
import { itemRows, rowBoxes } from "../src/harness/list.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import {
  ROW_SELECTION,
  SEARCH,
  SEARCH_DEFAULT_LABEL,
  HISTORY_SEARCH_EXPANDED_ATTRIBUTE,
  clearField,
  closeHistorySearch,
  count,
  gotoView,
  interactableControlSurfaceBox,
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
const KIND_FILTER = 'button[aria-label^="Filter by kind,"]';

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
  await closeKindMenu().catch(() => undefined);
  await closeHistorySearch(app).catch(() => undefined);
  await clearField(app, SEARCH).catch(() => undefined);
  await cleanUpItems(app, seeded);
  await app?.detach();
});

beforeEach(async () => {
  await closeKindMenu();
  await closeHistorySearch(app);
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

async function waitForKindMenu(expanded: boolean): Promise<void> {
  const state = expanded ? "open" : "closed";
  await waitFor(
    async () => {
      const snapshot = await app.withPage((page) =>
        page.evaluate((selector) => {
          const trigger = document.querySelector(selector);
          const menuId = trigger?.getAttribute("aria-controls");
          const menu = menuId ? document.getElementById(menuId) : null;
          return {
            ariaExpanded: trigger?.getAttribute("aria-expanded") ?? null,
            triggerState: trigger?.getAttribute("data-state") ?? null,
            menuPresent: menu !== null,
            menuRole: menu?.getAttribute("role") ?? null,
            menuState: menu?.getAttribute("data-state") ?? null,
          };
        }, KIND_FILTER),
      );
      return controlledMenuReached(snapshot, expanded);
    },
    `kind filter did not reach its exact ${state} state`,
  );
}

async function closeKindMenu(): Promise<void> {
  if ((await count(app, `${KIND_FILTER}[aria-expanded="true"]`)) > 0) {
    await app.withPage((page) => page.keyboard.press("Escape"));
  }
  await waitForKindMenu(false);
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
    try {
      const searchSurface = await interactableControlSurfaceBox(app, SEARCH);
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
      expect(searchSurface?.width ?? 0).toBeGreaterThanOrEqual(
        state.toolbarWidth - 1,
      );
    } finally {
      await closeHistorySearch(app);
    }
  });

  test("every control meets the touch target the tokens promise", async () => {
    // `--tap-min`, 44px. A pointer-sized control is the failure this catches
    // on a phone and cannot catch on a desktop engine.
    for (const control of await controlBoxes()) {
      expect(control.height, control.label).toBeGreaterThanOrEqual(44);
    }
  });

  test("filtering by kind removes the rows that do not match", async () => {
    const beforeIds = sortedItemIds(
      itemRows(await rowBoxes(app)).map((row) => row.id),
    );
    expect(beforeIds.length).toBeGreaterThanOrEqual(4);

    await tapElement(app, KIND_FILTER);
    await waitForKindMenu(true);
    await withCleanupPreservingPrimary(
      async () => {
        await tapElement(app, '[role="menuitemcheckbox"]', "Links");
        await waitFor(async () => {
          const rows = itemRows(await rowBoxes(app));
          return (
            rows.length > 0 &&
            rows.every((row) => row.text.includes("https://example.com"))
          );
        }, "the kind filter left rows that are not links on screen");

        // MultiSelect prevents item selection from closing its Radix portal.
        // Restore All from that live menu; the trigger is occluded until Escape.
        await waitForKindMenu(true);
        await tapElement(app, '[role="menuitemcheckbox"]', "All kinds");
        await waitFor(async () => {
          const restoredIds = itemRows(await rowBoxes(app)).map(
            (row) => row.id,
          );
          return (
            sameSortedItemIds(beforeIds, restoredIds) &&
            (await count(
              app,
              'button[aria-label="Filter by kind, default: All kinds"]',
            )) === 1
          );
        }, "clearing the kind filter never restored the exact list");
        await waitForKindMenu(true);
      },
      closeKindMenu,
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
