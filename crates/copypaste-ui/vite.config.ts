import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tailwind v4 is a Vite plugin — no postcss.config, no tailwind.config.
// The theme comes from design/dist/css, which Style Dictionary generates.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // chrome74 is the Android WebView baseline, not a preference: `minSdk = 24`
  // and the emulator's WebView is the one pinned into its system image rather
  // than a Play-updated one, so API 29 runs Chromium 74. `es2022` left logical
  // assignment in the bundle, which that engine reads as `||` followed by `=` —
  // run 31671766432 lost every API 29 assertion to one `Unexpected token =`
  // before a line of the app ran. `scripts/check-webview-baseline.mjs` holds
  // this value and the emitted bundle to each other.
  build: { target: "chrome74", emptyOutDir: true },
});
