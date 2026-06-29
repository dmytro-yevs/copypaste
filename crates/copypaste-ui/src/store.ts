import { create } from "zustand";

export type ViewId = "history" | "devices" | "settings" | "about" | "logs";

// ---------------------------------------------------------------------------
// AccentId — §2 STYLEGUIDE two-axis appearance: theme × accent.
// Six hues; "indigo" is the default. Drives data-accent on <html>.
// ---------------------------------------------------------------------------
export type AccentId = "indigo" | "blue" | "teal" | "green" | "amber" | "rose";

// ---------------------------------------------------------------------------
// UI preferences persisted to localStorage
// ---------------------------------------------------------------------------

// v4 key — Phase 2 redesign: two-axis theming (theme × accent). Migrates from v3/v2/v1.
const PREFS_KEY = "copypaste-ui-prefs-v4";
// v3 key — legacy; migrated to v4 (drops old appearance fields).
const LEGACY_PREFS_V3_KEY = "copypaste-ui-prefs-v3";
// v2 key — legacy; migrated to v4.
const LEGACY_PREFS_V2_KEY = "copypaste-ui-prefs-v2";
// v1 key — pre-redesign legacy. See loadPrefs() migration block.
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
   * UI color theme — §2 STYLEGUIDE two-axis appearance axis 1.
   *   "dark"  (default) — §2 STYLEGUIDE mandates dark-first.
   *   "light"           — Light theme.
   * Applied via <html data-theme="dark|light"> in App.tsx.
   * Note: the "system" value from v1–v3 is removed; old stored "system" is
   * mapped to "dark" by the migration shim in loadPrefs().
   */
  theme: "dark" | "light";
  /**
   * Active accent color — §2 STYLEGUIDE two-axis appearance axis 2.
   * Drives data-accent on <html>. Default: "indigo".
   * Six allowed values: indigo · blue · teal · green · amber · rose.
   */
  accent: AccentId;
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
  // §2 STYLEGUIDE dark-first: "dark" is the default theme. The former
  // PARITY-SPEC §0 light-first default is superseded by this redesign (CopyPaste-2hfj.3).
  theme: "dark",
  // §2 STYLEGUIDE: "indigo" is the default accent.
  accent: "indigo",
  // 1000 items is a sensible default — fast to render, shows plenty of history.
  historyDisplayLimit: 1000,
  // Show the "Sensitive — preview hidden · click to reveal" overlay by default (Android parity).
  showSensitiveWarnings: true,
  // Off by default — matches Android's default (sortByDevice is opt-in on both platforms).
  sortByDevice: false,
};

function loadPrefs(): UIPrefs {
  try {
    let raw = localStorage.getItem(PREFS_KEY);
    // ── Two-axis theming upgrade migration (v3 → v4) ──────────────────────
    // Phase 2 (CopyPaste-2hfj.3): old appearance fields removed from UIPrefs v4.
    // If only v3 prefs exist, adopt them and run the cleanup below.
    let migratedFromV3 = false;
    if (!raw) {
      const v3 = localStorage.getItem(LEGACY_PREFS_V3_KEY);
      if (v3) {
        raw = v3;
        migratedFromV3 = true;
      }
    }
    // ── v2 → v4 migration ────────────────────────────────────────────────
    // If only v2 prefs exist, adopt them and run the cleanup below.
    let migratedFromV2 = false;
    if (!raw) {
      const v2 = localStorage.getItem(LEGACY_PREFS_V2_KEY);
      if (v2) {
        raw = v2;
        migratedFromV2 = true;
      }
    }
    // ── Pre-Liquid-Glass upgrade migration (v1 → v4) ──────────────────────
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

    // ── Drop all keys not in the current schema (applies on every upgrade path) ──
    // Old appearance fields (Phase 2, CopyPaste-2hfj.3) and any future renames are
    // cleaned up automatically by this whitelist approach.
    const knownKeys = new Set(Object.keys(DEFAULT_PREFS));
    for (const key of Object.keys(parsed)) {
      if (!knownKeys.has(key)) {
        delete parsed[key];
      }
    }
    // "system" was a valid theme value in v1–v3; §2 STYLEGUIDE removes it.
    // Map to "dark" (the new §2 default — graphite is the design-language default).
    if (parsed.theme === "system") {
      parsed.theme = "dark";
    }
    // v1 used "dark" as its default; former PARITY-SPEC §0 reset it to "light"
    // on upgrade. That reset is removed — §2 STYLEGUIDE restores dark as default.
    // v1 users who stored "dark" (explicitly or via old default) keep it; "light"
    // is also preserved as-is (an explicit user choice).

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

    const merged = { ...DEFAULT_PREFS, ...parsed } as UIPrefs;
    if (migratedFromV3) {
      // Persist under v4 and drop the v3 key so this runs exactly once.
      savePrefs(merged);
      try { localStorage.removeItem(LEGACY_PREFS_V3_KEY); } catch { /* ignore */ }
    }
    if (migratedFromV2) {
      // Persist under v4 and drop the v2 key so this runs exactly once.
      savePrefs(merged);
      try { localStorage.removeItem(LEGACY_PREFS_V2_KEY); } catch { /* ignore */ }
    }
    if (migratedFromLegacy) {
      // Persist under v4 and drop the legacy v1 key so this runs exactly once.
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
