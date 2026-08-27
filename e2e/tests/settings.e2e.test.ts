/**
 * Preferences: the sections, and the controls that leave the screen.
 *
 * The app's own preferences are round-tripped because they have a layout
 * consequence — preview lines feeds INV-5's row reservation, and appearance has
 * to survive a reload (INV-22). The daemon's are round-tripped through the
 * daemon, since the screen writing them is the only thing that can be wrong.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { accessibleSurface, expectNoFilesystemPath } from "../src/harness/leaks.js";
import {
  clickButton,
  gotoView,
  rowBoxes,
  visibleText,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

const TABS = [
  "Appearance",
  "Clipboard behavior",
  "Privacy & retention",
  "Shortcuts",
  "Device sync",
  "Cloud sync",
  "Storage & history",
  "Diagnostics",
  "Runtime events",
  "About",
];

/** `rowHeight(n)` from `lib/layout.ts`, duplicated deliberately: a test that
 *  imported the function could not catch it changing. */
const ROW_HEIGHT = { 1: 67, 2: 88 } as const;

let app: App;

const config = () =>
  app.daemon.json<{ config: { retention_days: number } }>(["config", "show"]);

beforeAll(async () => {
  app = await startApp({ seed: ["a settings fixture", "another one"] });
  await waitForRows(app.browser, 2);
  await gotoView(app.browser, "Preferences");
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

async function openTab(label: string): Promise<void> {
  const trigger = await app.browser.$(`[role="tab"]=${label}`);
  await trigger.waitForClickable({ timeout: 15_000 });
  await trigger.click();
  await app.browser.waitUntil(
    async () => (await trigger.getAttribute("aria-selected")) === "true",
    { timeout: 10_000, timeoutMsg: `the ${label} tab never became selected` },
  );
  // DMY-138: the pane is selected but its content may still be loading
  // asynchronously. Wait for aria-busy to clear so assertions see real content.
  await app.browser.waitUntil(
    async () =>
      (await app.browser.execute(function () {
        const p = document.querySelector('[role="tabpanel"]:not([hidden])');
        return p !== null && p.querySelector("[aria-busy]") === null;
      })) === true,
    { timeout: 30_000, timeoutMsg: `the ${label} pane never finished loading` },
  );
}

/**
 * The visible panel, as the engine laid it out.
 *
 * `:not([hidden])` is load-bearing: a tab that has been opened once stays in
 * the DOM hidden, so the first `[role="tabpanel"]` is whichever pane was
 * visited first rather than the one on screen.
 */
async function panel() {
  return (await app.browser.execute(function () {
    const el = document.querySelector(
      '[role="tabpanel"]:not([hidden])',
    ) as HTMLElement | null;
    if (!el) return null;
    const rect = el.getBoundingClientRect();
    return { width: rect.width, height: rect.height, text: el.innerText.trim() };
  })) as { width: number; height: number; text: string } | null;
}

describe("the tabs", () => {
  test("every one opens onto a pane with a real box", async () => {
    for (const label of TABS) {
      await openTab(label);
      const pane = await panel();
      expect(pane, label).not.toBeNull();
      expect(pane!.height, label).toBeGreaterThan(20);
      expect(pane!.width, label).toBeGreaterThan(100);
      expect(pane!.text.length, label).toBeGreaterThan(20);
    }
  });

  test("the sidebar keeps every section reachable without horizontal overflow", async () => {
    const row = (await app.browser.execute(function () {
      const list = document.querySelector('[role="tablist"]') as HTMLElement | null;
      if (!list) return null;
      const box = list.getBoundingClientRect();
      const tabs = Array.prototype.map.call(
        list.querySelectorAll('[role="tab"]'),
        function (node) {
          const rect = (node as HTMLElement).getBoundingClientRect();
          return { right: rect.right, width: rect.width, text: (node as HTMLElement).innerText };
        },
      ) as Array<{ right: number; width: number; text: string }>;
      return {
        wrap: getComputedStyle(list).flexWrap,
        overflow: list.scrollWidth - list.clientWidth,
        right: box.right,
        tabs,
      };
    })) as {
      wrap: string;
      overflow: number;
      right: number;
      tabs: Array<{ right: number; width: number; text: string }>;
    } | null;

    expect(row).not.toBeNull();
    expect(row!.wrap).toBe("nowrap");
    expect(row!.overflow).toBeLessThanOrEqual(1);
    expect(row!.tabs).toHaveLength(TABS.length);
    for (const tab of row!.tabs) {
      expect(tab.width, tab.text).toBeGreaterThan(0);
      expect(tab.right, tab.text).toBeLessThanOrEqual(row!.right + 1);
    }
  });

  test("names no filesystem path on any pane (INV-12)", async () => {
    for (const label of TABS) {
      await openTab(label);
      expectNoFilesystemPath(
        await accessibleSurface(app.browser),
        app.daemon.dataHome,
      );
    }
  });
});

describe("the service's own settings", () => {
  test("a value chosen on the screen reaches the daemon", async () => {
    await openTab("Privacy & retention");
    const select = await app.browser.$('select[aria-label="Drop items older than"]');
    await select.waitForDisplayed({ timeout: 10_000 });
    await select.selectByAttribute("value", "30");

    // Read it back out of the daemon, not out of the control that wrote it.
    await app.browser.waitUntil(
      async () => (await config()).config.retention_days === 30,
      { timeout: 10_000, interval: 250, timeoutMsg: "the daemon never took the value" },
    );
  });

  test("storage reports what the service holds", async () => {
    await openTab("Storage & history");
    expect(await visibleText(app.browser)).toContain("Items stored");
  });
});

describe("a preference that changes layout", () => {
  test("preview lines re-reserves every row (INV-5)", async () => {
    await openTab("Clipboard behavior");
    const thumb = await app.browser.$('[aria-label="Preview lines"]');
    await thumb.click();

    await app.browser.keys(["Home"]);
    await waitForText(app.browser, "1 line");

    await gotoView(app.browser, "Library");
    await app.browser.waitUntil(
      async () => {
        const rows = await rowBoxes(app.browser);
        return rows.length > 0 && rows.every((row) => Math.round(row.height) === ROW_HEIGHT[1]);
      },
      { timeout: 15_000, timeoutMsg: "rows never shrank to the one-line reservation" },
    );

    await gotoView(app.browser, "Preferences");
    await openTab("Clipboard behavior");
    await (await app.browser.$('[aria-label="Preview lines"]')).click();
    await app.browser.keys(["ArrowRight"]);
    await waitForText(app.browser, "2 lines");

    await gotoView(app.browser, "Library");
    await app.browser.waitUntil(
      async () => {
        const rows = await rowBoxes(app.browser);
        return rows.length > 0 && rows.every((row) => Math.round(row.height) === ROW_HEIGHT[2]);
      },
      { timeout: 15_000, timeoutMsg: "rows never returned to the two-line reservation" },
    );
  });
});

describe("appearance", () => {
  test("survives a reload of the window (INV-22)", async () => {
    await gotoView(app.browser, "Preferences");
    await openTab("Appearance");
    await clickButton(app.browser, "Teal");

    await app.browser.waitUntil(
      async () =>
        (await app.browser.execute(
          () => document.documentElement.dataset.accent ?? "",
        )) === "teal",
      { timeout: 10_000, timeoutMsg: "the accent never reached <html>" },
    );

    await app.browser.execute(() => location.reload());
    await app.browser.waitUntil(
      async () =>
        (await app.browser.execute(
          () => document.querySelectorAll("nav").length,
        )) > 0,
      { timeout: 60_000, interval: 250, timeoutMsg: "the window never came back" },
    );

    // Persisted, and applied to the document that has just been painted.
    expect(
      await app.browser.execute(() => document.documentElement.dataset.accent),
    ).toBe("teal");

    await gotoView(app.browser, "Preferences");
    await openTab("Appearance");
    const teal = await app.browser.$('[aria-label="Teal"]');
    expect(await teal.getAttribute("aria-pressed")).toBe("true");
  });
});

describe("with the service down", () => {
  test("the client-owned settings still work", async () => {
    await app.daemon.kill();
    await gotoView(app.browser, "Preferences");
    await openTab("Appearance");

    // Nothing on this pane needs the daemon, so it must still be operable.
    await clickButton(app.browser, "Rose");
    await app.browser.waitUntil(
      async () =>
        (await app.browser.execute(
          () => document.documentElement.dataset.accent ?? "",
        )) === "rose",
      { timeout: 10_000, timeoutMsg: "Settings stopped working when the service did" },
    );
  });

  test("the panes that do need it say so without naming a path", async () => {
    for (const label of ["About", "Storage & history", "Cloud sync"]) {
      await openTab(label);
      const pane = await panel();
      expect(pane!.text.length, label).toBeGreaterThan(20);
      expectNoFilesystemPath(
        await accessibleSurface(app.browser),
        app.daemon.dataHome,
      );
    }
  });
});
