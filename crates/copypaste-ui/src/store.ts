import { create } from "zustand";

export type ViewId = "history" | "devices" | "settings" | "about";

// ---------------------------------------------------------------------------
// UI preferences persisted to localStorage
// ---------------------------------------------------------------------------

const PREFS_KEY = "copypaste-ui-prefs-v1";

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
};

function loadPrefs(): UIPrefs {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (!raw) return DEFAULT_PREFS;
    const parsed = JSON.parse(raw) as Record<string, unknown>;

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

    return { ...DEFAULT_PREFS, ...parsed };
  } catch {
    return DEFAULT_PREFS;
  }
}

function savePrefs(prefs: UIPrefs) {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // ignore
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
