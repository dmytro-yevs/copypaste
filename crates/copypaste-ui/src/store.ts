import { create } from "zustand";

export type ViewId = "history" | "devices" | "settings" | "about" | "logs";

// ---------------------------------------------------------------------------
// UI preferences persisted to localStorage
// ---------------------------------------------------------------------------

const PREFS_KEY = "copypaste-ui-prefs-v2";
// Pre-Liquid-Glass key. v1 persisted theme:"dark" as the old default; on upgrade
// we migrate non-theme fields and DROP the stored theme so the new light-first
// default applies once (otherwise the stale "dark" overrides it and the user
// keeps seeing the old palette). See loadPrefs().
const LEGACY_PREFS_KEY = "copypaste-ui-prefs-v1";

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
   * When true (default), the main window uses backdrop-blur translucency
   * (native macOS vibrancy + CSS backdrop-filter). When false, all surfaces
   * use solid opaque backgrounds — useful for accessibility or low-end GPUs.
   */
  translucency: boolean;
  /**
   * UI color theme.
   *   "dark"   (default) — Graphite Mist dark palette (CopyPaste-52mz new default).
   *   "light"            — Light palette (cloudSilver / system light look).
   *   "system"           — follow the OS `prefers-color-scheme` live (App.tsx
   *                        resolves it via matchMedia and re-resolves on change).
   * Applied via <html data-theme="light|dark"> in App.tsx.
   */
  theme: "dark" | "light" | "system";
  /**
   * Row density for the History view (Design System v2 §9 — Liquid Glass redesign).
   * "comfortable" = standard row spacing; "compact" (default) = reduced row height.
   * "spacious" = larger row height with more breathing room.
   * Compact matches the Graphite Mist styleguide default density.
   * CSS rule for spacious is at index.css html[data-density='spacious'] (already defined).
   */
  density: "comfortable" | "compact" | "spacious";
  /**
   * Active palette key. Drives data-palette attribute on <html>.
   * Default: "graphite-mist" (dark grey — CopyPaste-52mz).
   * A future palette picker writes this via setPrefs({ palette: "..." }).
   * Consuming code: App.tsx sets document.documentElement.setAttribute("data-palette", ...).
   */
  palette: string;
  /**
   * When true, the UI uses the "calm" motion profile (slow aurora, reduced
   * opacity) instead of the default "cinematic" profile.  Also syncs to
   * document.documentElement data-motion="calm"|"cinematic" in App.tsx.
   * Default: false (cinematic — the Liquid Glass launch default).
   */
  motionReduced: boolean;
  /**
   * Maximum number of items rendered in the HistoryView list.
   * This is a UI-only display filter — the daemon may store more items on disk
   * (pruned by byte quota). Sentinel value 100000 means "Unlimited".
   * Default: 1000. Range: one of [100, 250, 500, 1000, 2500, 5000, 10000, 100000].
   */
  historyDisplayLimit: number;
  /**
   * When true (default), show a "Sensitive — click to reveal" confirmation overlay
   * before revealing a sensitive item in the history list or detail modal. When
   * false, sensitive items are still blurred by maskSensitive but tapping/clicking
   * the blur reveals them immediately without an extra confirmation step.
   * Android parity: Android shows a warning sheet before revealing; this flag
   * lets users who find the warning redundant disable it (default on = same behaviour).
   */
  showSensitiveWarnings: boolean;
}

const DEFAULT_PREFS: UIPrefs = {
  previewLinesApp: 1,
  previewLinesPopup: 1,
  previewSize: 28,
  maskSensitive: true,
  imageMaxHeight: 40,
  playSoundOnCopy: true,
  notifyOnCopy: true,
  translucency: true,
  // PARITY-SPEC §0: light-first — default is "light"; saved pref overrides on load.
  theme: "light",
  density: "compact",
  palette: "graphite-mist",
  // Cinematic (false) is the Liquid Glass launch default — full aurora animation.
  motionReduced: false,
  // 1000 items is a sensible default — fast to render, shows plenty of history.
  historyDisplayLimit: 1000,
  // Show the "Sensitive — click to reveal" overlay by default (Android parity).
  showSensitiveWarnings: true,
};

function loadPrefs(): UIPrefs {
  try {
    let raw = localStorage.getItem(PREFS_KEY);
    // ── Liquid Glass upgrade migration (v1 → v2) ──────────────────────────
    // If only the legacy v1 prefs exist, adopt them but DROP the persisted
    // theme so the PARITY-SPEC §0 light default applies once.
    // Then re-persist under v2.
    let migratedFromLegacy = false;
    if (!raw) {
      const legacy = localStorage.getItem(LEGACY_PREFS_KEY);
      if (legacy) {
        raw = legacy;
        migratedFromLegacy = true;
      } else {
        return DEFAULT_PREFS;
      }
    }
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (migratedFromLegacy) {
      // The old v1 default was "dark" (pre-Liquid-Glass). PARITY-SPEC §0 now
      // mandates light-first. Only reset theme when it was the v1 DEFAULT ("dark")
      // — a v1 user who explicitly chose "light"/"system" keeps their choice
      // (deleting unconditionally would silently override an explicit pick).
      if (parsed.theme === "dark") delete parsed.theme;
      delete parsed.palette;
    }

    // ── v0.5.3 migration ──────────────────────────────────────────────────
    // Migrate the legacy `previewLines` (shared) field to the new split fields.
    // `historySize` and `previewDelay` are no longer stored — removed silently.
    if (typeof parsed.previewLines === "number" && parsed.previewLines > 0) {
      if (parsed.previewLinesApp === undefined) {
        parsed.previewLinesApp = parsed.previewLines;
      }
      if (parsed.previewLinesPopup === undefined) {
        parsed.previewLinesPopup = parsed.previewLines;
      }
    }
    // Drop obsolete keys so they don't accumulate
    delete parsed.previewLines;
    delete parsed.historySize;
    delete parsed.previewDelay;
    // ──────────────────────────────────────────────────────────────────────

    const merged = { ...DEFAULT_PREFS, ...parsed };
    if (migratedFromLegacy) {
      // Persist under v2 and drop the legacy key so this runs exactly once.
      savePrefs(merged);
      try { localStorage.removeItem(LEGACY_PREFS_KEY); } catch { /* ignore */ }
    }
    return merged;
  } catch (err) {
    console.warn("loadPrefs: failed to read localStorage, using defaults", err);
    return DEFAULT_PREFS;
  }
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
}));
