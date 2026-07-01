import {
  resolveTheme,
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

// Module-level live-OS-theme listener (CopyPaste-g27b.20). `applyAppearanceToRoot`
// is called from a useEffect in App.tsx / Popup.tsx on EVERY prefs change (not
// once at startup), so it must manage its own subscription idempotently: tear
// down any previous listener before deciding whether the new prefs need one,
// rather than accumulating one per call. Each window (main app vs popup) is a
// separate JS module realm in Tauri, so this singleton is naturally scoped
// per-window — no cross-window interference.
let unwatchSystemTheme: (() => void) | null = null;

/**
 * Subscribe `el`'s `data-theme` to live OS `prefers-color-scheme` changes,
 * re-resolving to "dark"/"light" on every change event. Returns an unsubscribe
 * function. When `matchMedia` is unavailable (SSR / older browsers) this is a
 * no-op subscription and the returned unsubscribe is a no-op too.
 */
export function watchSystemTheme(el: HTMLElement): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return () => {};
  }
  const mql = window.matchMedia("(prefers-color-scheme: dark)");
  const listener = () => {
    el.dataset.theme = resolveTheme("system");
  };
  mql.addEventListener("change", listener);
  return () => mql.removeEventListener("change", listener);
}

/**
 * Apply the appearance axes to a root element's `data-theme` / `data-accent` /
 * `data-translucency` attributes. The pre-paint `theme-bootstrap.js` owns the
 * FIRST paint; this owns LIVE updates from within a window (task 1.16). Kept as
 * one function so `App.tsx` and `Popup.tsx` write the exact same attributes the
 * bootstrap and the token-layer CSS selectors key off of.
 *
 * `data-theme` is always the RESOLVED "dark"/"light" value (what the CSS token
 * layer selects on — unchanged, no CSS edits needed). `data-theme-pref` is the
 * raw user CHOICE ("system"/"dark"/"light"), kept so the UI can show which
 * option is selected even while "system" is active. When the choice is
 * "system", a module-level `matchMedia` listener is (idempotently) set up so
 * OS theme changes update `data-theme` live without requiring callers to wire
 * up their own subscription (CopyPaste-g27b.20).
 */
export function applyAppearanceToRoot(
  el: HTMLElement,
  prefs: AppearancePrefs,
): void {
  el.dataset.theme = resolveTheme(prefs.theme);
  el.dataset.themePref = prefs.theme;
  el.dataset.accent = prefs.accent;
  el.dataset.translucency = translucencyAttr(prefs.translucency);

  if (unwatchSystemTheme) {
    unwatchSystemTheme();
    unwatchSystemTheme = null;
  }
  if (prefs.theme === "system") {
    unwatchSystemTheme = watchSystemTheme(el);
  }
}
