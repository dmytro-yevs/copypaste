/**
 * Reached only from the nomodule build, which only an engine without ES modules
 * loads — API 24's Chromium 53. `@vitejs/plugin-legacy` decides the ECMAScript
 * builtins from the target and core-js supplies them, so nothing here restates
 * one. What is left is DOM and Intl, which core-js does not carry.
 *
 * An ordinary app module on purpose: the plugin lowers these, and the polyfill
 * chunk it builds from `additionalLegacyPolyfills` it does not — `@formatjs`
 * ships optional chaining, which would have made the chunk that exists to
 * rescue Chromium 53 the one thing on the page it could not parse.
 *
 * `BigInt` is deliberately absent. It cannot be polyfilled, and zod's only use
 * of it sits behind `typeof value === "bigint"`, which no engine lacking it
 * can enter.
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
