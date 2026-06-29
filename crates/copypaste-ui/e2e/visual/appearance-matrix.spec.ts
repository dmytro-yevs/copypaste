/**
 * Visual-regression tests — full {dark,light} × 6-accent appearance matrix (CopyPaste-cv5q).
 *
 * The Settings → Display tab is chosen because it contains the accent-swatch
 * picker and the dark/light segmented control: the highest-signal screen for
 * verifying that every accent token renders correctly in both themes.
 *
 * Accents: indigo, blue, teal, green, amber, rose (all 6 supported values).
 * Themes:  dark, light.
 * Cells:   12 total.
 */

import { test, expect } from "playwright/test";
import {
  gotoMockApp,
  applyTheme,
  navigateToView,
  clickSettingsTab,
} from "./helpers";

type Theme = "dark" | "light";
type Accent = "indigo" | "blue" | "teal" | "green" | "amber" | "rose";

const ALL_ACCENTS: Accent[] = ["indigo", "blue", "teal", "green", "amber", "rose"];
const THEMES: Theme[] = ["dark", "light"];

// Build the full 12-cell matrix explicitly so each cell is a named test.
const FULL_MATRIX: { theme: Theme; accent: Accent }[] = THEMES.flatMap(
  (theme) => ALL_ACCENTS.map((accent) => ({ theme, accent })),
);

for (const { theme, accent } of FULL_MATRIX) {
  test(`appearance matrix – ${theme} / ${accent}`, async ({ page }) => {
    await gotoMockApp(page);
    await navigateToView(page, "Settings");

    // Wait for the Settings tablist to confirm the panel has mounted.
    await page.waitForSelector('[role="tablist"]', { timeout: 10_000 });

    // Navigate to the Display tab (contains Theme + Accent pickers / swatches).
    await clickSettingsTab(page, "Display");

    // Wait for the Display tab panel to be visible.
    await page.waitForSelector('[role="tabpanel"]', { timeout: 5_000 });

    // Apply theme + accent after the panel is ready so our override wins.
    await applyTheme(page, theme, accent);

    // Snapshot the full viewport — captures accent swatches, selected state,
    // and dark/light segmented control in a single frame.
    await expect(page).toHaveScreenshot(
      `appearance-matrix-${theme}-${accent}.png`,
    );
  });
}
