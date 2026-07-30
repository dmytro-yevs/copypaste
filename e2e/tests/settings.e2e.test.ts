/**
 * Settings: the tabs, and the two preferences that leave the screen.
 *
 * What this can exercise is bounded by what the app can reach. The daemon has
 * `GetConfig`/`SetConfig` and the CLI drives them (`daemon-config.e2e.test.ts`
 * covers that contract), but no Tauri command routes either one, so there is no
 * configuration round trip to drive from here. The screen's honesty about that
 * is asserted instead — a badge that says the service owns retention, rather
 * than a control that pretends to set it.
 *
 * The preferences that *are* the app's own do get a full round trip, because
 * they are the ones with a layout consequence: preview lines is the input to
 * INV-5's row reservation, and appearance has to survive a reload (INV-22).
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

const TABS = ["Appearance", "List", "Shortcut", "Sync", "Storage", "About"];

/** `rowHeight(n)` from `lib/layout.ts`, duplicated deliberately: a test that
 *  imported the function could not catch it changing. */
const ROW_HEIGHT = { 1: 63, 2: 84 } as const;

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: ["a settings fixture", "another one"] });
  await waitForRows(app.browser, 2);
  await gotoView(app.browser, "Settings");
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

  test("the row wraps instead of pushing a tab off-screen (A11Y-15)", async () => {
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
    expect(row!.wrap).toBe("wrap");
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

describe("what the screen refuses to invent", () => {
  test("retention is reported as the service's, not offered as a control", async () => {
    await openTab("Storage");
    const text = await visibleText(app.browser);
    expect(text).toContain("Set by the service");
    expect(text).toContain("Items stored");
  });
});

describe("a preference that changes layout", () => {
  test("preview lines re-reserves every row (INV-5)", async () => {
    await openTab("List");
    const thumb = await app.browser.$('[aria-label="Preview lines"]');
    await thumb.click();

    await app.browser.keys(["Home"]);
    await waitForText(app.browser, "1 line");

    await gotoView(app.browser, "History");
    await app.browser.waitUntil(
      async () => {
        const rows = await rowBoxes(app.browser);
        return rows.length > 0 && rows.every((row) => Math.round(row.height) === ROW_HEIGHT[1]);
      },
      { timeout: 15_000, timeoutMsg: "rows never shrank to the one-line reservation" },
    );

    await gotoView(app.browser, "Settings");
    await openTab("List");
    await (await app.browser.$('[aria-label="Preview lines"]')).click();
    await app.browser.keys(["ArrowRight"]);
    await waitForText(app.browser, "2 lines");

    await gotoView(app.browser, "History");
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
    await gotoView(app.browser, "Settings");
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

    await gotoView(app.browser, "Settings");
    await openTab("Appearance");
    const teal = await app.browser.$('[aria-label="Teal"]');
    expect(await teal.getAttribute("aria-pressed")).toBe("true");
  });
});

describe("with the service down", () => {
  test("the client-owned settings still work", async () => {
    await app.daemon.kill();
    await gotoView(app.browser, "Settings");
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
    for (const label of ["About", "Storage", "Sync"]) {
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
