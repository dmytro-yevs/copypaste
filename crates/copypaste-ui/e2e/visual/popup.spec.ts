/**
 * Visual-regression tests — Quick-paste Popup (CopyPaste-ojas.1).
 *
 * The popup is a separate Vite entry point (popup.html / src/popup/main.tsx).
 * It uses its own reduced viewport (480×640) to match the native popup window
 * dimensions used by the Tauri window config.
 *
 * Covers: dark×indigo, dark×rose, light×indigo, light×rose.
 */

import { test, expect } from "playwright/test";
import { THEME_MATRIX, gotoMockPopup, applyTheme } from "./helpers";

for (const { theme, accent } of THEME_MATRIX) {
  test(`popup – ${theme} / ${accent}`, async ({ page }) => {
    // The popup renders at a narrower width than the main window.
    await page.setViewportSize({ width: 480, height: 640 });

    await gotoMockPopup(page);
    await applyTheme(page, theme, accent);

    await expect(page).toHaveScreenshot(`popup-${theme}-${accent}.png`);
  });
}
