/**
 * Visual-regression tests — History view (CopyPaste-ojas.1).
 *
 * Covers: dark×indigo, dark×rose, light×indigo, light×rose.
 * The mock IPC returns 14 fixture clipboard items (text, URL, image, file,
 * sensitive) so the snapshot captures a representative populated list.
 */

import { test, expect } from "playwright/test";
import {
  THEME_MATRIX,
  gotoMockApp,
  applyTheme,
  navigateToView,
} from "./helpers";

for (const { theme, accent } of THEME_MATRIX) {
  test(`history view – ${theme} / ${accent}`, async ({ page }) => {
    await gotoMockApp(page);

    // History is the default view, but navigate explicitly to be explicit.
    await navigateToView(page, "History");

    // Wait for at least one history row to appear (fixture returns 14 items).
    await page.waitForSelector('[data-testid="view-transition"]', {
      timeout: 10_000,
    });

    await applyTheme(page, theme, accent);

    // Snapshot the full viewport.
    await expect(page).toHaveScreenshot(`history-${theme}-${accent}.png`);
  });
}
