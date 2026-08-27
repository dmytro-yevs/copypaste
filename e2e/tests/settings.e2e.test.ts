/**
 * Preferences: the sections, and the controls that leave the screen.
 *
 * The app's own preferences are round-tripped because they change what the
 * user sees — the history display limit must not delete items, and appearance
 * has to survive a reload (INV-22). The daemon's are round-tripped through the
 * daemon, since the screen writing them is the only thing that can be wrong.
 */
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { accessibleSurface, expectNoFilesystemPath } from "../src/harness/leaks.js";
import { retryResponsiveInteraction } from "../src/harness/responsive-interaction.js";
import {
  gotoView,
  visibleText,
  waitForRows,
  waitForText,
} from "../src/harness/ui.js";

const SHARED_SECTIONS = [
  "Appearance",
  "Clipboard behavior",
  "Privacy & retention",
  "Device sync",
  "Cloud sync",
  "Storage & history",
  "Diagnostics",
  "Runtime events",
  "About",
];
const TABS =
  process.platform === "win32"
    ? [...SHARED_SECTIONS.slice(0, 3), "Shortcuts", ...SHARED_SECTIONS.slice(3)]
    : SHARED_SECTIONS;

let app: App;

const config = () =>
  app.daemon.json<{ config: { retention_days: number } }>(["config", "show"]);

beforeAll(async () => {
  app = await startApp({
    seed: Array.from({ length: 101 }, (_, index) => `a settings fixture ${index}`),
  });
  await waitForRows(app.browser, 2);
  await gotoView(app.browser, "Preferences");
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

type SettingsNavigationControl =
  | { kind: "navigation"; element: WebdriverIO.Element }
  | { kind: "back"; element: WebdriverIO.Element };

async function displayed(selector: string) {
  for (const element of await app.browser.$$(selector)) {
    if (await element.isDisplayed()) return element;
  }
  return null;
}

async function withSettingsNavigation(
  action: (navigation: WebdriverIO.Element) => Promise<boolean>,
  timeoutMsg: string,
): Promise<void> {
  await retryResponsiveInteraction<SettingsNavigationControl>({
    acquire: async () => {
      const navigation = await displayed('[aria-label="Preference sections"]');
      if (navigation) return { kind: "navigation", element: navigation };
      const back = await displayed('button[aria-label="Back to Preferences"]');
      return back ? { kind: "back", element: back } : null;
    },
    interact: async (current) => {
      if (current.kind === "navigation") return action(current.element);
      expect(await current.element.isEnabled()).toBe(true);
      await current.element.click();
      return false;
    },
    waitUntil: async (attempt) => {
      await app.browser.waitUntil(attempt, { timeout: 15_000, timeoutMsg });
    },
  });
}

async function openSection(label: string): Promise<void> {
  await withSettingsNavigation(async (navigation) => {
    const tab = await navigation.$(`[role="tab"]=${label}`);
    const trigger = (await tab.isDisplayed())
      ? tab
      : await navigation.$(`.//button[.//strong[normalize-space(.)="${label}"]]`);
    if (!(await trigger.isClickable())) return false;
    await trigger.click();
    return true;
  }, `the ${label} section control never became interactable`);
  await app.browser.waitUntil(
    async () => {
      return (await app.browser.execute(function (sectionLabel: string) {
        const visible = (element: Element) =>
          element.getClientRects().length > 0;
        const selectedDesktopTab = Array.from(
          document.querySelectorAll<HTMLElement>('[role="tab"]'),
        ).find(
          (tab) =>
            tab.textContent?.trim() === sectionLabel && visible(tab),
        );
        const opened = Array.from(
          document.querySelectorAll<HTMLElement>(
            'section[aria-label], [role="tabpanel"][aria-label]',
          ),
        ).some(
          (section) =>
            section.getAttribute("aria-label") === sectionLabel &&
            visible(section),
        );
        return (
          opened &&
          (!selectedDesktopTab ||
            selectedDesktopTab.getAttribute("aria-selected") === "true")
        );
      }, label)) as boolean;
    },
    { timeout: 10_000, timeoutMsg: `the ${label} section never opened` },
  );
  // DMY-138: the section is selected but its content may still be loading
  // asynchronously. Wait for aria-busy to clear so assertions see real content.
  await app.browser.waitUntil(
    async () =>
      (await app.browser.execute(function (sectionLabel: string) {
        const section = Array.from(
          document.querySelectorAll<HTMLElement>(
            'section[aria-label], [role="tabpanel"][aria-label]',
          ),
        ).find(
          (candidate) =>
            candidate.getAttribute("aria-label") === sectionLabel &&
            candidate.getClientRects().length > 0,
        );
        return (
          section !== undefined && section.querySelector("[aria-busy]") === null
        );
      }, label)) === true,
    {
      timeout: 30_000,
      timeoutMsg: `the ${label} section never finished loading`,
    },
  );
}

/** The selected desktop tabpanel or compact detail, as WebKit laid it out. */
async function panel(label: string) {
  return (await app.browser.execute(function (sectionLabel: string) {
    const el = Array.from(
      document.querySelectorAll<HTMLElement>(
        'section[aria-label], [role="tabpanel"][aria-label]',
      ),
    ).find(
      (candidate) =>
        candidate.getAttribute("aria-label") === sectionLabel &&
        candidate.getClientRects().length > 0,
    );
    if (!el) return null;
    const rect = el.getBoundingClientRect();
    return {
      width: rect.width,
      height: rect.height,
      text: el.innerText.trim(),
    };
  }, label)) as { width: number; height: number; text: string } | null;
}

describe("the sections", () => {
  test("every one opens onto content with a real box", async () => {
    for (const label of TABS) {
      await openSection(label);
      const pane = await panel(label);
      expect(pane, label).not.toBeNull();
      expect(pane!.height, label).toBeGreaterThan(20);
      expect(pane!.width, label).toBeGreaterThan(100);
      expect(pane!.text.length, label).toBeGreaterThan(20);
    }
  });

  test("the navigation keeps every section reachable without horizontal overflow", async () => {
    let row: {
      overflow: number;
      left: number;
      right: number;
      tabs: Array<{
        left: number;
        right: number;
        width: number;
        text: string;
      }>;
    } | null = null;
    await withSettingsNavigation(async () => {
      row = (await app.browser.execute(function () {
        const list = Array.from(
          document.querySelectorAll<HTMLElement>(
            '[aria-label="Preference sections"]',
          ),
        ).find((candidate) => candidate.getClientRects().length > 0);
        if (!list) return null;
        const box = list.getBoundingClientRect();
        const tabs = Array.prototype.map.call(
          list.querySelectorAll("button"),
          function (node) {
            const rect = (node as HTMLElement).getBoundingClientRect();
            return {
              left: rect.left,
              right: rect.right,
              width: rect.width,
              text:
                node.querySelector("strong")?.textContent?.trim() ??
                (node as HTMLElement).innerText.trim(),
            };
          },
        );
        return {
          overflow: list.scrollWidth - list.clientWidth,
          left: box.left,
          right: box.right,
          tabs,
        };
      })) as typeof row;
      return row !== null;
    }, "the settings navigation never became measurable");

    expect(row).not.toBeNull();
    expect(row!.overflow).toBeLessThanOrEqual(1);
    expect(row!.tabs).toHaveLength(TABS.length);
    expect(row!.tabs.map((tab) => tab.text)).toEqual(TABS);
    for (const tab of row!.tabs) {
      expect(tab.width, tab.text).toBeGreaterThan(0);
      expect(tab.left, tab.text).toBeGreaterThanOrEqual(row!.left - 1);
      expect(tab.right, tab.text).toBeLessThanOrEqual(row!.right + 1);
    }
  });

  test("names no filesystem path in any section (INV-20)", async () => {
    for (const label of TABS) {
      await openSection(label);
      expectNoFilesystemPath(
        await accessibleSurface(app.browser),
        app.daemon.dataHome,
      );
    }
  });
});

describe("the service's own settings", () => {
  test("a value chosen on the screen reaches and persists in the daemon", async () => {
    await openSection("Privacy & retention");
    const select = await app.browser.$(
      '[role="combobox"][aria-label^="Drop items older than"]',
    );
    await select.waitForDisplayed({ timeout: 10_000 });
    await select.click();
    await app.browser.$('[role="option"][data-value="30"]').click();

    // Read it back out of the daemon, not out of the control that wrote it.
    await app.browser.waitUntil(
      async () => (await config()).config.retention_days === 30,
      { timeout: 10_000, interval: 250, timeoutMsg: "the daemon never took the value" },
    );
  });

  test("storage reports what the service holds", async () => {
    await openSection("Storage & history");
    expect(await visibleText(app.browser)).toContain("Items stored");
  });
});

describe("a preference that changes the visible list", () => {
  test("history display limit caps the list without deleting history", async () => {
    await openSection("Clipboard behavior");
    const limit = await app.browser.$(
      '[role="slider"][aria-label="History display limit"]',
    );
    await limit.click();
    await app.browser.keys(["Home"]);
    await waitForText(app.browser, "100");

    await gotoView(app.browser, "Library");
    await waitForText(app.browser, "Showing first 100 of 101 results");
    expect(await app.daemon.items()).toHaveLength(101);
  });
});

describe("appearance", () => {
  test("survives a reload of the window (INV-32)", async () => {
    await gotoView(app.browser, "Preferences");
    await openSection("Appearance");
    await (await app.browser.$('[data-product-theme="aurora"]')).click();

    await app.browser.waitUntil(
      async () =>
        (await app.browser.execute(
          () => document.documentElement.dataset.theme ?? "",
        )) === "aurora",
      { timeout: 10_000, timeoutMsg: "the theme never reached <html>" },
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
      await app.browser.execute(() => document.documentElement.dataset.theme),
    ).toBe("aurora");

    await gotoView(app.browser, "Preferences");
    await openSection("Appearance");
    const aurora = await app.browser.$('[data-product-theme="aurora"]');
    expect(await aurora.getAttribute("aria-pressed")).toBe("true");
  });
});

describe("with the service down", () => {
  test("the client-owned settings still work", async () => {
    await app.daemon.kill();
    await gotoView(app.browser, "Preferences");
    await openSection("Appearance");

    // Nothing on this pane needs the daemon, so it must still be operable.
    await (await app.browser.$('[data-product-theme="ember"]')).click();
    await app.browser.waitUntil(
      async () =>
        (await app.browser.execute(
          () => document.documentElement.dataset.theme ?? "",
        )) === "ember",
      { timeout: 10_000, timeoutMsg: "Settings stopped working when the service did" },
    );
  });

  test("the panes that do need it say so without naming a path", async () => {
    for (const label of ["About", "Storage & history", "Cloud sync"]) {
      await openSection(label);
      const pane = await panel(label);
      expect(pane!.text.length, label).toBeGreaterThan(20);
      expectNoFilesystemPath(
        await accessibleSurface(app.browser),
        app.daemon.dataHome,
      );
    }
  });
});
