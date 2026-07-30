/**
 * The four appearance axes on `<html>`, and the live system-theme subscription.
 *
 * `data-theme` (resolved) · `data-theme-pref` (the raw choice) · `data-accent` ·
 * `data-translucency`. Every themed selector in `design/dist/css` keys off
 * these, so this module is the only code in the app that decides what anything
 * looks like — and it still writes no colour.
 *
 * AT-53: a `theme: "system"` window follows the OS live, in both directions,
 * with **exactly one** matchMedia listener however many times the appearance is
 * re-applied. v1 accumulated a listener per apply (CopyPaste-g27b.20), so the
 * subscription here is module-level and idempotent.
 */
import type { Accent, Prefs, ThemePref } from "@/store/prefs";

export type ResolvedTheme = "dark" | "light";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function systemTheme(): ResolvedTheme {
  // matchMedia is absent in some test environments; dark is the default theme.
  if (typeof window === "undefined" || !window.matchMedia) return "dark";
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

export function resolveTheme(pref: ThemePref): ResolvedTheme {
  return pref === "system" ? systemTheme() : pref;
}

export function applyAppearance(prefs: {
  theme: ThemePref;
  accent: Accent;
  translucency: boolean;
}): void {
  const root = document.documentElement;
  root.dataset.theme = resolveTheme(prefs.theme);
  root.dataset.themePref = prefs.theme;
  root.dataset.accent = prefs.accent;
  root.dataset.translucency = prefs.translucency ? "on" : "off";
}

let subscribed = false;

/**
 * Subscribe once to OS appearance changes. Calling it again is a no-op, which
 * is what keeps re-applies from accumulating listeners.
 */
export function subscribeSystemTheme(onChange: () => void): void {
  if (subscribed || typeof window === "undefined" || !window.matchMedia) return;
  subscribed = true;
  window.matchMedia(DARK_QUERY).addEventListener("change", onChange);
}

/** Test seam: the module-level guard above is deliberately process-wide. */
export function _resetSystemThemeSubscription(): void {
  subscribed = false;
}

/** The full appearance slice of the prefs, for callers that hold all of it. */
export type Appearance = Pick<Prefs, "theme" | "accent" | "translucency">;
