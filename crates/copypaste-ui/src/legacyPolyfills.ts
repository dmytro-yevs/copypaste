/**
 * Reached only from the nomodule build — API 24's Chromium 53. core-js carries
 * the builtins @vitejs/plugin-legacy derives from the target, so what is left
 * is the DOM and Intl it does not.
 *
 * An app module on purpose: the plugin lowers these and not the chunk
 * `additionalLegacyPolyfills` builds, and `@formatjs` ships optional chaining —
 * which made the rescue for Chromium 53 the one thing it could not parse.
 *
 * `BigInt` is absent: unpolyfillable, and zod's only use is `typeof`-guarded.
 */
import "abortcontroller-polyfill/dist/polyfill-patch-fetch";
import "@formatjs/intl-relativetimeformat/polyfill.js";
// `en` is the only catalogue the app ships (`src/i18n/`), so one locale is the
// whole surface rather than a default.
import "@formatjs/intl-relativetimeformat/locale-data/en.js";
import { ResizeObserver as ResizeObserverPolyfill } from "@juggle/resize-observer";

// Assigned rather than imported for its side effect: the package exports the
// class and leaves installing it to the caller.
if (!("ResizeObserver" in window)) {
  (window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverPolyfill;
}
