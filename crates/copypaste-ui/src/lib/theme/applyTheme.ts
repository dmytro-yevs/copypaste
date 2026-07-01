import {
  translucencyAttr,
  type AccentValue,
  type ThemeValue,
} from "./prefsSchema";

/** The three appearance axes that map to `<html>` `data-*` attributes. */
export interface AppearancePrefs {
  theme: ThemeValue;
  accent: AccentValue;
  translucency: boolean;
}

/**
 * Apply the appearance axes to a root element's `data-theme` / `data-accent` /
 * `data-translucency` attributes. The pre-paint `theme-bootstrap.js` owns the
 * FIRST paint; this owns LIVE updates from within a window (task 1.16). Kept as
 * one function so `App.tsx` and `Popup.tsx` write the exact same attributes the
 * bootstrap and the token-layer CSS selectors key off of.
 */
export function applyAppearanceToRoot(
  el: HTMLElement,
  prefs: AppearancePrefs,
): void {
  el.dataset.theme = prefs.theme;
  el.dataset.accent = prefs.accent;
  el.dataset.translucency = translucencyAttr(prefs.translucency);
}
