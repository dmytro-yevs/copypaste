/**
 * The virtualiser and INV-5, on the engine Android ships.
 *
 * This is the most engine-dependent behaviour in the product: a reservation
 * that is a function of the setting rather than of the content, a spacer that
 * makes the scrollbar real, and rows that clip instead of growing. jsdom has no
 * box model and WebKitGTK is not this engine, so neither of the other two
 * layers is evidence for it here.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, deleteItems, storedItems } from "../src/harness/bridge.js";
import { listSnapshot, rowBoxes } from "../src/harness/list.js";
import {
  ROW,
  SEARCH,
  clearField,
  gotoView,
  scrollListToTop,
  waitFor,
  waitForRows,
} from "../src/harness/ui.js";

const COUNT = 120;
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

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "History");
  await clearField(app, SEARCH);

  // Alternating lengths: INV-5 says the reservation is a function of the
  // setting, so both must reserve identically.
  seeded = await addItems(
    app,
    Array.from({ length: COUNT }, (_, i) =>
      i % 2 === 0 ? `render item ${i} short` : `render item ${i} ${LONG}`,
    ),
  );

  await waitForRows(app, 4);
  await scrollListToTop(app);
  await waitFor(
    async () => (await listSnapshot(app)).totalSize > COUNT * RESERVATIONS[0]!,
    "the list never reserved room for the seeded items",
    60_000,
  );
}, 300_000);

afterAll(async () => {
  // The device is not reset between runs. Leaving 120 rows behind would make
  // the next run's list a different shape, and the run after that a different
  // one again.
  await deleteItems(app, seeded).catch(() => undefined);
  await app?.detach();
});

describe("the virtualiser", () => {
  test("renders a window of rows, not the whole list and not nothing", async () => {
    const rows = await rowBoxes(app);
    expect(rows.length).toBeGreaterThan(3);
    expect(rows.length).toBeLessThan(COUNT);
  });

  test("reserves the full list height so the scrollbar is real", async () => {
    // Retried rather than read once: the screen learns what the store holds
    // from a 3s poll, so the spacer and a fresh `list` describe the same list
    // only between polls. The property is the equality, not the first sample.
    let seen = "";
    await waitFor(
      async () => {
        const snapshot = await listSnapshot(app);
        const stored = await storedItems(app);
        const reserved = Math.round(snapshot.rows[0]?.height ?? 0);
        seen = `${snapshot.totalSize}px for ${stored.length} rows of ${reserved}px`;
        return (
          reservationFor(reserved) !== undefined &&
          Math.abs(snapshot.totalSize - stored.length * reserved) < 1 &&
          snapshot.scrollHeight > snapshot.clientHeight
        );
      },
      () => `the spacer never matched the store: ${seen}`,
      20_000,
    );
  });

  test("rows have non-zero laid-out size", async () => {
    for (const row of await rowBoxes(app)) expect(row.height).toBeGreaterThan(0);
  });
});

describe("row geometry (INV-5)", () => {
  test("over-reserves: a 2000-character clip reserves what a 5-word clip does", async () => {
    const rows = await rowBoxes(app);
    const heights = [...new Set(rows.map((row) => Math.round(row.height)))];
    expect(heights).toHaveLength(1);
    expect(reservationFor(heights[0]!)).toBeDefined();

    const long = rows.filter((row) => row.text.includes("long long"));
    const short = rows.filter((row) => row.text.includes("short"));
    expect(long.length).toBeGreaterThan(0);
    expect(short.length).toBeGreaterThan(0);
  });

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
