/**
 * The two engines the Android matrix runs, read out of the emulator images
 * rather than inferred. Both are pinned into their system image, so neither
 * moves the way a Play-updated WebView on a real device does.
 *
 * | image | com.google.android.webview |
 * |---|---|
 * | android-29 google_apis x86_64 | 74.0.3729.185 |
 * | android-24 google_apis x86_64 r27 | 53.0.2785.124 |
 */

/** API 29, 33, 34 and 36 — and the floor for Windows and macOS. */
export const MODERN_TARGET = "chrome74";

/** The modern engine as a browserslist query, for the plugin and for Babel. */
export const MODERN_BROWSERSLIST = "chrome >= 74";

/** API 24, which reaches only the nomodule build. */
export const LEGACY_TARGET = "chrome53";

/** The same engine as a browserslist query, which is what `@vitejs/plugin-legacy`
 *  and Babel take; `build.target` takes the esbuild spelling above. */
export const LEGACY_BROWSERSLIST = "chrome >= 53";

/** `@babel/compat-data` spells a target this way. */
export function babelTarget(target) {
  return { chrome: target.replace(/^chrome/, "") };
}
