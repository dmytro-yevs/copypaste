/**
 * Visual-regression tests — Settings → Display (Appearance) tab (CopyPaste-ojas.1).
 *
 * The Display tab contains the two-axis theme/accent picker (§2 STYLEGUIDE):
 * the segmented Dark/Light control and the six accent swatches.  It is the
 * highest-risk screen for design regressions: a token rename or CSS specificity
 * change silently breaks the picker rendering.
 *
 * Covers: dark×indigo, dark×rose, light×indigo, light×rose.
 */

import { test, expect } from "playwright/test";
import {
  THEME_MATRIX,
  gotoMockApp,
  applyTheme,
  navigateToView,
  clickSettingsTab,
} from "./helpers";

for (const { theme, accent } of THEME_MATRIX) {
  test(`settings appearance tab – ${theme} / ${accent}`, async ({ page }) => {
    await gotoMockApp(page);
    await navigateToView(page, "Settings");

    // Wait for Settings content to load (mock IPC resolves immediately).
    await page.waitForSelector('[role="tablist"]', { timeout: 10_000 });

    // Navigate to the Display tab (contains Theme + Accent pickers).
    await clickSettingsTab(page, "Display");

    // Wait for the Display tab panel to be visible.
    await page.waitForSelector('[role="tabpanel"]', { timeout: 5_000 });

    await applyTheme(page, theme, accent);

    await expect(page).toHaveScreenshot(
      `settings-appearance-${theme}-${accent}.png`,
    );
  });
}
