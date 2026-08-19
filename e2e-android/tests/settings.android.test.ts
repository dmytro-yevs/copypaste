/**
 * Settings on Android: the compact section index, a preference that reaches
 * layout, and a preference that has to survive a reload.
 *
 * The persistence half is a genuinely different mechanism here. The browser
 * layer round-trips the daemon's own settings over a socket; Android has no
 * daemon, so the app's preferences go to the Tauri store plugin
 * (`preferences.json`) and the service-shaped ones to the in-process core
 * (ADR-0003). Neither path exists on the other layer.
 *
 * Below the expanded width boundary Settings is an index + one subpage at a
 * time (DMY-154 / A11Y-15), not a tab strip — so this suite measures the
 * ladder, not `[role="tab"]`. Android also shows a different set of sections
 * — no Shortcut, plus Background capture.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { accessibleSurface, expectNoFilesystemPath } from "../src/harness/leaks.js";
import { rowBoxes } from "../src/harness/list.js";
import { addItems, cleanUpItems } from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import {
  filterHistoryTo,
  gotoView,
  resetHistoryFilters,
  reloadHistoryWith,
  scrollListToTop,
  tapButton,
  visibleText,
  waitFor,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

/** `rowHeight(n)` and `TITLE_LINE_PX` from
 *  `crates/copypaste-ui/src/lib/layout.ts`, duplicated deliberately: a test
 *  that imported them could not catch them changing. This file is where the
 *  setting-to-reservation mapping is pinned. */
const ROW_HEIGHT = { 1: 67, 2: 88 } as const;
const TITLE_LINE_PX = 21;

/**
 * The reservation is a floor on Android, an exact height on desktop:
 * `HistoryList` sets `minHeight` here and `height` there, so the virtualiser
 * measures the row back and a one-line row settles at 68 rather than 67. What
 * the setting must still decide is the band — one line may not reach what two
 * lines reserve — and that every row is the same height whatever it holds.
 */
function expectReservedFor(lines: 1 | 2, heights: number[]): boolean {
  const distinct = new Set(heights.map(Math.round));
  if (heights.length === 0 || distinct.size !== 1) return false;
  const height = [...distinct][0]!;
  return height >= ROW_HEIGHT[lines] && height < ROW_HEIGHT[lines] + TITLE_LINE_PX;
}

/** The compact Settings index — not Diagnostics' own nested tab strip. */
const INDEX = 'nav[aria-label="Settings sections"]';
const INDEX_ITEM = `${INDEX} button[data-settings-index-item]`;
const SUBPAGE = 'section[aria-labelledby="settings-subpage-title"]';
const BACK = "All settings";

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";

beforeAll(async () => {
  app = await attachToApp();
  await gotoView(app, "History");
  marker = fixtureMarker("settings");
  seeded = await addItems(app, [`${marker} first fixture`, `${marker} second fixture`]);
  await reloadHistoryWith(app, `${marker} second fixture`);
  await filterHistoryTo(app, marker, marker);
  await waitForRows(app, 2);
  // The previous file may have left the list scrolled: a virtualised list
  // renders a window, so a just-seeded row at the top is not in the document
  // until the viewport is there.
  await scrollListToTop(app);
  await gotoView(app, "Settings");
  await waitFor(
    async () => (await sectionLabels()).length > 0,
    "the Settings screen never rendered its section index",
  );
}, 300_000);

afterAll(async () => {
  await gotoView(app, "History").catch(() => undefined);
  await resetHistoryFilters(app).catch(() => undefined);
  await cleanUpItems(app, seeded);
  await app?.detach();
});

async function sectionLabels(): Promise<string[]> {
  return app.withPage((page) =>
    page.evaluate(
      (selector: string) =>
        Array.from(document.querySelectorAll(selector), (node) =>
          (node as HTMLElement).textContent!.trim(),
        ),
      INDEX_ITEM,
    ),
  );
}

async function settingsLevel(): Promise<"index" | "subpage" | "neither"> {
  return app.withPage((page) =>
    page.evaluate(
      (index: string, subpage: string) => {
        if (document.querySelector(index)) return "index";
        if (document.querySelector(subpage)) return "subpage";
        return "neither";
      },
      INDEX,
      SUBPAGE,
    ),
  );
}

/**
 * After a WebView reload Settings remounts empty for a beat: neither the index
 * nor a subpage is in the document yet. Tapping Back in that window fails —
 * wait for one of the two levels, then climb if needed.
 */
async function ensureIndex(): Promise<void> {
  await waitFor(
    async () => {
      const level = await settingsLevel();
      if (level === "index") return true;
      if (level === "subpage") {
        await tapButton(app, BACK);
        return false;
      }
      return false;
    },
    "the Settings index never became available",
    60_000,
  );
}

async function openSection(label: string): Promise<void> {
  await ensureIndex();
  await tapButton(app, label, { within: INDEX });
  await waitFor(
    async () =>
      app.withPage((page) =>
        page.evaluate(
          (subpage: string, name: string) => {
            const title = document.querySelector(`${subpage} #settings-subpage-title`);
            return title?.textContent?.trim() === name;
          },
          SUBPAGE,
          label,
        ),
      ),
    `the ${label} section never opened`,
  );
}

/**
 * The open subpage, as the engine laid it out.
 *
 * Compact Settings keeps only one section mounted, so there is no hidden
 * tabpanel sibling to exclude the way the desktop strip does.
 */
async function panel() {
  return app.withPage((page) =>
    page.evaluate((selector: string) => {
      const el = document.querySelector(selector) as HTMLElement | null;
      if (!el) return null;
      const rect = el.getBoundingClientRect();
      return { width: rect.width, height: rect.height, text: el.innerText.trim() };
    }, SUBPAGE),
  );
}

describe("the section index", () => {
  test("Android shows its own set, and every one opens onto a pane with a real box", async () => {
    const labels = await sectionLabels();
    // Shortcut is desktop-only; Background capture is Android's.
    expect(labels).toContain("Background capture");
    expect(labels).toContain("Storage");
    expect(labels).not.toContain("Shortcut");

    for (const label of labels) {
      await openSection(label);
      // Waited for, not sampled: Diagnostics fills in from a command and was
      // measured mid-flight at exactly its heading.
      let pane: Awaited<ReturnType<typeof panel>> = null;
      await waitFor(
        async () => {
          pane = await panel();
          return pane !== null && pane.height > 20 && pane.width > 100 && pane.text.length > 20;
        },
        () => `the ${label} pane never laid out with content: ${JSON.stringify(pane)}`,
        20_000,
      );
    }
    await ensureIndex();
  }, 120_000);

  /**
   * A11Y-15: the nine-item strip was replaced by this index because overflowing
   * labels stole neighbouring taps. Every index row must still own a tap at
   * its own centre — `elementFromPoint`, not only its box.
   */
  test("every section row is laid out, unclipped, and answers a tap at its own centre", async () => {
    await ensureIndex();
    const strip = await app.withPage((page) =>
      page.evaluate((selector: string) => {
        const list = document.querySelector(
          'nav[aria-label="Settings sections"]',
        ) as HTMLElement | null;
        if (!list) return null;
        return {
          documentOverflow:
            document.documentElement.scrollWidth - document.documentElement.clientWidth,
          rows: Array.from(document.querySelectorAll(selector), (node) => {
            const row = node as HTMLElement;
            const rect = row.getBoundingClientRect();
            const hit = document.elementFromPoint(
              rect.x + rect.width / 2,
              rect.y + rect.height / 2,
            );
            return {
              text: row.textContent!.trim(),
              width: rect.width,
              height: rect.height,
              clipped: row.scrollWidth - row.clientWidth,
              hit:
                hit?.closest("[data-settings-index-item]")?.textContent?.trim() ?? null,
            };
          }),
        };
      }, INDEX_ITEM),
    );

    expect(strip).not.toBeNull();
    expect(strip!.documentOverflow).toBeLessThanOrEqual(1);

    for (const row of strip!.rows) {
      expect(row.width, row.text).toBeGreaterThan(0);
      expect(row.height, row.text).toBeGreaterThanOrEqual(44);
      expect(row.clipped, `${row.text} does not fit its own box`).toBeLessThanOrEqual(1);
      expect(row.hit, `a tap on the middle of ${row.text} lands elsewhere`).toBe(row.text);
    }
  });

  test("names no filesystem path on any pane (INV-12)", async () => {
    for (const label of await sectionLabels()) {
      await openSection(label);
      expectNoFilesystemPath(await accessibleSurface(app));
    }
    await ensureIndex();
  }, 120_000);
});

describe("a preference that changes layout", () => {
  /** The slider is a `role="slider"`; Home and ArrowRight are how a keyboard
   *  drives it, and the WebView taking them at all is part of the claim. */
  async function setPreviewLines(
    key: "Home" | "ArrowRight",
    lines: 1 | 2,
    rendered: string,
  ): Promise<void> {
    await openSection("List");
    await app.withPage(async (page) => {
      await page.click('[aria-label="Preview lines"]');
      await page.keyboard.press(key);
    });
    await waitFor(
      async () =>
        (await app.withPage((page) =>
          page.evaluate(
            () =>
              document.querySelector('[aria-label="Preview lines"]')?.getAttribute("aria-valuenow") ??
              "",
          ),
        )) === String(lines),
      `the preview-lines slider never reached ${lines}`,
    );
    // The words beside it, not only the value: "1 line" and "2 lines" are what
    // the user reads back, and they are rendered text rather than a key.
    await waitForText(app, rendered);
  }

  test("preview lines re-reserves every row (INV-5)", async () => {
    await setPreviewLines("Home", 1, "1 line");
    await gotoView(app, "History");
    await waitFor(
      async () => expectReservedFor(1, (await rowBoxes(app)).map((row) => row.height)),
      "rows never shrank to the one-line reservation",
      20_000,
    );

    await gotoView(app, "Settings");
    await setPreviewLines("ArrowRight", 2, "2 lines");
    await gotoView(app, "History");
    await waitFor(
      async () => expectReservedFor(2, (await rowBoxes(app)).map((row) => row.height)),
      "rows never returned to the two-line reservation",
      20_000,
    );
    await gotoView(app, "Settings");
  }, 120_000);
});

describe("appearance", () => {
  /**
   * INV-22, through the Tauri store plugin rather than a daemon: the value has
   * to be written to `preferences.json` and read back by the bootstrap before
   * the reloaded document paints.
   */
  test("survives a reload of the WebView", async () => {
    await openSection("Appearance");
    await tapButton(app, "Teal");
    await waitFor(
      async () =>
        (await app.withPage((page) =>
          page.evaluate(() => document.documentElement.dataset.accent ?? ""),
        )) === "teal",
      "the accent never reached <html>",
    );

    await app.withPage((page) => page.evaluate(() => location.reload()));
    await waitFor(
      async () =>
        (await app.withPage((page) =>
          page.evaluate(() => document.querySelectorAll("nav").length),
        )) > 0,
      "the WebView never came back after a reload",
      60_000,
    );

    expect(
      await app.withPage((page) =>
        page.evaluate(() => document.documentElement.dataset.accent),
      ),
    ).toBe("teal");

    await gotoView(app, "Settings");
    await openSection("Appearance");
    const pressed = await app.withPage((page) =>
      page.evaluate(
        () => document.querySelector('[aria-label="Teal"]')?.getAttribute("aria-pressed") ?? null,
      ),
    );
    expect(pressed).toBe("true");

    // The device keeps its preferences between runs, so the next run starts
    // where this one left off unless it is put back.
    await tapButton(app, "System accent");
  }, 180_000);
});

describe("the service-shaped settings", () => {
  test("Background capture reports its state in words a user can act on", async () => {
    await openSection("Background capture");
    const text = await visibleText(app);
    expect(text).toContain("Background capture");
    expectNoFilesystemPath(await accessibleSurface(app));
  });
});
