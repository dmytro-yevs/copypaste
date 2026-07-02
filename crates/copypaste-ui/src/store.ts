import { create } from "zustand";
import {
  PREFS_KEY,
  DEFAULT_ACCENT,
  DEFAULT_THEME,
  DEFAULT_TRANSLUCENCY,
  validateAccent,
  validateTheme,
  validateTranslucency,
  type AccentValue,
  type ThemeValue,
} from "./lib/theme/prefsSchema";

// About + Logs moved into Settings as trailing tabs — no longer top-level views.
export type ViewId = "history" | "devices" | "settings";

// ---------------------------------------------------------------------------
// UI preferences persisted to localStorage
// ---------------------------------------------------------------------------
//
// Single current key (`copypaste-ui-prefs-v4`, defined in ./lib/theme/prefsSchema).
// No back-compat: the former v1/v2/v3 legacy-key migration branches were removed
// in the design-system redesign (design.md Decision 10 / task 1.10a). A user whose
// prefs remain under an old v1–v3 key (never re-saved under v4) resets to defaults —
// an accepted, documented impact of the no-back-compat directive.

export interface UIPrefs {
  /**
   * Number of text lines shown in a clip preview in the main History window
   * (1–6). Separate from popup setting — see previewLinesPopup.
   */
  previewLinesApp: number;
  /**
   * Number of text lines shown in a clip preview in the Quick-Paste popup
   * (1–6). Separate from the main window setting — see previewLinesApp.
   */
  previewLinesPopup: number;
  /** Preview row height in px (24–64). Kept for layout wiring; not exposed in UI. */
  previewSize: number;
  /** When true, redact sensitive_spans ranges in clip previews. */
  maskSensitive: boolean;
  /**
   * Max height (px) of the image thumbnail bounding box — Maccy parity.
   * The image is scaled to fit within 340 × imageMaxHeight, aspect-preserving,
   * never upscaled. Default 40, range 1–200.
   */
  imageMaxHeight: number;
  /**
   * Play a soft system sound (Tink) when an item is copied — Maccy parity.
   * Default true.
   */
  playSoundOnCopy: boolean;
  /**
   * Show a macOS notification banner when an item is copied — Maccy parity.
   * Default true.
   */
  notifyOnCopy: boolean;
  /**
   * Maximum number of items rendered in the HistoryView list.
   * This is a UI-only display filter — the daemon may store more items on disk
   * (pruned by byte quota). Sentinel value 100000 means "Unlimited".
   * Default: 1000. Range: one of [100, 250, 500, 1000, 2500, 5000, 10000, 100000].
   */
  historyDisplayLimit: number;
  /**
   * When true (default), show a "Sensitive — preview hidden · click to reveal" confirmation overlay
   * before revealing a sensitive item in the history list or detail modal. When
   * false, sensitive items are still blurred by maskSensitive but tapping/clicking
   * the blur reveals them immediately without an extra confirmation step.
   * Android parity: Android shows a warning sheet before revealing; this flag
   * lets users who find the warning redundant disable it (default on = same behaviour).
   */
  showSensitiveWarnings: boolean;
  /**
   * When true, the History list groups items by the device they originated from,
   * with the local device shown first. Matches Android's "Group by device" pref
   * (Settings.kt:627 / strings.xml:483). Default false (sort by recency).
   * Android parity: Android default is also false (off-by-default).
   */
  sortByDevice: boolean;
  /**
   * Appearance theme axis — `"dark"` (default) or `"light"`. Applied to
   * `<html data-theme>` by the pre-paint bootstrap (first paint) and the
   * App/Popup effect (live). Additive field on the v4 key — no migration.
   */
  theme: ThemeValue;
  /**
   * Appearance accent axis — one of 6 hues (default `"indigo"`), independent of
   * theme. Applied to `<html data-accent>`. Additive field on the v4 key.
   */
  accent: AccentValue;
  /**
   * Translucency axis (default `true`). `true` frosts chrome surfaces via
   * `backdrop-filter`; `false` renders every surface solid. Applied to
   * `<html data-translucency="on"|"off">`. Additive field on the v4 key.
   */
  translucency: boolean;
}

export const DEFAULT_PREFS: UIPrefs = {
  previewLinesApp: 1,
  previewLinesPopup: 1,
  previewSize: 28,
  maskSensitive: true,
  imageMaxHeight: 40,
  playSoundOnCopy: true,
  notifyOnCopy: true,
  // 1000 items is a sensible default — fast to render, shows plenty of history.
  historyDisplayLimit: 1000,
  // Show the "Sensitive — preview hidden · click to reveal" overlay by default (Android parity).
  showSensitiveWarnings: true,
  // Off by default — matches Android's default (sortByDevice is opt-in on both platforms).
  sortByDevice: false,
  // Appearance axes (redesign, Slice 1). Defaults sourced from prefsSchema so the
  // store and the pre-paint bootstrap can never disagree on them (task 1.14).
  theme: DEFAULT_THEME,
  accent: DEFAULT_ACCENT,
  translucency: DEFAULT_TRANSLUCENCY,
};

export function loadPrefs(): UIPrefs {
  // Read the single current key. A localStorage access exception (private mode,
  // disabled storage) falls back to defaults rather than throwing to callers.
  let raw: string | null;
  try {
    raw = localStorage.getItem(PREFS_KEY);
  } catch (err) {
    console.warn("loadPrefs: localStorage access failed, using defaults", err);
    return DEFAULT_PREFS;
  }
  if (!raw) return DEFAULT_PREFS;

  // Malformed JSON → full defaults (logged, never thrown). A non-object payload
  // (array, string, number, null) is treated the same way.
  let parsed: Record<string, unknown>;
  try {
    const value: unknown = JSON.parse(raw);
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      console.warn("loadPrefs: stored prefs not an object, using defaults");
      return DEFAULT_PREFS;
    }
    parsed = value as Record<string, unknown>;
  } catch (err) {
    console.warn("loadPrefs: malformed JSON, using defaults", err);
    return DEFAULT_PREFS;
  }

  // Drop any key not in the current schema (unknown keys are dropped, never
  // re-persisted — design.md Decision 10). This also sweeps up stale appearance
  // fields from earlier eras via the same whitelist.
  const knownKeys = new Set(Object.keys(DEFAULT_PREFS));
  for (const key of Object.keys(parsed)) {
    if (!knownKeys.has(key)) {
      delete parsed[key];
    }
  }

  // Whitelist-merge over defaults: a blob predating the appearance fields gains
  // them at their defaults here (no migration step runs).
  const merged = { ...DEFAULT_PREFS, ...parsed } as UIPrefs;

  // Per-field runtime validation for the appearance axes: an invalid stored value
  // for one axis defaults independently without discarding the others, and can
  // never reach the DOM (design.md Decision 10 / task 1.11).
  merged.theme = validateTheme(merged.theme);
  merged.accent = validateAccent(merged.accent);
  merged.translucency = validateTranslucency(merged.translucency);

  return merged;
}

function savePrefs(prefs: UIPrefs) {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch (err) {
    console.warn("savePrefs: failed to write localStorage", err);
  }
}

interface UIState {
  view: ViewId;
  setView: (view: ViewId) => void;
  prefs: UIPrefs;
  setPrefs: (patch: Partial<UIPrefs>) => void;
  /**
   * Re-read persisted prefs from storage into the store. The quick-paste popup
   * is a warm WebView built once and shown/hidden — its JS runtime (and this
   * module's one-time `loadPrefs()`) never re-evaluates across shows. Calling
   * this when the popup regains focus lets it pick up appearance (and every
   * other pref) changed in the main window since it was built (task 1.17).
   */
  reloadPrefs: () => void;
}

export const useUI = create<UIState>((set, get) => ({
  view: "history",
  setView: (view) => set({ view }),
  prefs: loadPrefs(),
  setPrefs: (patch) => {
    const next = { ...get().prefs, ...patch };
    savePrefs(next);
    set({ prefs: next });
  },
  reloadPrefs: () => set({ prefs: loadPrefs() }),
}));
