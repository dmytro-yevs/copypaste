import { afterAll, beforeAll, expect, inject, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { ordinaryFor } from "../src/harness/fixtures.js";
import {
  HISTORY_LIST,
  ROW,
  SEARCH,
  clearField,
  count,
  gotoView,
  rowCount,
  openHistorySearch,
  resetHistoryFilters,
  typeInto,
  visibleText,
  waitFor,
  waitForRows,
} from "../src/harness/ui.js";

const nonce = inject("nonce");
const ordinary = ordinaryFor(nonce);

let app: AndroidApp;

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "Library");
  await resetHistoryFilters(app);
  await waitForRows(app, 1);
}, 180_000);

afterAll(async () => {
  await clearField(app, SEARCH).catch(() => undefined);
  await app?.detach();
});

test("a tap moves between screens and the nav says which one is current", async () => {
  await gotoView(app, "Devices");
  await waitFor(
    async () => (await count(app, HISTORY_LIST)) === 0,
    "the history list was still in the document on the Devices screen",
  );

  await gotoView(app, "Library");
  await waitForRows(app, 1);
});

test("typing into search filters the list the engine laid out", async () => {
  const before = await rowCount(app);
  expect(before).toBeGreaterThan(1);

  await openHistorySearch(app);
  await typeInto(app, SEARCH, `HARNESS${nonce}`);
  await waitFor(
    async () =>
      (await visibleText(app)).includes(ordinary) && (await rowCount(app)) < before,
    "searching for this run's own clipping never narrowed the list",
  );

  await clearField(app, SEARCH);
  await waitFor(
    async () => (await rowCount(app)) === before,
    "clearing the search never restored the list",
  );
});

test("rows are laid out with a real box, which jsdom cannot show", async () => {
  const boxes = await app.withPage((page) =>
    page.evaluate(
      (selector) =>
        Array.from(document.querySelectorAll(selector)).map((node) => {
          const rect = node.getBoundingClientRect();
          return { width: rect.width, height: rect.height };
        }),
      ROW,
    ),
  );

  expect(boxes.length).toBeGreaterThan(0);
  for (const box of boxes) {
    expect(box.width).toBeGreaterThan(0);
    expect(box.height).toBeGreaterThan(0);
  }
});
