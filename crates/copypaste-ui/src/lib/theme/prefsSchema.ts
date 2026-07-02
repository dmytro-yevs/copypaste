// ---------------------------------------------------------------------------
// Appearance preference schema — the single source of truth (Slice 1, task 1.14)
// ---------------------------------------------------------------------------
//
// The persisted-prefs KEY, the three appearance-axis default values, the allowed
// enum values, and the `translucency: boolean → data-translucency: "on"|"off"`
// dataset mapping are all needed in TWO places:
//
//   1. `store.ts`         — the authoritative React/Zustand loader (this module).
//   2. `theme-bootstrap.js` — a standalone pre-paint classic script in `public/`
//      that CANNOT `import` (it must stay a synchronous, dependency-free asset
//      served under `script-src 'self'`), so it re-declares the same literals.
//
// Because the bootstrap duplicates these literals, they can silently DRIFT from
// this module. `themeBootstrap.test.ts` asserts exact parity of KEY / defaults /
// allowed values / dataset mapping between the two (design.md Decision 4 · N1).
// This module is the value everything else is checked against.

export const PREFS_KEY = "copypaste-ui-prefs-v4";

export const THEME_VALUES = ["system", "dark", "light"] as const;
export const ACCENT_VALUES = [
  "indigo",
  "blue",
  "teal",
  "green",
  "amber",
  "rose",
] as const;

export type ThemeValue = (typeof THEME_VALUES)[number];
export type AccentValue = (typeof ACCENT_VALUES)[number];

/** The two concrete values the CSS token layer (`tokens.css`) keys off of —
 * `[data-theme="dark"|"light"]`. "system" is never a resolved value; it is
 * always mapped to one of these two via `resolveTheme()` before being written
 * to `data-theme`. */
export type ResolvedTheme = Exclude<ThemeValue, "system">;

// DEFAULT_THEME stays "dark", NOT "system" (CopyPaste-g27b.20): the pre-paint
// fallback in index.html / popup.html hardcodes `data-theme="dark"` as a
// static attribute (those files are outside this task's owned scope, see the
// worktree file list), and themeBootstrap.test.ts's anti-drift check asserts
// that static HTML default against DEFAULT_THEME. Flipping the default to
// "system" would require editing those HTML files to keep that test green,
// which this task does not own — so the safer, test-preserving choice is to
// keep "dark" as the DEFAULT_THEME while still fully supporting "system" as a
// user-selectable, non-default option.
export const DEFAULT_THEME: ThemeValue = "dark";
export const DEFAULT_ACCENT: AccentValue = "indigo";
export const DEFAULT_TRANSLUCENCY = true;

/**
 * Resolve a persisted theme CHOICE to the concrete "dark"/"light" value the
 * CSS token layer keys off of. "dark"/"light" pass through unchanged;
 * "system" is resolved live from the OS via `matchMedia`. SSR / environments
 * without `matchMedia` (or where it throws) fall back to "dark" — matching
 * DEFAULT_THEME — instead of throwing.
 */
export function resolveTheme(theme: ThemeValue): ResolvedTheme {
  if (theme !== "system") return theme;
  try {
    if (
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    ) {
      return "dark";
    }
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  } catch {
    return "dark";
  }
}

/**
 * `data-translucency` attribute value for a given boolean pref. The DOM axis is
 * `"on"`/`"off"` (design-tokens spec) — never the raw boolean — so the CSS
 * selectors `[data-translucency="on"|"off"]` resolve. Kept here so the store,
 * the runtime effect, and the bootstrap all map identically.
 */
export function translucencyAttr(translucency: boolean): "on" | "off" {
  return translucency ? "on" : "off";
}

// --- Per-field runtime validators (robustness only — NOT migration) ----------
//
// Each validator defaults ONE field independently: an invalid value for one axis
// never discards the others (design.md Decision 10). A value that is `undefined`
// (field absent from an older blob) defaults silently — the whitelist-merge in
// `loadPrefs()` already supplied the default, so absence is not a "fallback".
// A value that is present-but-invalid logs a console warning, because that is a
// real corruption/tampering signal the user/dev should see (task 1.11).

function warnInvalid(field: string, value: unknown, fallback: unknown): void {
  console.warn(
    `[prefs] invalid ${field} value ${JSON.stringify(value)}; using default ${JSON.stringify(
      fallback,
    )}`,
  );
}

export function validateTheme(value: unknown): ThemeValue {
  if (value === undefined) return DEFAULT_THEME;
  if (
    typeof value === "string" &&
    (THEME_VALUES as readonly string[]).includes(value)
  ) {
    return value as ThemeValue;
  }
  warnInvalid("theme", value, DEFAULT_THEME);
  return DEFAULT_THEME;
}

export function validateAccent(value: unknown): AccentValue {
  if (value === undefined) return DEFAULT_ACCENT;
  if (
    typeof value === "string" &&
    (ACCENT_VALUES as readonly string[]).includes(value)
  ) {
    return value as AccentValue;
  }
  warnInvalid("accent", value, DEFAULT_ACCENT);
  return DEFAULT_ACCENT;
}

export function validateTranslucency(value: unknown): boolean {
  if (value === undefined) return DEFAULT_TRANSLUCENCY;
  if (typeof value === "boolean") return value;
  warnInvalid("translucency", value, DEFAULT_TRANSLUCENCY);
  return DEFAULT_TRANSLUCENCY;
}
