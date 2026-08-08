/**
 * Scroll anchoring on the shipping Android engine: INV-1 when the list grows
 * under the viewport, INV-6 when it shrinks out from under it.
 *
 * Both assertions read the scroll box and the rendered row window in ONE
 * evaluation and wait for that geometry to repeat across frames (6e9d7b7f).
 * Two round trips describe two different states while the list is still
 * moving, and the pair that comes back fails an invariant the app never
 * violated.
 */
import { afterAll, beforeAll, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, deleteItems, storedItems } from "../src/harness/bridge.js";
import { scrollTo, settledList, type ListSnapshot } from "../src/harness/list.js";
import {
  gotoView,
  resetHistoryFilters,
  scrollListToTop,
  waitForRows,
} from "../src/harness/ui.js";

const COUNT = 150;

/** The smallest reservation `rowHeight` can return, from
 *  `crates/copypaste-ui/src/lib/layout.ts`. Used only as a floor for "the list
 *  is taller than the viewport", never as the expected height. */
const MIN_ROW = 67;

/**
 * How far past the end the clamp may leave the offset: one row.
 *
 * The browser layer allows a pixel. Here the rows are measured rather than
 * fixed — `HistoryList` sets `minHeight` on Android — so the spacer, the
 * measured content and the scroll box round against each other and the
 * residue is a few pixels that varies with the shrink (1.5 to 5.5 observed).
 * The failure INV-6 exists for is a viewport left over blank space, which
 * under a row's worth of overshoot cannot produce; the assertion that no
 * blank viewport happened is the `covered` one below, and it is exact.
 */
function overshootSlack(rowHeight: number): number {
  return Math.max(1, rowHeight);
}

let app: AndroidApp;
let seeded: string[] = [];

/** The row occupying the top of the viewport, by list-space offset. */
function topRow(snapshot: ListSnapshot) {
  const rows = [...snapshot.rows].sort((a, b) => a.start - b.start);
  const row = rows.find((candidate) => candidate.start + candidate.height > snapshot.scrollTop);
  if (!row) throw new Error("no row covers the current scroll offset");
  return { row, scrollTop: snapshot.scrollTop, intra: snapshot.scrollTop - row.start };
}

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "History");
  await resetHistoryFilters(app);
  seeded = await addItems(
    app,
    Array.from({ length: COUNT }, (_, i) => `anchor item ${i}`),
  );
  await waitForRows(app, 4);
  await scrollListToTop(app);
  await settledList(app, (list) => list.totalSize > COUNT * MIN_ROW, {
    timeout: 60_000,
    describe: "the list never reserved room for the seeded items",
  });
}, 300_000);

afterAll(async () => {
  // This file is the one that scrolls. Leaving the viewport at the bottom
  // hides the next file's freshly seeded rows behind the virtualiser.
  await scrollListToTop(app).catch(() => undefined);
  await deleteItems(app, seeded).catch(() => undefined);
  await app?.detach();
});

test("a prepend keeps the row under the viewport top where it was (INV-1)", async () => {
  await scrollTo(app, 2_400);
  const resting = await settledList(
    app,
    (list) => list.scrollTop > 2_000 && list.rows.length > 0,
    { timeout: 15_000, describe: "the list did not come to rest scrolled past 2000px" },
  );
  const before = topRow(resting);
  expect(before.row.text).toContain("anchor item");

  const grown = before.row.height;
  const [added] = await addItems(app, [
    `a brand new clipping that arrives while scrolled ${Date.now()}`,
  ]);
  seeded.push(added!);

  const after = topRow(
    await settledList(
      app,
      (list) => list.totalSize >= resting.totalSize + grown - 1 && list.rows.length > 0,
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
}, 120_000);

test("scroll offset is never left past the end when the list shrinks (INV-6)", async () => {
  const bottom = await settledList(app, (list) => list.rows.length > 0, {
    timeout: 15_000,
    describe: "the list never came to rest before scrolling to the bottom",
  });
  await scrollTo(app, bottom.scrollHeight);
  const resting = await settledList(
    app,
    (list) => list.scrollTop > bottom.scrollHeight / 2 && list.rows.length > 0,
    { timeout: 15_000, describe: "the list did not scroll to the bottom" },
  );

  // Only rows this file seeded. The run's own fixtures live in the same store,
  // the leak assertions are written against them, and the device keeps
  // whatever earlier runs left — so how much of the *store* this is cannot be
  // assumed, and the shrink is measured in rows removed rather than in halves.
  const doomed = seeded.slice(0, Math.floor(seeded.length * 0.7));
  const before = await storedItems(app);
  await deleteItems(app, doomed);
  const remaining = await storedItems(app);
  expect(remaining.length).toBe(before.length - doomed.length);

  const row = resting.rows[0]?.height ?? MIN_ROW;
  const shrunkTo = resting.totalSize - doomed.length * row;

  // Waits for the shrink to land and the virtualiser to stop moving — never for
  // the invariant below, which is asserted once against that resting state.
  const after = await settledList(app, (list) => list.totalSize <= shrunkTo + row, {
    timeout: 60_000,
    describe:
      `list height never came to rest at or below ${Math.round(shrunkTo)}px after ` +
      `deleting ${doomed.length} of ${before.length} items (the store now has ` +
      `${remaining.length})`,
  });
  seeded = seeded.filter((id) => !doomed.includes(id));

  expect(after.scrollTop).toBeLessThanOrEqual(
    Math.max(0, after.scrollHeight - after.clientHeight) +
      overshootSlack(after.rows[0]?.height ?? 0),
  );

  // A clamp that only fixed the DOM would leave the virtualiser rendering the
  // rows it thinks are on screen, so the window must still cover the viewport.
  expect(after.rows.length).toBeGreaterThan(0);
  const covered = after.rows.some(
    (row) => row.start <= after.scrollTop && row.start + row.height > after.scrollTop,
  );
  expect(covered, `no rendered row covers scrollTop ${after.scrollTop}`).toBe(true);
}, 180_000);
