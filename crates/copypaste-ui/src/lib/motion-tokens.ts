/**
 * Motion duration constants (ms) — mirror the --mo-* CSS custom properties from
 * styles/tokens.css.  These TS constants let components set JS timers without
 * hardcoding magic numbers that diverge from the CSS animation durations.
 *
 * Source of truth for each value: the corresponding --mo-* token in tokens.css.
 * If tokens.css changes a duration, update the matching constant here too.
 */

export const MO_INSTANT_MS = 90;   // --mo-instant
export const MO_FAST_MS    = 130;  // --mo-fast
export const MO_BASE_MS    = 180;  // --mo-base
export const MO_SLOW_MS    = 240;  // --mo-slow

/**
 * Copy-flash JS timer: must match the .copy-flash CSS animation duration.
 * Both are driven by --mo-base (180 ms) — single source of truth (crh3.20).
 *
 * The JS timer clears the `copy-flash` class after the animation completes so
 * a subsequent copy can re-trigger the flash.  Keeping timer === animation
 * duration prevents the class being removed while the animation is still
 * visible (previous bug: CSS finished at 90 ms, JS cleared at 200 ms).
 */
export const COPY_FLASH_MS = MO_BASE_MS;
