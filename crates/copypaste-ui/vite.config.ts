import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vite";
import legacy from "@vitejs/plugin-legacy";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

import { dropOrphanChunks } from "./scripts/orphan-chunks.mjs";
import { LEGACY_BROWSERSLIST, MODERN_BROWSERSLIST } from "./scripts/webview-baseline.mjs";

// Tailwind v4 is a Vite plugin — no postcss.config, no tailwind.config.
// The theme comes from design/dist/css, which Style Dictionary generates.
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    // Two builds, because the matrix spans two engines: the module build for
    // API 29 and above, the nomodule build for API 24. Only an engine that
    // cannot read `type="module"` asks for the second, so the generators and
    // the core-js payload reach that one and nothing else.
    //
    // `additionalLegacyPolyfills` is deliberately unused: the chunk it builds
    // is not lowered, and the DOM and Intl polyfills it would carry ship modern
    // syntax of their own — which made the chunk that exists to rescue
    // Chromium 53 the one thing on the page it could not parse.
    // `src/legacyPolyfills.ts` goes through the app graph instead, behind
    // `import.meta.env.LEGACY` in `main.tsx`.
    legacy({
      targets: [LEGACY_BROWSERSLIST],
      modernTargets: [MODERN_BROWSERSLIST],
      // The module build's floor is API 29, already above every builtin
      // core-js would add.
      modernPolyfills: false,
    }),
    // The branch in `main.tsx` is dropped from the module build too late to
    // drop the chunk with it; `scripts/orphan-chunks.mjs` says why that is and
    // why removal waits for the output that does not name it. The nomodule
    // output loads its copy, so only the modern one loses anything.
    dropOrphanChunks(/^legacyPolyfills-/),
  ],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // The Android WebView is the one pinned into the emulator's system image, not
  // a Play-updated one, so both engines are measured rather than assumed: API 29
  // is Chromium 74 and API 24 is Chromium 53 (`scripts/webview-baseline.mjs`).
  // The plugin owns the lowering for both builds, and
  // `scripts/check-webview-baseline.mjs` holds each emitted chunk to the engine
  // that will load it.
  build: { emptyOutDir: true },
});
