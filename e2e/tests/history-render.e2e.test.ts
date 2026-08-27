import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import {
  ROW,
  focusList,
  rowBoxes,
  scroller,
  waitForRows,
} from "../src/harness/ui.js";

const COUNT = 120;

/** Current text-card geometry, including the virtual row's wrapper chrome.
 *  Duplicated deliberately: a test that imports the metrics cannot catch them
 *  changing. Mounted rows are intrinsic: one line is 77px, while the complete
 *  three-line preview cap is 119px. */
const SHORT_ROW_HEIGHT = 77;
const LONG_ROW_HEIGHT = 119;

const LONG = "long ".repeat(400);

function seed(): string[] {
  const items: string[] = [];
  for (let i = 0; i < COUNT; i += 1) {
    // Alternating lengths: INV-5 exercises intrinsic measurement and the
    // conservative three-line cap across both content shapes.
    items.push(i % 2 === 0 ? `item ${i} short` : `item ${i} ${LONG}`);
  }
  return items;
}

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: seed() });
  await waitForRows(app.browser, 2);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

describe("the virtualiser", () => {
  test("renders a window of rows, not the whole list and not nothing", async () => {
    const rows = await rowBoxes(app.browser);
    expect(rows.length).toBeGreaterThan(3);
    expect(rows.length).toBeLessThan(COUNT);
  });

  test("reserves the full list height so the scrollbar is real", async () => {
    const { totalSize, scrollHeight, clientHeight } = await scroller(app.browser);
    // Unmounted rows retain the conservative three-line estimate; mounted
    // rows replace it with their measured intrinsic height.
    expect(totalSize).toBeGreaterThan(COUNT * SHORT_ROW_HEIGHT);
    expect(totalSize).toBeLessThanOrEqual(COUNT * LONG_ROW_HEIGHT);
    expect(scrollHeight).toBeGreaterThan(clientHeight);
  });

  test("rows have non-zero laid-out size", async () => {
    const rows = await rowBoxes(app.browser);
    for (const row of rows) expect(row.height).toBeGreaterThan(0);
  });
});

describe("row geometry (INV-5)", () => {
  test("measures intrinsic rows within the conservative preview cap", async () => {
    const rows = await rowBoxes(app.browser);
    const heights = new Set(rows.map((row) => Math.round(row.height)));
    expect([...heights].sort((a, b) => a - b)).toEqual([
      SHORT_ROW_HEIGHT,
      LONG_ROW_HEIGHT,
    ]);

    const long = rows.filter((row) => row.text.includes("long long"));
    const short = rows.filter((row) => row.text.includes("short"));
    expect(long.length).toBeGreaterThan(0);
    expect(short.length).toBeGreaterThan(0);
    expect(
      long.every((row) => Math.round(row.height) === LONG_ROW_HEIGHT),
    ).toBe(true);
    expect(
      short.every((row) => Math.round(row.height) === SHORT_ROW_HEIGHT),
    ).toBe(true);
  });

  test("rows never overlap", async () => {
    const rows = (await rowBoxes(app.browser)).sort((a, b) => a.start - b.start);
    for (let i = 1; i < rows.length; i += 1) {
      const previous = rows[i - 1]!;
      const current = rows[i]!;
      expect(previous.start + previous.height).toBeLessThanOrEqual(current.start + 0.5);
    }
  });

  test("a long clip is clipped to its reserved box rather than expanding it", async () => {
    const overflow = (await app.browser.execute(function (selector: string) {
      return Array.prototype.map.call(
        document.querySelectorAll(selector),
        function (node) {
          const el = node as HTMLElement;
          const box = el.firstElementChild as HTMLElement;
          return {
            reserved: Math.round(el.getBoundingClientRect().height),
            drawn: Math.round(box.getBoundingClientRect().height),
            clipped: getComputedStyle(box).overflow,
          };
        },
      );
    }, ROW)) as { reserved: number; drawn: number; clipped: string }[];

    expect(overflow.length).toBeGreaterThan(0);
    for (const row of overflow) {
      expect(row.drawn).toBeLessThanOrEqual(row.reserved);
      expect(row.clipped).toContain("hidden");
    }
  });
});

describe("keyboard navigation", () => {
  test("the named list owns focus, scrolling, and valid ARIA state", async () => {
    const { browser } = app;
    await focusList(browser);

    const semantics = (await browser.execute(function (selector: string) {
      const list = document.querySelector(selector) as HTMLElement | null;
      return {
        focused: document.activeElement === list,
        tabIndex: list?.tabIndex,
        overflowY: list ? getComputedStyle(list).overflowY : "",
        multiselectable: list?.getAttribute("aria-multiselectable") ?? null,
      };
    }, '[role="list"][aria-label="Clipboard history"]')) as {
      focused: boolean;
      tabIndex: number;
      overflowY: string;
      multiselectable: string | null;
    };
    expect(semantics).toEqual({
      focused: true,
      tabIndex: 0,
      overflowY: "auto",
      multiselectable: null,
    });

    await browser.keys(["ArrowDown"]);
    await browser.waitUntil(
      async () => (await rowBoxes(browser)).some((row) => row.active),
      { timeout: 10_000, timeoutMsg: "ArrowDown never selected a row" },
    );

    const first = (await rowBoxes(browser)).find((row) => row.active)!;
    await browser.keys(["ArrowDown"]);
    await browser.waitUntil(
      async () => {
        const active = (await rowBoxes(browser)).find((row) => row.active);
        return active !== undefined && active.id !== first.id;
      },
      { timeout: 10_000, timeoutMsg: "the second ArrowDown did not move the selection" },
    );
  });

  test("End selects the last row and keeps it inside the rendered window (INV-7)", async () => {
    const { browser } = app;
    await focusList(browser);
    await browser.keys(["End"]);

    await browser.waitUntil(
      async () => {
        const rows = await rowBoxes(browser);
        const active = rows.find((row) => row.active);
        // Newest first, so the oldest seeded item is the last row.
        return active !== undefined && active.text.includes("item 0 short");
      },
      { timeout: 15_000, timeoutMsg: "End did not select and render the last row" },
    );
  });

  test("Home returns to the top and ArrowUp does not wrap (AT-10)", async () => {
    const { browser } = app;
    await focusList(browser);
    await browser.keys(["Home"]);

    await browser.waitUntil(
      async () => {
        const active = (await rowBoxes(browser)).find((row) => row.active);
        return active !== undefined && active.text.includes(`item ${COUNT - 1} `);
      },
      { timeout: 15_000, timeoutMsg: "Home did not select the first row" },
    );

    const before = (await rowBoxes(browser)).find((row) => row.active)!;
    await browser.keys(["ArrowUp"]);
    const after = (await rowBoxes(browser)).find((row) => row.active)!;
    expect(after.id).toBe(before.id);
  });

  test("Escape clears the selection", async () => {
    const { browser } = app;
    await focusList(browser);
    await browser.keys(["Escape"]);
    await browser.waitUntil(
      async () => !(await rowBoxes(browser)).some((row) => row.active),
      { timeout: 10_000, timeoutMsg: "Escape did not clear the selection" },
    );
  });

  test("Ctrl+F moves focus to the search field", async () => {
    const { browser } = app;
    await focusList(browser);
    await browser.keys(["Control", "f"]);
    await browser.releaseActions();

    await browser.waitUntil(
      async () =>
        (await browser.execute(() => {
          const active = document.activeElement;
          return (
            active?.getAttribute("role") === "searchbox" &&
            active
              .getAttribute("aria-label")
              ?.startsWith("Search clipboard history,") === true
          );
        })) === true,
      { timeout: 10_000, timeoutMsg: "Ctrl+F did not focus the search field" },
    );
  });
});
