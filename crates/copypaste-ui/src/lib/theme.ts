/**
 * The appearance axes on `<html>`, which every themed selector in
 * `design/dist/css` keys off.
 *
 * AT-53: the subscription is module-level and idempotent because v1
 * accumulated a matchMedia listener on every re-apply (CopyPaste-g27b.20).
 */
import {
  APPEARANCE_SERIALIZATION,
  translucencyAttribute,
  type ColorTheme,
  type ThemePref,
  type Translucency,
} from "@/lib/appearancePrefs";
import type { Prefs } from "@/store/prefs";
import { applyNativeAppearance } from "@/lib/nativeAppearance";

export type ResolvedTheme = "dark" | "light";

const DARK_QUERY = APPEARANCE_SERIALIZATION.systemThemeQuery;

function systemTheme(): ResolvedTheme {
  // matchMedia is absent in some environments; dark is the default theme.
  if (typeof window === "undefined" || !window.matchMedia) {
    return APPEARANCE_SERIALIZATION.systemThemeFallback;
  }
  return systemThemeMedia()?.matches ? "dark" : "light";
}

export function resolveTheme(pref: ThemePref): ResolvedTheme {
  return pref === "system" ? systemTheme() : pref;
}

export function applyAppearance(prefs: {
  theme: ThemePref;
  colorTheme: ColorTheme;
  translucency: Translucency;
}): void {
  const root = document.documentElement;
  const colorScheme = resolveTheme(prefs.theme);
  root.dataset.colorScheme = colorScheme;
  root.dataset.mode = prefs.theme;
  root.dataset.theme = prefs.colorTheme;
  root.dataset.translucency = translucencyAttribute(prefs.translucency);
  applyNativeAppearance(colorScheme);
}

let media: MediaQueryList | null = null;
let subscribed = false;
const themeListeners = new Set<() => void>();

function systemThemeMedia(): MediaQueryList | null {
  if (media) return media;
  if (typeof window === "undefined" || !window.matchMedia) return null;
  media = window.matchMedia(DARK_QUERY);
  return media;
}

function notifySystemTheme(): void {
  for (const listener of themeListeners) listener();
}

export function systemThemeSnapshot(): ResolvedTheme {
  return systemTheme();
}

export function subscribeSystemTheme(onChange: () => void): () => void {
  const query = systemThemeMedia();
  themeListeners.add(onChange);
  if (!subscribed && query) {
    subscribed = true;
    query.addEventListener("change", notifySystemTheme);
  }
  return () => {
    themeListeners.delete(onChange);
    if (themeListeners.size === 0 && subscribed && media) {
      media.removeEventListener?.("change", notifySystemTheme);
      subscribed = false;
    }
  };
}

/** Test seam: the guard above is process-wide. */
export function _resetSystemThemeSubscription(): void {
  media?.removeEventListener?.("change", notifySystemTheme);
  media = null;
  themeListeners.clear();
  subscribed = false;
}

export type Appearance = Pick<Prefs, "theme" | "colorTheme" | "translucency">;
