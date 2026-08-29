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
  captureRowSelectionClick,
  type RowSelectionClickReceipt,
} from "../src/harness/row-selection-diagnostics.js";
import {
  selectionDiagnosticFailure,
  type SelectionProbeRead,
  withSelectionActionProbe,
} from "../src/harness/selection-diagnostics.js";
import {
  clickButton,
  count,
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
const ROW_ACTIONS = ["Copy", "Pin item", "Delete item"];

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

async function enterSelectionMode(
  firstId?: string,
  receipts?: RowSelectionClickReceipt[],
  intendedIds: readonly string[] = [],
): Promise<void> {
  const bar = await app.browser.$(BULK_BAR);
  if (await bar.isExisting()) return;

  // The controls are revealed by the row hover state before selection starts.
  if (firstId) {
    const row = await app.browser.$(`#history-row-${firstId}`);
    const box = await row.$('[role="checkbox"]');
    const receipt = await captureRowSelectionClick(
      app.browser,
      firstId,
      async () => {
        await box.moveTo();
        await box.click();
      },
      { intendedIds, priorReceipts: receipts ?? [] },
    );
    receipts?.push(receipt);
  } else {
    const boxes = await rowCheckboxes();
    expect(boxes).toHaveLength(SEED.length);
    await boxes[0]!.moveTo();
    await boxes[0]!.click();
  }
  try {
    await app.browser.waitUntil(
      async () => (await app.browser.$(BULK_BAR)).isDisplayed(),
      {
        timeout: 10_000,
        timeoutMsg: "selecting a row never opened the bulk bar",
      },
    );
  } catch (cause) {
    throw selectionDiagnosticFailure(cause, "row-selection-toolbar-open", {
      budgetMs: 10_000,
      intendedIds,
      rowClickReceipts: receipts ?? [],
      checkedIds: await bestEffortSelectedRowIds(),
    });
  }
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

async function bestEffortSelectedRowIds(): Promise<string[] | "unavailable"> {
  try {
    return await selectedRowIds();
  } catch {
    return "unavailable";
  }
}

async function selectIds(
  ids: readonly string[],
  receipts: RowSelectionClickReceipt[],
): Promise<void> {
  for (const id of ids) {
    const row = await app.browser.$(`#history-row-${id}`);
    const checkbox = await row.$('[role="checkbox"]');
    if ((await checkbox.getAttribute("aria-checked")) === "true") continue;
    const receipt = await captureRowSelectionClick(
      app.browser,
      id,
      async () => {
        await checkbox.moveTo();
        await checkbox.click();
      },
      { intendedIds: ids, priorReceipts: receipts },
    );
    receipts.push(receipt);
  }

  const expected = [...ids].sort();
  try {
    await app.browser.waitUntil(
      async () =>
        (await selectedRowIds()).sort().join("\u0000") ===
        expected.join("\u0000"),
      { timeout: 10_000, timeoutMsg: "the expected rows were not selected" },
    );
  } catch (cause) {
    throw selectionDiagnosticFailure(cause, "row-selection-settle", {
      budgetMs: 10_000,
      intendedIds: ids,
      rowClickReceipts: receipts,
      checkedIds: await bestEffortSelectedRowIds(),
    });
  }
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
  await withSelectionActionProbe(app.browser, "Done", (probe) =>
    probe.perform(
      "bulk-done",
      {
        budgetMs: 10_000,
      },
      () => clickButton(app.browser, "Done", { within: BULK_BAR }),
      () =>
        app.browser.waitUntil(
          async () => (await count(app.browser, BULK_BAR)) === 0,
          {
            timeout: 10_000,
            timeoutMsg: "the bulk bar stayed after leaving selection mode",
          },
        ),
    ),
  );
}

function boundaryChanged(
  first: SelectionProbeRead,
  last: SelectionProbeRead,
): boolean {
  return JSON.stringify(first) !== JSON.stringify(last);
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
    expect(await count(app.browser, BULK_BAR)).toBe(1);

    for (const label of ROW_ACTIONS) {
      // Scope the document query to history rows: the desktop inspector keeps
      // its legitimate action for the active item during selection mode.
      // `count` ignores CSS, so display:none controls would still be counted.
      expect(
        await count(app.browser, `${HISTORY_ROWS} [aria-label="${label}"]`),
        label,
      ).toBe(0);
    }

    // Keep the accessibility guarantee explicit: no row action remains in a
    // history row's DOM, rather than merely being visually hidden.
    expect(
      await app.browser.execute((selector: string) => {
        const rows = Array.from(document.querySelectorAll(selector));
        return rows.every(
          (row) =>
            row.querySelectorAll(
              '[aria-label="Copy"], [aria-label="Pin item"], [aria-label="Delete item"]',
            ).length === 0,
        );
      }, HISTORY_ROWS),
    ).toBe(true);

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

    await withSelectionActionProbe(app.browser, "Pin", async (probe) => {
      await probe.perform(
        "bulk-pin-daemon-confirmation",
        { budgetMs: 20_000, selectedIds: pinnedIds },
        () => clickButton(app.browser, "Pin", { within: BULK_BAR }),
        () =>
          app.browser.waitUntil(
            async () =>
              (await app.daemon.items()).filter((item) => item.pinned).length ===
              2,
            {
              timeout: 20_000,
              timeoutMsg: "the daemon never recorded two pinned items",
            },
          ),
      );

      const afterDaemon = await probe.read();
      let lastSelectionState = afterDaemon;
      let selectionPolls = 0;

      // Bulk actions release selection when every write succeeds. The refresh
      // that moves rows into the pinned section must not hold that UI state
      // hostage, so the next selection targets stable ids after both claims
      // have independently become true.
      try {
        await app.browser.waitUntil(
          async () => {
            selectionPolls += 1;
            lastSelectionState = await probe.read();
            return (await count(app.browser, BULK_BAR)) === 0;
          },
          {
            timeout: 10_000,
            timeoutMsg: "pinning did not clear the bulk selection",
          },
        );
      } catch (cause) {
        throw await probe.failure(cause, "bulk-pin-selection-release", {
          budgetMs: 10_000,
          selectedIds: pinnedIds,
          polls: selectionPolls,
          stateChanged: boundaryChanged(afterDaemon, lastSelectionState),
          afterDaemon,
          lastSelectionState,
        });
      }

      const afterSelection = await probe.read();
      let lastRenderedState = afterSelection;
      let renderedPolls = 0;
      try {
        await app.browser.waitUntil(
          async () => {
            renderedPolls += 1;
            lastRenderedState = await probe.read();
            for (const id of pinnedIds) {
              const row = await app.browser.$(`#history-row-${id}`);
              if (!(await row.isExisting())) return false;
              const badges = await row.$$('[title="Pinned"]');
              if ((await badges.length) !== 1) return false;
              if (!(await badges[0]!.isDisplayed())) return false;
              const size = await badges[0]!.getSize();
              if (size.width <= 0 || size.height <= 0) return false;
            }
            return true;
          },
          {
            timeout: 10_000,
            timeoutMsg: "the pinned rows never reached the rendered history",
          },
        );
      } catch (cause) {
        throw await probe.failure(cause, "bulk-pin-rendered-badges", {
          budgetMs: 10_000,
          selectedIds: pinnedIds,
          polls: renderedPolls,
          stateChanged: boundaryChanged(afterSelection, lastRenderedState),
          afterDaemon,
          afterSelection,
          lastRenderedState,
        });
      }

      // The toggle's label is a claim about every selected row, so selecting
      // the same two rows after their reorder must flip it.
      const rowClickReceipts: RowSelectionClickReceipt[] = [];
      await enterSelectionMode(pinnedIds[0], rowClickReceipts, pinnedIds);
      await selectIds(pinnedIds, rowClickReceipts);
      const unpinSelector = `${BULK_BAR} button[aria-label="Unpin"]`;
      try {
        await app.browser.waitUntil(
          async () => {
            if ((await count(app.browser, unpinSelector)) !== 1) return false;
            const selectedIds = (await selectedRowIds()).sort();
            const expectedIds = [...pinnedIds].sort();
            if (selectedIds.join("\u0000") !== expectedIds.join("\u0000")) {
              return false;
            }
            const unpin = await app.browser.$(unpinSelector);
            return (
              (await unpin.isDisplayed()) &&
              (await unpin.isEnabled()) &&
              (await unpin.isClickable())
            );
          },
          {
            timeout: 10_000,
            timeoutMsg:
              "the toggle still offered to pin two already-pinned items",
          },
        );
      } catch (cause) {
        throw await probe.failure(cause, "bulk-pin-unpin-presentation", {
          budgetMs: 10_000,
          selectedIds: pinnedIds,
          rowClickReceipts,
        });
      }
    });
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
