/**
 * The virtualiser and INV-5, on the engine Android ships.
 *
 * This is the most engine-dependent behaviour in the product: a reservation
 * that is a function of the setting rather than of the content, a spacer that
 * makes the scrollbar real, and rows that clip instead of growing. jsdom has no
 * box model and WebKitGTK is not this engine, so neither of the other two
 * layers is evidence for it here.
 */
import { afterAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems } from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import {
  itemRows,
  listSnapshot,
  reservesConservativeTextList,
  rowBoxes,
  settledList,
} from "../src/harness/list.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import {
  ROW,
  SEARCH,
  clearField,
  filterHistoryTo,
  gotoView,
  reloadHistoryWith,
  scrollListToTop,
  waitFor,
  waitForRows,
} from "../src/harness/ui.js";

/**
 * Enough rows to virtualise several times over, and no more. The browser layer
 * seeds 120 into an empty store; this store is a device's, it already holds
 * whatever earlier runs left, and pushing the total past `PAGE_SIZE` splits the
 * list into accumulated pages whose count lags the store's after a delete.
 */
const COUNT = 60;
const LONG = "long ".repeat(400);

/** Measured text-card geometry, duplicated so the test catches product drift. */
const TEXT_GEOMETRY = { group: 34, short: 77, long: 119 } as const;

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

beforeAllWithEvidence("history-render", async () => {
  app = await attachToApp();
  await gotoView(app, "Library");
  marker = fixtureMarker("render");

  // Both shapes begin with the conservative preview cap; measurement then
  // replaces short rows with their smaller intrinsic height.
  seeded = await addItems(
    app,
    Array.from({ length: COUNT }, (_, i) =>
      i % 2 === 0 ? `${marker} item ${i} short` : `${marker} item ${i} ${LONG}`,
    ),
  );

  await reloadHistoryWith(app, `${marker} item ${COUNT - 1}`);
  await filterHistoryTo(app, marker, marker);
  await waitForRows(app, 4);
  await scrollListToTop(app);
  await waitFor(
    async () =>
      reservesConservativeTextList(
        await listSnapshot(app),
        COUNT,
        TEXT_GEOMETRY,
      ),
    "the list never reserved room for the seeded items",
    60_000,
  );
}, 300_000);

afterAll(async () => {
  // The device is not reset between runs. Leaving these rows behind would make
  // the next run's list a different shape, and the run after that a different
  // one again.
  await clearField(app, SEARCH).catch(() => undefined);
  await cleanUpItems(app, seeded);
  await app?.detach();
});

describe("the virtualiser", () => {
  test("renders a window of rows, not the whole list and not nothing", async () => {
    const rows = await rowBoxes(app);
    expect(rows.length).toBeGreaterThan(3);
    expect(rows.length).toBeLessThan(COUNT);
  });

  test("reserves the full list height so the scrollbar is real", async () => {
    let seen = "";
    await waitFor(
      async () => {
        const snapshot = await listSnapshot(app);
        const measured = itemRows(snapshot.rows).map((row) =>
          Math.round(row.height),
        );
        seen = `${snapshot.totalSize}px with mounted item heights ${measured.join(", ")}`;
        return (
          measured.length > 3 &&
          measured.every(
            (height) =>
              height === TEXT_GEOMETRY.short || height === TEXT_GEOMETRY.long,
          ) &&
          reservesConservativeTextList(snapshot, COUNT, TEXT_GEOMETRY)
        );
      },
      () => `the spacer never matched the store: ${seen}`,
      30_000,
    );
  });

  test("rows have non-zero laid-out size", async () => {
    for (const row of await rowBoxes(app))
      expect(row.height).toBeGreaterThan(0);
  });
});

describe("row geometry (INV-5)", () => {
  test("measures intrinsic rows within the conservative preview cap", async () => {
    await filterHistoryTo(app, `${marker} short`, marker);
    const short = await settledList(
      app,
      (list) => {
        const rows = itemRows(list.rows);
        return (
          rows.length > 0 && rows.every((row) => row.text.includes("short"))
        );
      },
      {
        timeout: 30_000,
        describe: "the short-only search never came to rest on short rows",
      },
    );

    await filterHistoryTo(app, `${marker} long`, marker);
    const long = await settledList(
      app,
      (list) => {
        const rows = itemRows(list.rows);
        return (
          rows.length > 0 && rows.every((row) => row.text.includes("long long"))
        );
      },
      {
        timeout: 30_000,
        describe: "the long-only search never came to rest on long rows",
      },
    );

    const shortRows = itemRows(short.rows);
    const longRows = itemRows(long.rows);
    expect(shortRows.length).toBeGreaterThan(0);
    expect(longRows.length).toBeGreaterThan(0);
    expect(
      shortRows.every((row) => Math.round(row.height) === TEXT_GEOMETRY.short),
    ).toBe(true);
    expect(
      longRows.every((row) => Math.round(row.height) === TEXT_GEOMETRY.long),
    ).toBe(true);
  }, 120_000);

  test("rows never overlap", async () => {
    const rows = (await rowBoxes(app)).sort((a, b) => a.start - b.start);
    for (let i = 1; i < rows.length; i += 1) {
      const previous = rows[i - 1]!;
      expect(previous.start + previous.height).toBeLessThanOrEqual(
        rows[i]!.start + 0.5,
      );
    }
  });

  test("a long clip is clipped to its reserved box rather than expanding it", async () => {
    const overflow = await app.withPage((page) =>
      page.evaluate(
        (selector: string) =>
          Array.from(document.querySelectorAll(selector))
            .filter((node) =>
              (node as HTMLElement).id.startsWith("history-row-"),
            )
            .map((node) => {
              const el = node as HTMLElement;
              const box = el.firstElementChild as HTMLElement;
              return {
                reserved: Math.round(el.getBoundingClientRect().height),
                drawn: Math.round(box.getBoundingClientRect().height),
                clipped: getComputedStyle(box).overflow,
              };
            }),
        ROW,
      ),
    );

    expect(overflow.length).toBeGreaterThan(0);
    for (const row of overflow) {
      expect(row.drawn).toBeLessThanOrEqual(row.reserved);
      expect(row.clipped).toContain("hidden");
    }
  });
});

describe("the list's own semantics (INV-8)", () => {
  test("is a named, scrollable list rather than a listbox", async () => {
    const semantics = await app.withPage((page) =>
      page.evaluate((selector: string) => {
        const list = document.querySelector(selector) as HTMLElement | null;
        if (!list) return null;
        return {
          role: list.getAttribute("role"),
          tabIndex: list.tabIndex,
          overflowY: getComputedStyle(list).overflowY,
          multiselectable: list.getAttribute("aria-multiselectable"),
          items: document.querySelectorAll('[role="option"]').length,
        };
      }, '[role="list"][aria-label="Clipboard history"]'),
    );

    expect(semantics).toEqual({
      role: "list",
      tabIndex: 0,
      overflowY: "auto",
      multiselectable: null,
      items: 0,
    });
  });
});
