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
import { afterAll, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import {
  addItems,
  cleanUpItems,
  deleteItems,
  storedItems,
} from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import {
  maxScrollTop,
  renderedRowsCoverViewport,
  scrollTo,
  settledList,
  type ListSnapshot,
} from "../src/harness/list.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import {
  gotoView,
  filterHistoryTo,
  reloadHistoryWith,
  resetHistoryFilters,
  scrollListToTop,
  waitForText,
  waitForRows,
} from "../src/harness/ui.js";

const COUNT = 150;

/** The smallest reservation `rowHeight` can return, from
 *  `crates/copypaste-ui/src/lib/layout.ts`. Used only as a floor for "the list
 *  is taller than the viewport", never as the expected height. */
const MIN_ROW = 67;

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

/**
 * Fixed width, because this file isolates its rows through the product search
 * path and that path ranks by relevance rather than recency (manifest 06
 * §3.1.7). One character of label is enough to reorder the list: an unpadded
 * `item 150` scores below all 150 shorter labels and is appended at the END,
 * which grows the spacer under the viewport and prepends nothing.
 *
 * Equal length puts every candidate on the same score, and the tie-break is the
 * caller's order — newest first — so the arrival lands at the top.
 */
function label(index: number): string {
  return `${marker} item ${String(index).padStart(3, "0")}`;
}

/** The row occupying the top of the viewport, by list-space offset. */
function topRow(snapshot: ListSnapshot) {
  const rows = [...snapshot.rows].sort((a, b) => a.start - b.start);
  const row = rows.find(
    (candidate) => candidate.start + candidate.height > snapshot.scrollTop,
  );
  if (!row) throw new Error("no row covers the current scroll offset");
  return {
    row,
    scrollTop: snapshot.scrollTop,
    intra: snapshot.scrollTop - row.start,
  };
}

beforeAllWithEvidence("scroll-anchor", async () => {
  app = await attachToApp();
  await gotoView(app, "Library");
  marker = fixtureMarker("anchor");
  seeded = await addItems(
    app,
    Array.from({ length: COUNT }, (_, i) => label(i)),
  );
  await reloadHistoryWith(app, label(COUNT - 1));
  await filterHistoryTo(app, marker, marker);
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
  await resetHistoryFilters(app).catch(() => undefined);
  await cleanUpItems(app, seeded);
  await app?.detach();
});

test("a prepend keeps the row under the viewport top where it was (INV-1)", async () => {
  await scrollTo(app, 2_400);
  const resting = await settledList(
    app,
    (list) => list.scrollTop > 2_000 && list.rows.length > 0,
    {
      timeout: 15_000,
      describe: "the list did not come to rest scrolled past 2000px",
    },
  );
  const before = topRow(resting);
  expect(before.row.text).toContain(marker);

  const grown = before.row.height;
  // The same template as the seed, one index past its end, so the arrival is a
  // prepend in the searched list and not merely a taller spacer below it.
  const [added] = await addItems(app, [label(COUNT)]);
  seeded.push(added!);

  const after = topRow(
    await settledList(
      app,
      (list) =>
        list.totalSize >= resting.totalSize + grown - 1 && list.rows.length > 0,
      {
        timeout: 30_000,
        describe:
          "the poll never picked up the new item, or the list height never grew",
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
    {
      timeout: 15_000,
      describe: "the list did not scroll to the bottom",
    },
  );

  // Only rows this file seeded. The run's own fixtures live in the same store,
  // the leak assertions are written against them, and the device keeps
  // whatever earlier runs left — so how much of the *store* this is cannot be
  // assumed, and the shrink is measured in rows removed rather than in halves.
  const doomed = seeded.slice(0, Math.floor(seeded.length * 0.7));
  await deleteItems(app, doomed);
  const remaining = new Set((await storedItems(app)).map((item) => item.id));
  expect(doomed.every((id) => !remaining.has(id))).toBe(true);
  expect(
    seeded
      .filter((id) => !doomed.includes(id))
      .every((id) => remaining.has(id)),
  ).toBe(true);
  const survivingSeeded = seeded.filter((id) => !doomed.includes(id));
  await waitForText(app, `${survivingSeeded.length} items`, 60_000);

  // Waits for the shrink to land and the virtualiser to stop moving — never for
  // the invariant below, which is asserted once against that resting state.
  const after = await settledList(
    app,
    (list) => list.totalSize < resting.totalSize,
    {
      timeout: 60_000,
      describe:
        `list height never shrank after deleting ${doomed.length} of ` +
        `${seeded.length} suite items and rendering ${survivingSeeded.length} results`,
    },
  );
  seeded = survivingSeeded;

  expect(after.scrollTop).toBeCloseTo(maxScrollTop(after), 5);

  // A clamp that only fixed the DOM would leave the virtualiser rendering the
  // rows it thinks are on screen, so the window must still cover the viewport.
  expect(after.rows.length).toBeGreaterThan(0);
  expect(
    renderedRowsCoverViewport(after),
    `rendered rows do not cover virtual content ${after.scrollTop}..${Math.min(
      after.scrollTop + after.clientHeight,
      after.totalSize,
    )}`,
  ).toBe(true);
}, 180_000);
