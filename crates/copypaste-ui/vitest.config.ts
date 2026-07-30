import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// The app cannot be exercised end-to-end here — WebKit does not execute under
// headless Xvfb without a GPU — so these run in jsdom and need no display.
export default defineConfig({
  plugins: [react()],
  test: { environment: "jsdom", globals: false, include: ["src/**/*.test.{ts,tsx}"] },
});
