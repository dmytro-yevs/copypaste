/**
 * Selection mode and the bulk bar, on the shipping Android engine.
 *
 * Each row exposes one full-surface activation and one semantic selection
 * control. Activating a row opens the current detail surface, whose explicit
 * copy, pin and delete actions remain accessible on Android.
 */
import { afterAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { addItems, cleanUpItems, storedItems } from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import { itemRows, rowBoxes } from "../src/harness/list.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import {
  ROW_SELECTION,
  ROW,
  SEARCH,
  count,
  filterHistoryTo,
  gotoView,
  interactableElementBox,
  rowCount,
  tapButton,
  tapElement,
  visibleText,
  waitFor,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

const BULK_BAR = '[role="toolbar"][aria-label="Selection actions"]';
const CHECKBOX = ROW_SELECTION;
const DETAIL_ACTIONS = ["Copy", "Pin item", "Delete item"];

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

beforeAllWithEvidence("bulk-actions", async () => {
  app = await attachToApp();
  await gotoView(app, "Library");

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
  if ((await count(app, BULK_BAR)) > 0) return;
  // A dialog left open by a failing assertion would otherwise swallow the tap
  // and report itself as "selection mode never produced checkboxes".
  if ((await count(app, '[role="dialog"]')) > 0) {
    await tapButton(app, "Close", { within: '[role="dialog"]' }).catch(
      async () =>
        tapButton(app, "Cancel", { within: '[role="dialog"]' }).catch(
          () => undefined,
        ),
    );
  }
  await tapElement(app, CHECKBOX);
  await waitFor(
    async () => (await count(app, BULK_BAR)) === 1,
    "selecting a row never opened the bulk bar",
  );
}

async function leaveSelectionMode(): Promise<void> {
  if ((await count(app, BULK_BAR)) === 0) return;
  await tapButton(app, "Done", { within: BULK_BAR });
  await waitFor(
    async () => (await count(app, BULK_BAR)) === 0,
    "selection mode stayed active after Done",
  );
}

/** Tap the first `n` checkboxes. Real taps: a checkbox the bulk bar has
 *  covered is a checkbox the user cannot tick. */
async function select(n: number): Promise<void> {
  expect(await count(app, CHECKBOX)).toBeGreaterThanOrEqual(n);
  while ((await count(app, `${CHECKBOX}[aria-checked="true"]`)) < n) {
    await tapElement(app, `${CHECKBOX}:not([aria-checked="true"])`);
  }
  await waitFor(
    async () => (await count(app, `${CHECKBOX}[aria-checked="true"]`)) === n,
    `expected ${n} selected rows`,
  );
}

async function selectedRowIds(): Promise<string[]> {
  return app.withPage((page) =>
    page.evaluate(
      (selector) =>
        Array.from(document.querySelectorAll(selector), (checkbox) =>
          checkbox
            .closest('[role="listitem"]')
            ?.id.replace(/^history-row-/, ""),
        ).filter((id): id is string => Boolean(id)),
      `${CHECKBOX}[aria-checked="true"]`,
    ),
  );
}

async function selectIds(ids: readonly string[]): Promise<void> {
  for (const id of ids) {
    const selector = `#history-row-${id} [role="checkbox"]`;
    if ((await count(app, `${selector}[aria-checked="true"]`)) === 0) {
      await tapElement(app, selector);
    }
  }
  await waitFor(async () => {
    const selected = (await selectedRowIds()).sort();
    return selected.join("\u0000") === [...ids].sort().join("\u0000");
  }, "the expected rows were not selected");
}

describe("entering selection mode", () => {
  test("each row exposes one activation and one semantic selection control", async () => {
    const rows = await app.withPage((page) =>
      page.evaluate(
        (selector) =>
          Array.from(document.querySelectorAll(selector))
            .filter((row) => (row as HTMLElement).id.startsWith("history-row-"))
            .map((row) => {
              const action = row.querySelector<HTMLButtonElement>(
                'button:not([role="checkbox"])',
              );
              return {
                actions: row.querySelectorAll('button:not([role="checkbox"])')
                  .length,
                actionLabel: action?.getAttribute("aria-label") ?? "",
                selections: row.querySelectorAll('[role="checkbox"]').length,
              };
            }),
        ROW,
      ),
    );

    expect(rows.length).toBeGreaterThanOrEqual(4);
    expect(rows.every((row) => row.actions === 1)).toBe(true);
    expect(rows.every((row) => row.actionLabel.length > 0)).toBe(true);
    expect(rows.every((row) => row.selections === 1)).toBe(true);
  });

  test("activating a row opens the accessible item actions", async () => {
    await tapElement(app, `${ROW} button:not([role="checkbox"])`);
    await waitFor(
      async () => (await count(app, '[role="dialog"]')) === 1,
      "activating the row never opened its detail dialog",
    );
    const actions = await app.withPage((page) =>
      page.evaluate(() => {
        const dialog = document.querySelector('[role="dialog"]');
        return Array.from(
          dialog?.querySelectorAll("button") ?? [],
          (button) =>
            button.getAttribute("aria-label") ??
            button.textContent?.trim() ??
            "",
        );
      }),
    );
    for (const label of DETAIL_ACTIONS) expect(actions).toContain(label);
    await tapButton(app, "Close", { within: '[role="dialog"]' });
    await waitFor(
      async () => (await count(app, '[role="dialog"]')) === 0,
      "the item detail dialog never closed",
    );
  });

  test("selection mode exposes checked state on every rendered item", async () => {
    await enterSelectionMode();
    const states = await app.withPage((page) =>
      page.evaluate(
        (selector) =>
          Array.from(document.querySelectorAll(selector))
            .filter((row) => (row as HTMLElement).id.startsWith("history-row-"))
            .map((row) => ({
              checked: row.getAttribute("aria-checked"),
              selections: row.querySelectorAll('[role="checkbox"]').length,
            })),
        ROW,
      ),
    );
    expect(states.length).toBeGreaterThan(0);
    expect(
      states.every((row) => row.checked === "true" || row.checked === "false"),
    ).toBe(true);
    expect(states.every((row) => row.selections === 1)).toBe(true);
    expect(await count(app, BULK_BAR)).toBe(1);
  });

  test("the bar sits above the list rather than floating over it", async () => {
    const layout = await app.withPage((page) =>
      page.evaluate(
        (bar: string, box: string) => {
          const region = document.querySelector(bar) as HTMLElement | null;
          const first = document.querySelector(
            '[role="listitem"]',
          ) as HTMLElement | null;
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
    const pinnedIds = await selectedRowIds();
    expect(pinnedIds).toHaveLength(2);
    await tapButton(app, "Pin", { within: BULK_BAR });
    await waitFor(
      async () =>
        (await storedItems(app)).filter(
          (item) => seeded.includes(item.id) && item.pinned,
        ).length === 2,
      "the store never recorded two pinned items",
      20_000,
    );
    await waitFor(
      async () => (await count(app, BULK_BAR)) === 0,
      "pinning did not clear the bulk selection",
    );

    // The toggle's label is a claim about every selected row, so selecting the
    // two rows that are now pinned must flip it.
    await enterSelectionMode();
    await selectIds(pinnedIds);
    expect((await selectedRowIds()).sort()).toEqual([...pinnedIds].sort());
    await waitFor(
      async () =>
        (await interactableElementBox(app, 'button[aria-label="Unpin"]')) !==
        null,
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
    expect(copy).toContain(
      "This permanently removes the selected clipboard items.",
    );

    // The two rows the checkboxes ticked, by the text they render. Never a row
    // count: the virtualiser draws a fixed window, so deleting two rows pulls
    // two more in from below and the count is unchanged.
    const doomed = itemRows(await rowBoxes(app))
      .sort((a, b) => a.start - b.start)
      .slice(0, 2);
    expect(doomed).toHaveLength(2);

    await tapButton(app, "Delete", { within: '[role="alertdialog"]' });

    await waitFor(
      async () => {
        const remaining = new Set(
          (await storedItems(app)).map((item) => item.id),
        );
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
              ids.every(
                (id) => document.getElementById(`history-row-${id}`) === null,
              ),
            doomed.map((row) => row.id),
          ),
        ),
      `the deleted rows stayed on screen: ${doomed.map((row) => row.id).join(", ")}`,
      20_000,
    );
  }, 120_000);

  test("leaving selection mode keeps semantic selection controls available", async () => {
    await leaveSelectionMode();
    expect(await count(app, CHECKBOX)).toBeGreaterThan(0);
    const checkedStates = await app.withPage((page) =>
      page.evaluate(
        (selector) =>
          Array.from(document.querySelectorAll(selector))
            .filter((row) => (row as HTMLElement).id.startsWith("history-row-"))
            .map((row) => row.getAttribute("aria-checked")),
        ROW,
      ),
    );
    expect(checkedStates.every((state) => state === null)).toBe(true);
  });
});
