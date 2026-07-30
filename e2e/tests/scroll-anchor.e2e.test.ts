import { afterAll, beforeAll, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { rowBoxes, scrollTo, scroller, waitForRows } from "../src/harness/ui.js";

const COUNT = 150;

let app: App;

beforeAll(async () => {
  const seed = Array.from({ length: COUNT }, (_, i) => `anchor item ${i}`);
  app = await startApp({ seed });
  await waitForRows(app.browser, 2);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

/** The row occupying the top of the viewport, by list-space offset. */
async function topRow(app: App) {
  const { scrollTop } = await scroller(app.browser);
  const rows = (await rowBoxes(app.browser)).sort((a, b) => a.start - b.start);
  const row = rows.find((candidate) => candidate.start + candidate.height > scrollTop);
  if (!row) throw new Error("no row covers the current scroll offset");
  return { row, scrollTop, intra: scrollTop - row.start };
}

test("a prepend keeps the row under the viewport top where it was (INV-1)", async () => {
  const { browser } = app;

  await scrollTo(browser, 2_400);
  await browser.waitUntil(
    async () => (await scroller(browser)).scrollTop > 2_000,
    { timeout: 10_000, timeoutMsg: "the list did not scroll" },
  );

  const before = await topRow(app);
  expect(before.row.text).toContain("anchor item");

  await app.daemon.add("a brand new clipping that arrives while scrolled");

  await browser.waitUntil(
    async () => {
      const rows = await rowBoxes(browser);
      const total = (await scroller(browser)).totalSize;
      return rows.length > 0 && total > COUNT * 84 - 1;
    },
    { timeout: 30_000, interval: 500, timeoutMsg: "the poll never picked up the new item" },
  );
  await browser.waitUntil(
    async () => (await scroller(browser)).totalSize >= (COUNT + 1) * 84 - 1,
    { timeout: 30_000, interval: 500, timeoutMsg: "the list height never grew" },
  );

  const after = await topRow(app);
  expect(after.row.id).toBe(before.row.id);
  // The anchored row keeps its position under the viewport top, so the whole
  // list must have shifted down by exactly one row's reservation.
  expect(Math.abs(after.intra - before.intra)).toBeLessThan(4);
  expect(after.scrollTop).toBeGreaterThan(before.scrollTop);
});

test("scroll offset is never left past the end when the list shrinks (INV-6)", async () => {
  const { browser } = app;

  const bottom = await scroller(browser);
  await scrollTo(browser, bottom.scrollHeight);
  await browser.waitUntil(
    async () => (await scroller(browser)).scrollTop > bottom.scrollHeight / 2,
    { timeout: 10_000, timeoutMsg: "the list did not scroll to the bottom" },
  );

  const items = await app.daemon.items();
  for (const item of items.slice(0, Math.floor(items.length * 0.7))) {
    await app.daemon.remove(item.id);
  }

  await browser.waitUntil(
    async () => (await scroller(browser)).totalSize < bottom.totalSize / 2,
    { timeout: 40_000, interval: 500, timeoutMsg: "the list never shrank" },
  );

  const after = await scroller(browser);
  expect(after.scrollTop).toBeLessThanOrEqual(
    Math.max(0, after.scrollHeight - after.clientHeight) + 1,
  );

  // A clamp that only fixed the DOM would leave the virtualiser rendering the
  // rows it thinks are on screen, so the window must still cover the viewport.
  const rows = await rowBoxes(browser);
  expect(rows.length).toBeGreaterThan(0);
  const covered = rows.some(
    (row) =>
      row.start <= after.scrollTop && row.start + row.height > after.scrollTop,
  );
  expect(covered).toBe(true);
});
