import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Standalone test config so the Tauri/Vite build config stays untouched.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
