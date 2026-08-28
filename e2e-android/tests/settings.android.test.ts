/**
 * Preferences on Android: adaptive section navigation, a display-only history
 * limit, and a product theme that has to survive a reload.
 *
 * The persistence half is a genuinely different mechanism here. The browser
 * layer round-trips the daemon's own settings over a socket; Android has no
 * daemon, so the app's preferences go to the Tauri store plugin
 * (`preferences.json`) and the service-shaped ones to the in-process core
 * (ADR-0003). Neither path exists on the other layer.
 *
 * Below the expanded width boundary Preferences is a category menu plus one
 * detail at a time (DMY-154 / A11Y-15); wider windows use a tablist and panel.
 * The harness follows either shape while holding the same section contract.
 */
import { afterAll, describe, expect, test } from "vitest";

import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { accessibleSurface, expectNoFilesystemPath } from "../src/harness/leaks.js";
import {
  addItems,
  cleanUpItems,
  missingFixtureIds,
  storedItems,
} from "../src/harness/bridge.js";
import { fixtureMarker } from "../src/harness/fixtures.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import {
  ensureSettingsNavigation,
  openSettingsSection,
  settingsPanel,
  settingsSectionGeometry,
  settingsSectionLabels,
  settingsSliderIndex,
} from "../src/harness/settings.js";
import {
  filterHistoryTo,
  gotoView,
  resetHistoryFilters,
  reloadHistoryWith,
  scrollListToTop,
  tapElement,
  visibleText,
  waitFor,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

const SECTIONS = [
  "Appearance",
  "Clipboard behavior",
  "Privacy & retention",
  "Device sync",
  "Cloud sync",
  "Storage & history",
  "Diagnostics",
  "Runtime events",
  "About",
] as const;

let app: AndroidApp;
let seeded: string[] = [];
let marker = "";
let originalHistoryDisplayIndex: number | null = null;
let originalProductTheme: string | null = null;

async function historyDisplayIndex(): Promise<number> {
  const state = await app.withPage((page) =>
    page.evaluate(() => {
      const slider = document.querySelector('[aria-label="History display limit"]');
      const row = slider?.closest("[data-settings-search-target]");
      return {
        exists: slider !== null,
        index: slider?.getAttribute("aria-valuenow") ?? null,
        min: slider?.getAttribute("aria-valuemin") ?? null,
        max: slider?.getAttribute("aria-valuemax") ?? null,
        output: row?.querySelector("output")?.textContent?.trim() ?? null,
      };
    }),
  );
  return settingsSliderIndex(state);
}

async function setHistoryDisplayLimit(
  index: number,
  rendered?: string,
): Promise<void> {
  await gotoView(app, "Settings");
  await openSettingsSection(app, "Clipboard behavior");
  await app.withPage(async (page) => {
    await page.click('[aria-label="History display limit"]');
    await page.keyboard.press("Home");
    for (let step = 0; step < index; step++) await page.keyboard.press("ArrowRight");
  });
  let observed: { index: string | null; output: string | null } | null = null;
  await waitFor(
    async () => {
      observed = await app.withPage((page) =>
        page.evaluate(() => {
          const slider = document.querySelector('[aria-label="History display limit"]');
          const row = slider?.closest("[data-settings-search-target]");
          return {
            index: slider?.getAttribute("aria-valuenow") ?? null,
            output: row?.querySelector("output")?.textContent?.trim() ?? null,
          };
        }),
      );
      return observed.index === String(index) &&
        (rendered === undefined || observed.output === rendered);
    },
    () =>
      `the history display limit never reached index ${index}` +
      `${rendered === undefined ? "" : ` with output ${JSON.stringify(rendered)}`}: ` +
      JSON.stringify(observed),
  );
}

async function restoreHistoryDisplayLimit(): Promise<void> {
  if (originalHistoryDisplayIndex === null) return;
  await setHistoryDisplayLimit(originalHistoryDisplayIndex);
  originalHistoryDisplayIndex = null;
}

async function restoreProductTheme(): Promise<void> {
  if (originalProductTheme === null) return;
  const theme = originalProductTheme;
  await gotoView(app, "Settings");
  await openSettingsSection(app, "Appearance");
  await tapElement(app, `[data-product-theme="${theme}"]`);
  await waitFor(
    async () =>
      (await app.withPage((page) =>
        page.evaluate(() => document.documentElement.dataset.theme ?? ""),
      )) === theme,
    `the original product theme ${JSON.stringify(theme)} was not restored`,
  );
  originalProductTheme = null;
}

async function attemptSettingsRestore(
  name: string,
  restore: () => Promise<void>,
): Promise<void> {
  try {
    await restore();
  } catch (error) {
    console.warn(`settings cleanup could not restore ${name}: ${String(error)}`);
  }
}

async function restoreAfterTest(
  name: string,
  restore: () => Promise<void>,
  primaryFailure: unknown,
): Promise<void> {
  try {
    await restore();
  } catch (error) {
    if (primaryFailure === undefined) throw error;
    console.warn(
      `settings cleanup also failed to restore ${name}: ${String(error)}`,
    );
  }
}

beforeAllWithEvidence("settings", async () => {
  app = await attachToApp();
  await gotoView(app, "Library");
  marker = fixtureMarker("settings");
  seeded = await addItems(
    app,
    Array.from({ length: 101 }, (_, index) => `${marker} fixture ${index}`),
  );
  await reloadHistoryWith(app, `${marker} fixture 100`);
  await filterHistoryTo(app, marker, marker);
  await waitForRows(app, 2);
  // The previous file may have left the list scrolled: a virtualised list
  // renders a window, so a just-seeded row at the top is not in the document
  // until the viewport is there.
  await scrollListToTop(app);
  await gotoView(app, "Settings");
  await waitFor(
    async () => (await settingsSectionLabels(app)).length > 0,
    "the Preferences screen never rendered its section navigation",
  );
}, 300_000);

afterAll(async () => {
  await attemptSettingsRestore("History display limit", restoreHistoryDisplayLimit);
  await attemptSettingsRestore("product theme", restoreProductTheme);
  await gotoView(app, "Library").catch(() => undefined);
  await resetHistoryFilters(app).catch(() => undefined);
  await cleanUpItems(app, seeded);
  await app?.detach();
});

describe("the section index", () => {
  test("Android shows its own set, and every one opens onto a pane with a real box", async () => {
    const labels = await settingsSectionLabels(app);
    expect(labels).toEqual(SECTIONS);

    for (const label of labels) {
      await openSettingsSection(app, label);
      // Waited for, not sampled: Diagnostics fills in from a command and was
      // measured mid-flight at exactly its heading.
      let pane: Awaited<ReturnType<typeof settingsPanel>> = null;
      await waitFor(
        async () => {
          pane = await settingsPanel(app, label);
          return pane !== null && pane.height > 20 && pane.width > 100 && pane.text.length > 20;
        },
        () => `the ${label} pane never laid out with content: ${JSON.stringify(pane)}`,
        20_000,
      );
    }
    await ensureSettingsNavigation(app);
  }, 120_000);

  /**
   * A11Y-15: the nine-item strip was replaced by this index because overflowing
   * labels stole neighbouring taps. Every index row must still own a tap at
   * its own centre — `elementFromPoint`, not only its box.
   */
  test("scrolls every section into reach without clipping its tap target", async () => {
    for (const label of SECTIONS) {
      const row = await settingsSectionGeometry(app, label);
      expect(row.width, label).toBeGreaterThan(0);
      expect(row.height, label).toBeGreaterThanOrEqual(44);
      expect(row.clipped, `${label} does not fit its own box`).toBeLessThanOrEqual(1);
      expect(row.documentOverflow, label).toBeLessThanOrEqual(1);
      expect(row.centerHit, `a tap on the middle of ${label} lands elsewhere`).toBe(true);
    }
  });

  test("names no filesystem path on any pane (INV-20 / AT-24)", async () => {
    for (const label of await settingsSectionLabels(app)) {
      await openSettingsSection(app, label);
      expectNoFilesystemPath(await accessibleSurface(app));
    }
    await ensureSettingsNavigation(app);
  }, 120_000);
});

describe("a preference that limits only the visible list", () => {
  test("caps rendering, never deletes, and survives a WebView reload", async () => {
    await gotoView(app, "Settings");
    await openSettingsSection(app, "Clipboard behavior");
    originalHistoryDisplayIndex = await historyDisplayIndex();
    let primaryFailure: unknown;
    try {
      await setHistoryDisplayLimit(0, "100");
      await gotoView(app, "Library");
      await waitForText(app, "Showing first 100 of 101 results");
      expect(missingFixtureIds(await storedItems(app), seeded)).toEqual([]);

      await app.withPage((page) => page.reload({ waitUntil: "domcontentloaded" }));
      await waitFor(
        async () =>
          app.withPage((page) =>
            page.evaluate(() => document.querySelectorAll("nav").length > 0),
          ),
        "the WebView never came back after the display-limit reload",
        60_000,
      );
      await gotoView(app, "Settings");
      await openSettingsSection(app, "Clipboard behavior");
      expect(await historyDisplayIndex()).toBe(0);
    } catch (error) {
      primaryFailure = error;
      throw error;
    } finally {
      await restoreAfterTest(
        "History display limit",
        restoreHistoryDisplayLimit,
        primaryFailure,
      );
    }
  }, 120_000);
});

describe("appearance", () => {
  /**
   * INV-32 / AT-49, through the Tauri store plugin: the value has
   * to be written to `preferences.json` and read back by the bootstrap before
   * the reloaded document paints.
  */
  test("survives a reload of the WebView", async () => {
    await openSettingsSection(app, "Appearance");
    const initialProductTheme = await app.withPage((page) =>
      page.evaluate(() => document.documentElement.dataset.theme ?? ""),
    );
    expect(["midnight", "aurora", "ember", "graphite"]).toContain(initialProductTheme);
    originalProductTheme = initialProductTheme;
    const selected = initialProductTheme === "aurora" ? "ember" : "aurora";
    let primaryFailure: unknown;
    try {
      await tapElement(app, `[data-product-theme="${selected}"]`);
      await waitFor(
        async () =>
          (await app.withPage((page) =>
            page.evaluate(() => document.documentElement.dataset.theme ?? ""),
          )) === selected,
        "the product theme never reached <html>",
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
          page.evaluate(() => document.documentElement.dataset.theme),
        ),
      ).toBe(selected);

      await gotoView(app, "Settings");
      await openSettingsSection(app, "Appearance");
      const pressed = await app.withPage((page) =>
        page.evaluate(
          (theme) =>
            document
              .querySelector(`[data-product-theme="${theme}"]`)
              ?.getAttribute("aria-pressed") ?? null,
          selected,
        ),
      );
      expect(pressed).toBe("true");
    } catch (error) {
      primaryFailure = error;
      throw error;
    } finally {
      await restoreAfterTest(
        "product theme",
        restoreProductTheme,
        primaryFailure,
      );
    }
  }, 180_000);
});

describe("the service-shaped settings", () => {
  test("Background capture reports its state in words a user can act on", async () => {
    await openSettingsSection(app, "Clipboard behavior");
    const text = await visibleText(app);
    expect(text).toContain("Background capture");
    expectNoFilesystemPath(await accessibleSurface(app));
  });
});
