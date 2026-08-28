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
import { listSnapshot, rowBoxes, settledList } from "../src/harness/list.js";
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

/**
 * Every height `rowHeight(1..6)` can return, from
 * `crates/copypaste-ui/src/lib/layout.ts` and duplicated deliberately: a test
 * that imported the function could not catch it changing.
 *
 * The whole table rather than one value, because the device keeps its
 * preferences between runs and `previewLines` is a preference. What INV-5
 * claims is that every row reserves the SAME height whatever it holds, not
 * which setting the device is on; `settings.android` pins the mapping.
 *
 * A band rather than an equality, because `HistoryList` gives an Android row
 * `minHeight` where desktop gets `height`. The virtualiser measures the row
 * back, so a one-line row settles at 68 against a 67px reservation, and the
 * property that survives is "at least this, and short of the next setting's".
 */
const RESERVATIONS = [67, 88, 109, 130, 151, 172];
const TITLE_LINE_PX = 21;

function reservationFor(height: number): number | undefined {
  return RESERVATIONS.find((base) => height >= base && height < base + TITLE_LINE_PX);
}

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

beforeAllWithEvidence("history-render", async () => {
  app = await attachToApp();
  await gotoView(app, "Library");
  marker = fixtureMarker("render");

  // Alternating lengths: INV-5 says the reservation is a function of the
  // setting, so both must reserve identically.
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
    async () => (await listSnapshot(app)).totalSize > COUNT * RESERVATIONS[0]!,
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

  /**
   * Retried rather than read once, and to within a row or two rather than
   * exactly. The screen learns what the store holds from a 3s poll, and past
   * `PAGE_SIZE` it holds several accumulated pages — a row deleted from a page
   * already fetched leaves the two counts differing until the next refetch.
   * The claim is that the spacer reserves the whole list rather than the
   * rendered window, which a two-row tolerance cannot hide: the window here is
   * an order of magnitude smaller than the list.
   */
  test("reserves the full list height so the scrollbar is real", async () => {
    let seen = "";
    await waitFor(
      async () => {
        const snapshot = await listSnapshot(app);
        const reserved = Math.round(snapshot.rows[0]?.height ?? 0);
        if (reservationFor(reserved) === undefined) return false;
        const reservedRows = snapshot.totalSize / reserved;
        seen = `${snapshot.totalSize}px is ${reservedRows} rows of ${reserved}px, expected ${COUNT}`;
        return (
          Math.abs(reservedRows - COUNT) <= 2 &&
          reservedRows > snapshot.rows.length * 2 &&
          snapshot.scrollHeight > snapshot.clientHeight
        );
      },
      () => `the spacer never matched the store: ${seen}`,
      30_000,
    );
  });

  test("rows have non-zero laid-out size", async () => {
    for (const row of await rowBoxes(app)) expect(row.height).toBeGreaterThan(0);
  });
});

describe("row geometry (INV-5)", () => {
  /**
   * One query per fixture length, because the marker-wide search this suite
   * isolates through ranks by relevance (manifest 06 §3.1.7): a 2000-character
   * clip scores below every short one, so the list opens on 30 short rows and a
   * long row is never on screen to compare against.
   *
   * `short` and `long` each occur in exactly one of the two fixtures, so either
   * query selects its own half on the fuzzy and the FTS path alike.
   */
  test("over-reserves: a 2000-character clip reserves what a 5-word clip does", async () => {
    await filterHistoryTo(app, `${marker} short`, marker);
    const short = await settledList(
      app,
      (list) => list.rows.length > 0 && list.rows.every((row) => row.text.includes("short")),
      { timeout: 30_000, describe: "the short-only search never came to rest on short rows" },
    );

    await filterHistoryTo(app, `${marker} long`, marker);
    const long = await settledList(
      app,
      (list) => list.rows.length > 0 && list.rows.every((row) => row.text.includes("long long")),
      { timeout: 30_000, describe: "the long-only search never came to rest on long rows" },
    );

    expect(short.rows.length).toBeGreaterThan(0);
    expect(long.rows.length).toBeGreaterThan(0);

    const heights = [
      ...new Set([...short.rows, ...long.rows].map((row) => Math.round(row.height))),
    ];
    expect(heights).toHaveLength(1);
    expect(reservationFor(heights[0]!)).toBeDefined();
  }, 120_000);

  test("rows never overlap", async () => {
    const rows = (await rowBoxes(app)).sort((a, b) => a.start - b.start);
    for (let i = 1; i < rows.length; i += 1) {
      const previous = rows[i - 1]!;
      expect(previous.start + previous.height).toBeLessThanOrEqual(rows[i]!.start + 0.5);
    }
  });

  test("a long clip is clipped to its reserved box rather than expanding it", async () => {
    const overflow = await app.withPage((page) =>
      page.evaluate(
        (selector: string) =>
          Array.from(document.querySelectorAll(selector), (node) => {
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
