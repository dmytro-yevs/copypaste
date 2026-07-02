// Dev-drive config: drives the REAL app against a RUNNING daemon via the
// /__ipc bridge (?bridge=1). Distinct from playwright.config.ts (mock, :1421).
// Requires: `cargo run -p copypaste-daemon` AND `pnpm dev` (Vite on :1420).
import { defineConfig } from "playwright/test";

export default defineConfig({
  testDir: "./e2e/dev-drive",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:1420",
    reducedMotion: "reduce",
  },
});
