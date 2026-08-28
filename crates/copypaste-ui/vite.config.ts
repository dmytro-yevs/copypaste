import { readFileSync } from "node:fs";
import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vite";
import legacy from "@vitejs/plugin-legacy";
import react from "@vitejs/plugin-react";
import postcssCascadeLayers from "@csstools/postcss-cascade-layers";
import postcssCustomMedia from "postcss-custom-media";
import postcssGlobalData from "@csstools/postcss-global-data";
import postcssSimpleVars from "postcss-simple-vars";
import flexGapPolyfill from "flex-gap-polyfill";

import { dropOrphanChunks } from "./scripts/orphan-chunks.mjs";
import { RESPONSIVE_POSTCSS_VARIABLES } from "./src/lib/layoutBreakpoints.ts";
import {
  LEGACY_BROWSERSLIST,
  LEGACY_CSS_TARGET,
  MODERN_BROWSERSLIST,
} from "./scripts/webview-baseline.mjs";

const androidBuild = process.env.VITE_ANDROID_BUILD === "1";
const packageManifest = JSON.parse(
  readFileSync(fileURLToPath(new URL("./package.json", import.meta.url)), "utf8"),
) as { version: string };
function quickPasteAndroidGate() {
  return {
    name: "copypaste:android-quick-paste-gate",
    resolveId(id: string) {
      if (androidBuild && id.endsWith("/desktopQuickPaste")) {
        return "\0copypaste-android-desktop-quick-paste";
      }
    },
    load(id: string) {
      if (id === "\0copypaste-android-desktop-quick-paste") {
        return "export async function loadQuickPaste() { throw new Error('Quick Paste is desktop-only'); }";
      }
    },
    generateBundle() {
      if (!androidBuild) return;
      for (const id of this.getModuleIds()) {
        if (id.includes("/features/quick-paste/")) {
          this.error(`Android bundle contains desktop-only Quick Paste module: ${id}`);
        }
      }
    },
  };
}

export default defineConfig({
  base: "./",
  define: {
    __COPYPASTE_APP_VERSION__: JSON.stringify(packageManifest.version),
  },
  css: {
    postcss: {
      plugins: [
        postcssCascadeLayers(),
        postcssGlobalData({ files: ["./src/styles/media.css"] }),
        postcssSimpleVars({ variables: RESPONSIVE_POSTCSS_VARIABLES }),
        postcssCustomMedia(),
        flexGapPolyfill({
          only: true,
          flexGapNotSupported: ".flexGapNotSupported",
        }),
      ],
    },
  },
  plugins: [
    quickPasteAndroidGate(),
    react(),
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
    {
      name: "copypaste:production-web-bridge-runtime",
      apply: "build",
      generateBundle() {
        this.emitFile({
          type: "asset",
          fileName: "copypaste-web-bridge.js",
          source: "window.__COPYPASTE_WEB_BRIDGE__ = null;\n",
        });
      },
    },
  ],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: { host: "127.0.0.1", port: 1420, strictPort: true },
  // The Android WebView is the one pinned into the emulator's system image, not
  // a Play-updated one, so both engines are measured rather than assumed: API 29
  // is Chromium 74 and API 24 is Chromium 53 (`scripts/webview-baseline.mjs`).
  // The plugin owns the lowering for both builds, and
  // `scripts/check-webview-baseline.mjs` holds each emitted chunk to the engine
  // that will load it.
  build: {
    // Run 33131597896 rendered route CSS but never fired the custom-protocol
    // link event Vite awaits before importing its chunk. Android loads one
    // static sheet; JavaScript routes stay split and independently lazy.
    cssCodeSplit: !androidBuild,
    cssTarget: LEGACY_CSS_TARGET,
    emptyOutDir: true,
  },
});
