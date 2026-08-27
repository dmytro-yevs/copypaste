/**
 * Selection mode and the bulk bar, driven end to end.
 *
 * The claim this file exists for is §3.1.5's: in selection mode the per-row
 * Pin/Copy/Delete buttons are **not rendered**, rather than hidden with a
 * class. jsdom would agree with either implementation and so would a screenshot
 * — but a `display: none` button is still in the accessibility tree and still a
 * tab stop, so "hidden" and "absent" are different products for the user who
 * cannot see the screen.
 *
 * The rest is the round trip: what the bulk bar says it will do, and what the
 * daemon holds afterwards.
 *
 * WebKitGTK on Linux; green here says nothing about WKWebView or Android's
 * WebView (README).
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import {
  byLabel,
  clickButton,
  rowCount,
  visibleText,
  waitForRows,
} from "../src/harness/ui.js";

const SEED = ["bulk alpha", "bulk beta", "bulk gamma", "bulk delta"];

/** Every per-row action the row renders when it is not being selected. */
const ROW_ACTIONS = ["Copy to clipboard", "Pin item", "Delete item"];

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: SEED });
  await waitForRows(app.browser, SEED.length);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

/** Spread into a plain array: the chainable collection's `length` is itself a
 *  promise, which compares as true against any number. */
async function checkboxes() {
  return [...(await app.browser.$$('[role="checkbox"]'))];
}

async function enterSelectionMode(): Promise<void> {
  await clickButton(app.browser, "Select multiple items");
  await app.browser.waitUntil(async () => (await checkboxes()).length > 0, {
    timeout: 10_000,
    timeoutMsg: "selection mode never produced checkboxes",
  });
}

async function select(count: number): Promise<void> {
  const boxes = await checkboxes();
  expect(boxes.length).toBeGreaterThanOrEqual(count);
  for (let i = 0; i < count; i += 1) await boxes[i]!.click();
}

describe("entering selection mode", () => {
  test("per-row actions are rendered when it is off", async () => {
    for (const label of ROW_ACTIONS) {
      const found = await byLabel(app.browser, label);
      expect(found.length, label).toBeGreaterThan(0);
      expect(found[0]!.height, label).toBeGreaterThan(0);
    }
  });

  test("per-row actions are absent from the document, not merely hidden", async () => {
    await enterSelectionMode();

    for (const label of ROW_ACTIONS) {
      // The query is over the whole document and ignores CSS, so a button
      // hidden with `display: none` would still be counted here.
      expect(await byLabel(app.browser, label), label).toHaveLength(0);
    }

    // ...and the bulk bar that replaced them really is on screen, so the
    // assertion above is about selection mode rather than about an empty list.
    const bar = await app.browser.$('[role="region"][aria-label="Selection actions"]');
    expect(await bar.isDisplayed()).toBe(true);
  });
});

describe("the bulk bar", () => {
  test("counts what is selected, in rendered words", async () => {
    await select(2);
    await app.browser.waitUntil(
      async () => (await visibleText(app.browser)).includes("2 items selected"),
      { timeout: 10_000, timeoutMsg: "the bulk bar never counted the selection" },
    );
  });

  test("pins the selection, and then offers to unpin it (CopyPaste-8ebg.55)", async () => {
    await clickButton(app.browser, "Pin", {
      within: '[aria-label="Selection actions"]',
    });

    await app.browser.waitUntil(
      async () => (await app.daemon.items()).filter((item) => item.pinned).length === 2,
      { timeout: 20_000, timeoutMsg: "the daemon never recorded two pinned items" },
    );

    // The toggle's label is a claim about every selected row, so selecting the
    // two rows that are now pinned must flip it.
    await enterSelectionMode();
    await select(2);
    await app.browser.waitUntil(
      async () => {
        const bar = await app.browser.$('[aria-label="Selection actions"]');
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
        async () => (await visibleText(app.browser)).includes("2 items selected"),
        { timeout: 10_000, timeoutMsg: "the bulk bar never counted the selection" },
      );
    }

    await clickButton(app.browser, "Delete", {
      within: '[aria-label="Selection actions"]',
    });

    const dialog = await app.browser.$('[role="alertdialog"]');
    await dialog.waitForDisplayed({
      timeout: 10_000,
      timeoutMsg: "bulk delete did not ask for confirmation",
    });
    const copy = await dialog.getText();
    expect(copy).toContain("Delete 2 items?");
    expect(copy).toContain("You have a few seconds to undo it.");

    const before = await app.daemon.items();
    await clickButton(app.browser, "Delete", { within: '[role="alertdialog"]' });

    await app.browser.waitUntil(
      async () => (await app.daemon.items()).length === before.length - 2,
      { timeout: 20_000, timeoutMsg: "the daemon still holds the deleted items" },
    );
    await app.browser.waitUntil(
      async () => (await rowCount(app.browser)) === SEED.length - 2,
      { timeout: 20_000, timeoutMsg: "the deleted rows stayed on screen" },
    );
  });

  test("cancelling selection mode brings the per-row actions back", async () => {
    const bar = await app.browser.$('[aria-label="Selection actions"]');
    if (await bar.isExisting()) {
      await clickButton(app.browser, "Done", {
        within: '[aria-label="Selection actions"]',
      });
    }

    await app.browser.waitUntil(
      async () => (await byLabel(app.browser, "Delete item")).length > 0,
      { timeout: 10_000, timeoutMsg: "the per-row actions never came back" },
    );
    expect(await checkboxes()).toHaveLength(0);
  });
});
