/**
 * The four appearance axes on `<html>`, which every themed selector in
 * `design/dist/css` keys off.
 *
 * AT-53: the subscription is module-level and idempotent because v1
 * accumulated a matchMedia listener on every re-apply (CopyPaste-g27b.20).
 */
import {
  APPEARANCE_SERIALIZATION,
  translucencyAttribute,
  translucencyStyle,
  type Accent,
  type ThemePref,
  type Translucency,
} from "@/lib/appearancePrefs";
import type { Prefs } from "@/store/prefs";
import { applyNativeAppearance, applySystemAccent } from "@/lib/nativeAppearance";

export type ResolvedTheme = "dark" | "light";

const DARK_QUERY = APPEARANCE_SERIALIZATION.systemThemeQuery;

function systemTheme(): ResolvedTheme {
  // matchMedia is absent in some environments; dark is the default theme.
  if (typeof window === "undefined" || !window.matchMedia) {
    return APPEARANCE_SERIALIZATION.systemThemeFallback;
  }
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

export function resolveTheme(pref: ThemePref): ResolvedTheme {
  return pref === "system" ? systemTheme() : pref;
}

export function applyAppearance(prefs: {
  theme: ThemePref;
  accent: Accent;
  translucency: Translucency;
}): void {
  const root = document.documentElement;
  const theme = resolveTheme(prefs.theme);
  root.dataset.theme = theme;
  root.dataset.themePref = prefs.theme;
  root.dataset.accent = prefs.accent;
  if (prefs.accent === "system") {
    void applySystemAccent();
  } else {
    clearSystemAccent(root);
  }
  root.dataset.translucency = translucencyAttribute(prefs.translucency);
  for (const [name, value] of Object.entries(translucencyStyle(prefs.translucency))) {
    root.style.setProperty(name, value);
  }
  applyNativeAppearance(theme);
}

function clearSystemAccent(root: HTMLElement): void {
  for (const name of [
    "--accent",
    "--accent-2",
    "--on-accent",
    "--accent-away",
    "--system-accent-preview",
    "--system-on-accent",
  ]) {
    root.style.removeProperty(name);
  }
}

let subscribed = false;

export function subscribeSystemTheme(onChange: () => void): void {
  if (subscribed || typeof window === "undefined" || !window.matchMedia) return;
  subscribed = true;
  window.matchMedia(DARK_QUERY).addEventListener("change", onChange);
}

/** Test seam: the guard above is process-wide. */
export function _resetSystemThemeSubscription(): void {
  subscribed = false;
}

export type Appearance = Pick<Prefs, "theme" | "accent" | "translucency">;
