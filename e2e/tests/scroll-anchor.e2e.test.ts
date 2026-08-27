import { afterAll, beforeAll, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import {
  scrollTo,
  scroller,
  settledList,
  waitForRows,
  type ListSnapshot,
} from "../src/harness/ui.js";

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
function topRow(snapshot: ListSnapshot) {
  const rows = [...snapshot.rows].sort((a, b) => a.start - b.start);
  const row = rows.find(
    (candidate) => candidate.start + candidate.height > snapshot.scrollTop,
  );
  if (!row) throw new Error("no row covers the current scroll offset");
  return { row, scrollTop: snapshot.scrollTop, intra: snapshot.scrollTop - row.start };
}

test("a prepend keeps the row under the viewport top where it was (INV-1)", async () => {
  const { browser } = app;

  await scrollTo(browser, 2_400);
  const beforeSnapshot = await settledList(
    browser,
    (list) => list.scrollTop > 2_000 && list.rows.length > 0,
    {
      timeout: 10_000,
      describe: "the list did not come to rest scrolled past 2000px",
    },
  );
  const before = topRow(beforeSnapshot);
  expect(before.row.text).toContain("anchor item");

  await app.daemon.add("a brand new clipping that arrives while scrolled");

  const after = topRow(
    await settledList(
      browser,
      (list) =>
        list.totalSize > beforeSnapshot.totalSize && list.rows.length > 0,
      {
        timeout: 30_000,
        describe: "the poll never picked up the new item, or the list height never grew",
      },
    ),
  );
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
  const doomed = items.slice(0, Math.floor(items.length * 0.7));
  await app.daemon.removeMany(doomed.map((item) => item.id));

  const remaining = await app.daemon.items();
  expect(remaining.length).toBeLessThan(items.length / 2);

  // Waits for the shrink to land and the virtualiser to stop moving — never for
  // the invariant below, which is asserted once against that resting state.
  const after = await settledList(
    browser,
    (list) => list.totalSize < bottom.totalSize / 2,
    {
      timeout: 45_000,
      describe:
        `list height never came to rest below ${bottom.totalSize / 2}px after ` +
        `deleting ${doomed.length} of ${items.length} items (daemon now has ` +
        `${remaining.length})`,
    },
  );

  expect(after.scrollTop).toBeLessThanOrEqual(
    Math.max(0, after.scrollHeight - after.clientHeight) + 1,
  );

  // A clamp that only fixed the DOM would leave the virtualiser rendering the
  // rows it thinks are on screen, so the window must still cover the viewport.
  expect(after.rows.length).toBeGreaterThan(0);
  const covered = after.rows.some(
    (row) =>
      row.start <= after.scrollTop && row.start + row.height > after.scrollTop,
  );
  expect(covered, `no rendered row covers scrollTop ${after.scrollTop}`).toBe(true);
}, 120_000);
