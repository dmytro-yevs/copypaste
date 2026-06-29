/**
 * Playwright visual-regression config (CopyPaste-ojas.1).
 *
 * Tests run against the Vite dev server with ?mock=1 so all IPC calls are
 * served by the in-process fixture harness (src/lib/mockIpc.ts) and no
 * daemon or Tauri runtime is required.
 *
 * Port 1421 is used instead of 1420 (Tauri dev) to avoid colliding with an
 * active `tauri dev` session on the developer's machine.
 *
 * reducedMotion: 'reduce' sets prefers-reduced-motion:reduce in Chromium,
 * which the app CSS respects (all @keyframe rules are gated on it), giving
 * deterministic, animation-free snapshots.
 */

import { defineConfig, devices } from "playwright/test";

export default defineConfig({
  testDir: "./e2e/visual",
  snapshotDir: "./e2e/visual/__snapshots__",

  // Sequential for determinism; visual diffs from stale state are hard to debug.
  fullyParallel: false,
  retries: 0,
  workers: 1,

  reporter: [
    ["html", { open: "never", outputFolder: "playwright-report" }],
    ["list"],
  ],

  use: {
    baseURL: "http://localhost:1421",

    // prefers-reduced-motion:reduce → no CSS animations → stable pixel output.
    // The redesign explicitly honours this (§MO gate in src/styles/animations.css).
    reducedMotion: "reduce",

    // Dark colour-scheme hint; tests override data-theme / data-accent explicitly.
    colorScheme: "dark",

    // Fixed 1280×800 viewport — same canvas for every snapshot.
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,

    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },

  expect: {
    toHaveScreenshot: {
      // Up to 2 % of pixels may differ by up to 10 % brightness.
      // Covers sub-pixel antialiasing variance without masking real regressions.
      maxDiffPixelRatio: 0.02,
      threshold: 0.1,
      animations: "disabled",
    },
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  // Start the Vite dev server before running tests.
  // --port 1421 overrides vite.config.ts default of 1420.
  // Playwright tears it down automatically after the run.
  webServer: {
    command: "npx vite --port 1421",
    port: 1421,
    // Re-use a running server during local iteration; always start fresh on CI.
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
