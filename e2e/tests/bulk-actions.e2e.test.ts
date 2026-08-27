/**
 * Selection mode and the bulk bar, driven end to end.
 *
 * In selection mode the per-row Copy/Pin/Delete controls are absent rather
 * than merely hidden. A document query catches a regression to display:none,
 * which would leave those controls in the accessibility tree and tab order.
 *
 * WebKitGTK on Linux; green here says nothing about WKWebView or Android's
 * WebView (README).
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import {
  byLabel,
  clickButton,
  HISTORY_LIST,
  ROW,
  rowCount,
  visibleText,
  waitForRows,
} from "../src/harness/ui.js";

const SEED = ["bulk alpha", "bulk beta", "bulk gamma", "bulk delta"];
const BULK_BAR = '[role="toolbar"][aria-label="Selection actions"]';
const HISTORY_ROWS = `${HISTORY_LIST} ${ROW}`;
const ROW_CHECKBOXES = `${HISTORY_ROWS} [role="checkbox"]`;
const ROW_ACTIONS = ["Copy to clipboard", "Pin item", "Delete item"];

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: SEED });
  await waitForRows(app.browser, SEED.length);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

async function rowCheckboxes() {
  return [...(await app.browser.$$(ROW_CHECKBOXES))];
}

async function enterSelectionMode(firstId?: string): Promise<void> {
  const bar = await app.browser.$(BULK_BAR);
  if (await bar.isExisting()) return;

  // The controls are revealed by the row hover state before selection starts.
  if (firstId) {
    const row = await app.browser.$(`#history-row-${firstId}`);
    const box = await row.$('[role="checkbox"]');
    await box.moveTo();
    await box.click();
  } else {
    const boxes = await rowCheckboxes();
    expect(boxes).toHaveLength(SEED.length);
    await boxes[0]!.moveTo();
    await boxes[0]!.click();
  }
  await app.browser.waitUntil(
    async () => (await app.browser.$(BULK_BAR)).isDisplayed(),
    {
      timeout: 10_000,
      timeoutMsg: "selecting a row never opened the bulk bar",
    },
  );
}

async function selectedRowIds(): Promise<string[]> {
  return (await app.browser.execute(
    (selector: string) =>
      Array.from(document.querySelectorAll(selector))
        .filter((row) => row.getAttribute("aria-checked") === "true")
        .map((row) => row.id.replace(/^history-row-/, "")),
    HISTORY_ROWS,
  )) as string[];
}

async function selectIds(ids: readonly string[]): Promise<void> {
  for (const id of ids) {
    const row = await app.browser.$(`#history-row-${id}`);
    const checkbox = await row.$('[role="checkbox"]');
    if ((await checkbox.getAttribute("aria-checked")) === "true") continue;
    await checkbox.moveTo();
    await checkbox.click();
  }

  const expected = [...ids].sort();
  await app.browser.waitUntil(
    async () =>
      (await selectedRowIds()).sort().join("\u0000") === expected.join("\u0000"),
    { timeout: 10_000, timeoutMsg: "the expected rows were not selected" },
  );
}

async function select(count: number): Promise<void> {
  const boxes = await rowCheckboxes();
  expect(boxes.length).toBeGreaterThanOrEqual(count);

  let selected = 0;
  for (const box of boxes) {
    if ((await box.getAttribute("aria-checked")) === "true") selected += 1;
  }
  expect(selected).toBeLessThanOrEqual(count);

  for (const box of boxes) {
    if (selected >= count) break;
    if ((await box.getAttribute("aria-checked")) === "true") continue;
    await box.moveTo();
    await box.click();
    selected += 1;
  }

  await app.browser.waitUntil(
    async () => {
      const current = await rowCheckboxes();
      let checked = 0;
      for (const box of current) {
        if ((await box.getAttribute("aria-checked")) === "true") checked += 1;
      }
      return checked === count;
    },
    { timeout: 10_000, timeoutMsg: `expected ${count} selected rows` },
  );
}

async function leaveSelectionMode(): Promise<void> {
  const bar = await app.browser.$(BULK_BAR);
  if (!(await bar.isExisting())) return;
  await clickButton(app.browser, "Done", { within: BULK_BAR });
  await app.browser.waitUntil(
    async () => !(await app.browser.$(BULK_BAR)).isExisting(),
    {
      timeout: 10_000,
      timeoutMsg: "the bulk bar stayed after leaving selection mode",
    },
  );
}

describe("entering selection mode", () => {
  test("each row exposes one semantic selection control", async () => {
    const perRow = (await app.browser.execute(
      (selector: string) =>
        Array.from(
          document.querySelectorAll(selector),
          (row) => row.querySelectorAll('[role="checkbox"]').length,
        ),
      HISTORY_ROWS,
    )) as number[];

    expect(perRow).toEqual(SEED.map(() => 1));
  });

  test("per-row actions are absent from the document, not merely hidden", async () => {
    await enterSelectionMode();

    for (const label of ROW_ACTIONS) {
      // The query ignores CSS, so display:none controls would still be counted.
      expect(await byLabel(app.browser, label), label).toHaveLength(0);
    }

    // The bulk bar confirms this is selection mode rather than an empty list.
    const bar = await app.browser.$(BULK_BAR);
    expect(await bar.isDisplayed()).toBe(true);
  });
});

describe("the bulk bar", () => {
  test("counts what is selected, in rendered words", async () => {
    await select(2);
    await app.browser.waitUntil(
      async () => (await visibleText(app.browser)).includes("2 items selected"),
      {
        timeout: 10_000,
        timeoutMsg: "the bulk bar never counted the selection",
      },
    );
  });

  test("pins the selection, and then offers to unpin it (CopyPaste-8ebg.55)", async () => {
    const pinnedIds = await selectedRowIds();
    expect(pinnedIds).toHaveLength(2);

    await clickButton(app.browser, "Pin", {
      within: BULK_BAR,
    });

    await app.browser.waitUntil(
      async () =>
        (await app.daemon.items()).filter((item) => item.pinned).length === 2,
      {
        timeout: 20_000,
        timeoutMsg: "the daemon never recorded two pinned items",
      },
    );

    // Bulk actions release selection on success. The pin write also moves the
    // rows into the pinned section, so the next selection must target their
    // stable ids rather than assuming the old row positions.
    await app.browser.waitUntil(
      async () => !(await app.browser.$(BULK_BAR)).isExisting(),
      {
        timeout: 10_000,
        timeoutMsg: "pinning did not clear the bulk selection",
      },
    );
    await app.browser.waitUntil(
      async () => {
        for (const id of pinnedIds) {
          const row = await app.browser.$(`#history-row-${id}`);
          if (
            !(await row.isExisting()) ||
            !(await row.getText()).includes("Pinned")
          ) {
            return false;
          }
        }
        return true;
      },
      {
        timeout: 10_000,
        timeoutMsg: "the pinned rows never reached the rendered history",
      },
    );

    // The toggle's label is a claim about every selected row, so selecting the
    // same two rows after their reorder must flip it.
    await enterSelectionMode(pinnedIds[0]);
    await selectIds(pinnedIds);
    await app.browser.waitUntil(
      async () => {
        const bar = await app.browser.$(BULK_BAR);
        return (await bar.getText()).includes("Unpin");
      },
      {
        timeout: 10_000,
        timeoutMsg: "the toggle still offered to pin two already-pinned items",
      },
    );
  });

  test("bulk delete confirms first, then really deletes", async () => {
    if (!(await visibleText(app.browser)).includes("2 items selected")) {
      await enterSelectionMode();
      await select(2);
      await app.browser.waitUntil(
        async () =>
          (await visibleText(app.browser)).includes("2 items selected"),
        {
          timeout: 10_000,
          timeoutMsg: "the bulk bar never counted the selection",
        },
      );
    }

    await clickButton(app.browser, "Delete", {
      within: BULK_BAR,
    });

    const dialog = await app.browser.$('[role="alertdialog"]');
    await dialog.waitForDisplayed({
      timeout: 10_000,
      timeoutMsg: "bulk delete did not ask for confirmation",
    });
    const copy = await dialog.getText();
    expect(copy).toContain("Delete 2 items?");
    expect(copy).toContain(
      "This permanently removes the selected clipboard items.",
    );

    const before = await app.daemon.items();
    await clickButton(app.browser, "Delete", {
      within: '[role="alertdialog"]',
    });

    await app.browser.waitUntil(
      async () => (await app.daemon.items()).length === before.length - 2,
      {
        timeout: 20_000,
        timeoutMsg: "the daemon still holds the deleted items",
      },
    );
    await app.browser.waitUntil(
      async () => (await rowCount(app.browser)) === SEED.length - 2,
      { timeout: 20_000, timeoutMsg: "the deleted rows stayed on screen" },
    );
  });

  test("leaving selection mode keeps the row controls available", async () => {
    await leaveSelectionMode();
    expect(await rowCheckboxes()).toHaveLength(2);
  });
});
