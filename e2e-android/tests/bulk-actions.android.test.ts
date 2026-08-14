/**
 * Selection mode and the bulk bar, on the shipping Android engine.
 *
 * The claim this file exists for is §3.1.5's: in selection mode the per-row
 * actions are **not rendered**, rather than hidden with a class. jsdom would
 * agree with either implementation and so would a screenshot — but a
 * `display: none` button is still in the accessibility tree and still a tab
 * stop, so "hidden" and "absent" are different products for the user who
 * cannot see the screen.
 *
 * Android reaches those actions differently. `HistoryRowActions` renders one
 * "Item actions" trigger opening a dialog, where desktop renders Open, Copy,
 * Pin and Delete side by side on the row. The claim is the same and the
 * surface it is asked of is not, which is the reason this is not a copy of
 * `e2e/tests/bulk-actions.e2e.test.ts`.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems, storedItems } from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import { rowBoxes } from "../src/harness/list.js";
import {
  SEARCH,
  byLabel,
  count,
  filterHistoryTo,
  gotoView,
  rowCount,
  tapButton,
  tapNth,
  visibleText,
  waitFor,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

const BULK_BAR = '[role="region"][aria-label="Selection actions"]';
const CHECKBOX = '[role="checkbox"]';

/** The per-row surface as Android renders it, and the actions the dialog
 *  behind it offers. Both must be absent while a selection is being made. */
const ROW_TRIGGER = "Item actions";
const DIALOG_ACTIONS = ["Copy to clipboard", "Show full contents", "Pin item", "Delete item"];

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "History");

  marker = fixtureMarker("bulk");
  seeded = await addItems(
    app,
    ["alpha", "beta", "gamma", "delta"].map((word) => `${marker} ${word}`),
  );
  await filterHistoryTo(app, marker, `${marker} delta`);
  await waitForRows(app, 4);
}, 300_000);

afterAll(async () => {
  await leaveSelectionMode().catch(() => undefined);
  await cleanUpItems(app, seeded);
  await app?.detach();
});

async function enterSelectionMode(): Promise<void> {
  // A dialog left open by a failing assertion would otherwise swallow the tap
  // and report itself as "selection mode never produced checkboxes".
  if ((await count(app, '[role="dialog"]')) > 0) {
    await tapButton(app, "Cancel", { within: '[role="dialog"]' }).catch(() => undefined);
  }
  await tapButton(app, "Select multiple items");
  await waitFor(
    async () => (await count(app, CHECKBOX)) > 0,
    "selection mode never produced checkboxes",
  );
}

async function leaveSelectionMode(): Promise<void> {
  if ((await count(app, BULK_BAR)) === 0) return;
  await tapButton(app, "Done", { within: BULK_BAR });
  await waitFor(
    async () => (await count(app, CHECKBOX)) === 0,
    "the checkboxes stayed after leaving selection mode",
  );
}

/** The dialog's own text, not the document's: it is portalled beside the app
 *  root, and the row underneath renders the same words. */
async function waitForDialogText(): Promise<string> {
  let text = "";
  await waitFor(
    async () => {
      text = await app.withPage((page) =>
        page.evaluate(
          () => (document.querySelector('[role="dialog"]') as HTMLElement | null)?.innerText ?? "",
        ),
      );
      return text.length > 0;
    },
    "the row actions dialog never opened",
  );
  return text;
}

/** Tap the first `n` checkboxes. Real taps: a checkbox the bulk bar has
 *  covered is a checkbox the user cannot tick. */
async function select(n: number): Promise<void> {
  expect(await count(app, CHECKBOX)).toBeGreaterThanOrEqual(n);
  for (let i = 0; i < n; i += 1) await tapNth(app, CHECKBOX, i);
}

describe("entering selection mode", () => {
  test("the per-row action trigger is rendered when it is off", async () => {
    const triggers = await byLabel(app, ROW_TRIGGER);
    expect(triggers.length).toBeGreaterThan(0);
    expect(triggers[0]!.height).toBeGreaterThan(0);
    expect(triggers[0]!.width).toBeGreaterThan(0);
  });

  test("the dialog behind it offers the actions the row does not show", async () => {
    await tapButton(app, ROW_TRIGGER);

    const shown = await waitForDialogText();
    for (const label of DIALOG_ACTIONS) expect(shown, label).toContain(label);

    await tapButton(app, "Cancel", { within: '[role="dialog"]' });
    await waitFor(
      async () => (await count(app, '[role="dialog"]')) === 0,
      "the row actions dialog never closed",
    );
  });

  test("per-row actions are absent from the document, not merely hidden", async () => {
    await enterSelectionMode();

    // The query is over the whole document and ignores CSS, so a trigger
    // hidden with `display: none` would still be counted here.
    expect(await byLabel(app, ROW_TRIGGER)).toHaveLength(0);

    // ...and the bulk bar that replaced them really is on screen, so the
    // assertion above is about selection mode rather than about an empty list.
    expect(await count(app, BULK_BAR)).toBe(1);
  });

  test("the bar sits above the list rather than floating over it", async () => {
    const layout = await app.withPage((page) =>
      page.evaluate(
        (bar: string, box: string) => {
          const region = document.querySelector(bar) as HTMLElement | null;
          const first = document.querySelector('[role="listitem"]') as HTMLElement | null;
          const check = document.querySelector(box) as HTMLElement | null;
          return {
            barBottom: region ? region.getBoundingClientRect().bottom : NaN,
            rowTop: first ? first.getBoundingClientRect().top : NaN,
            checkbox: check
              ? {
                  width: check.getBoundingClientRect().width,
                  height: check.getBoundingClientRect().height,
                }
              : null,
          };
        },
        BULK_BAR,
        CHECKBOX,
      ),
    );

    // A floating bar covers the rows the user is choosing between.
    expect(layout.barBottom).toBeLessThanOrEqual(layout.rowTop + 1);
    expect(layout.checkbox).not.toBeNull();
    expect(layout.checkbox!.width).toBeGreaterThan(8);
  });
});

describe("the bulk bar", () => {
  test("counts what is selected, in rendered words", async () => {
    await select(2);
    await waitForText(app, "2 items selected");
  });

  test("pins the selection, and then offers to unpin it (CopyPaste-8ebg.55)", async () => {
    await tapButton(app, "Pin", { within: BULK_BAR });
    await waitFor(
      async () =>
        (await storedItems(app)).filter(
          (item) => seeded.includes(item.id) && item.pinned,
        ).length === 2,
      "the store never recorded two pinned items",
      20_000,
    );

    // The toggle's label is a claim about every selected row, so selecting the
    // two rows that are now pinned must flip it.
    await enterSelectionMode();
    await select(2);
    await waitFor(
      async () => (await visibleText(app)).includes("Unpin"),
      "the toggle still offered to pin two already-pinned items",
    );
  }, 120_000);

  test("bulk delete confirms first, then really deletes", async () => {
    await tapButton(app, "Delete", { within: BULK_BAR });
    await waitFor(
      async () => (await count(app, '[role="alertdialog"]')) > 0,
      "bulk delete did not ask for confirmation",
    );

    const copy = await visibleText(app);
    expect(copy).toContain("Delete 2 items?");
    expect(copy).toContain("cannot be undone");

    // The two rows the checkboxes ticked, by the text they render. Never a row
    // count: the virtualiser draws a fixed window, so deleting two rows pulls
    // two more in from below and the count is unchanged.
    const doomed = (await rowBoxes(app))
      .sort((a, b) => a.start - b.start)
      .slice(0, 2);
    expect(doomed).toHaveLength(2);

    await tapButton(app, "Delete", { within: '[role="alertdialog"]' });

    await waitFor(
      async () => {
        const remaining = new Set((await storedItems(app)).map((item) => item.id));
        return doomed.every((row) => !remaining.has(row.id));
      },
      "the store still holds the deleted items",
      20_000,
    );
    await waitFor(
      async () =>
        app.withPage((page) =>
          page.evaluate(
            (ids: string[]) =>
              ids.every((id) => document.getElementById(`history-row-${id}`) === null),
            doomed.map((row) => row.id),
          ),
        ),
      `the deleted rows stayed on screen: ${doomed.map((row) => row.id).join(", ")}`,
      20_000,
    );
  }, 120_000);

  test("leaving selection mode brings the per-row trigger back", async () => {
    await leaveSelectionMode();
    await waitFor(
      async () => (await byLabel(app, ROW_TRIGGER)).length > 0,
      "the per-row action trigger never came back",
    );
    expect(await count(app, CHECKBOX)).toBe(0);
  });
});
