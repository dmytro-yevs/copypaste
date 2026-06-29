/**
 * Visual-regression tests — Devices view & DeviceCard (CopyPaste-ojas.1).
 *
 * The mock IPC returns 2 paired devices (MacBook Pro + iPhone) so the device
 * card component is exercised in every snapshot.
 *
 * Covers: dark×indigo, dark×rose, light×indigo, light×rose.
 */

import { test, expect } from "playwright/test";
import {
  THEME_MATRIX,
  gotoMockApp,
  applyTheme,
  navigateToView,
} from "./helpers";

for (const { theme, accent } of THEME_MATRIX) {
  test(`devices view – ${theme} / ${accent}`, async ({ page }) => {
    await gotoMockApp(page);
    await navigateToView(page, "Devices");

    // Wait for the view-transition wrapper to confirm the panel has mounted.
    await page.waitForSelector('[data-testid="view-transition"]', {
      timeout: 10_000,
    });

    await applyTheme(page, theme, accent);

    await expect(page).toHaveScreenshot(`devices-${theme}-${accent}.png`);
  });
}
