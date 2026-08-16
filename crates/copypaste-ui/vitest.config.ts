import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// The app cannot be exercised end-to-end here — WebKit does not execute under
// headless Xvfb without a GPU — so these run in jsdom and need no display.
//
// `cpus - 1` workers over-subscribe a high-core machine: on a fresh checkout
// every worker re-transforms the full module graph at once, and the group
// starves one async-util wait past the 5000ms budget — a different test on
// every run. The cap bounds the pool at 8; the 15000ms budget still dwarfs the
// 2500ms async waits (setup.ts), so a broken screen fails named, not anonymous.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  test: {
    environment: "jsdom",
    environmentOptions: {
      jsdom: { url: "http://localhost" },
    },
    globals: false,
    include: ["src/**/*.test.{ts,tsx}"],
    // jsdom has no layout and no ResizeObserver; see the file for what is
    // faked and why it has to be.
    setupFiles: ["./src/test/setup.ts"],
    maxWorkers: 8,
    testTimeout: 15000,
  },
});
