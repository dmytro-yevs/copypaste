/**
 * Shared helpers for Playwright visual-regression tests (CopyPaste-ojas.1).
 *
 * Theme matrix tested: dark × indigo, dark × rose, light × indigo, light × rose.
 * Accent "rose" is the second accent required by the spec ("indigo + at least one other").
 */

import type { Page } from "playwright/test";

export type Theme = "dark" | "light";
export type Accent = "indigo" | "rose";

// All (theme, accent) combinations exercised in every screen test.
export const THEME_MATRIX: { theme: Theme; accent: Accent }[] = [
  { theme: "dark", accent: "indigo" },
  { theme: "dark", accent: "rose" },
  { theme: "light", accent: "indigo" },
  { theme: "light", accent: "rose" },
];

/**
 * Navigate to the mock-enabled main window and wait for the app shell to be
 * ready (sidebar buttons visible, IPC data resolved).
 *
 * The mock IPC returns fixture data synchronously, so "networkidle" is
 * reached quickly. An additional wait for the first sidebar nav button
 * ensures React has finished rendering the shell.
 */
export async function gotoMockApp(page: Page): Promise<void> {
  await page.goto("/?mock=1");
  // Wait for any sidebar nav button to confirm the shell is rendered.
  await page.waitForSelector("nav button", { timeout: 15_000 });
  // Let any micro-task queue flush (store initialisation, useEffects).
  await page.waitForTimeout(200);
}

/**
 * Navigate to the mock-enabled quick-paste popup and wait for it to render.
 */
export async function gotoMockPopup(page: Page): Promise<void> {
  await page.goto("/popup.html?mock=1");
  // Wait for the popup root div to have children.
  await page.waitForSelector("#popup-root > *", { timeout: 15_000 });
  await page.waitForTimeout(200);
}

/**
 * Set data-theme and data-accent directly on <html> so the CSS token layer
 * switches immediately without touching the Zustand store.
 *
 * Called AFTER gotoMockApp / gotoMockPopup so App.tsx's initial useEffect
 * (which syncs the store prefs to the DOM) has already run. Our override then
 * wins, and App.tsx won't re-run the effect (store state hasn't changed).
 */
export async function applyTheme(
  page: Page,
  theme: Theme,
  accent: Accent,
): Promise<void> {
  await page.evaluate(
    ({ theme, accent }: { theme: string; accent: string }) => {
      const el = document.documentElement;
      el.dataset.theme = theme;
      el.dataset.accent = accent;
    },
    { theme, accent },
  );
  // Give the CSS cascade one animation frame to apply.
  await page.waitForTimeout(50);
}

/**
 * Click the sidebar navigation button by its visible label and wait for the
 * view transition to settle (animations disabled → fast).
 */
export async function navigateToView(
  page: Page,
  label: string,
): Promise<void> {
  await page.getByRole("button", { name: label, exact: true }).click();
  // Short settle time even with reduced-motion enabled (view components mount).
  await page.waitForTimeout(300);
}

/**
 * Click a settings tab by its visible label and wait for the panel to render.
 */
export async function clickSettingsTab(
  page: Page,
  label: string,
): Promise<void> {
  await page.getByRole("tab", { name: label }).click();
  await page.waitForTimeout(150);
}
