import { create } from "zustand";

export type ViewId = "history" | "devices" | "settings" | "about";

// ---------------------------------------------------------------------------
// UI preferences persisted to localStorage
// ---------------------------------------------------------------------------

const PREFS_KEY = "copypaste-ui-prefs-v1";

export interface UIPrefs {
  /** Number of text lines shown in a clip preview (1–10). */
  previewLines: number;
  /** Preview row height in px (28–80). */
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
   * Maximum number of clipboard items to display. Default 200, range 1–999.
   * Used as the `limit` parameter of history_page so HistoryView never shows
   * more than this many items.
   */
  historySize: number;
  /**
   * Delay in milliseconds before showing a large hover-preview of an item.
   * Default 1500 ms, range 200–100 000. Persisted for future use when a
   * hover-preview panel is implemented.
   * TODO: wire this to a hover-preview component when one is built.
   */
  previewDelay: number;
  /**
   * Play a soft system sound (Tink) when an item is copied — Maccy parity.
   * Default false (Maccy default is off).
   */
  playSoundOnCopy: boolean;
  /**
   * Show a macOS notification banner when an item is copied — Maccy parity.
   * Default false (Maccy default is off).
   */
  notifyOnCopy: boolean;
}

const DEFAULT_PREFS: UIPrefs = {
  previewLines: 1,
  previewSize: 28,
  maskSensitive: true,
  imageMaxHeight: 40,
  historySize: 200,
  previewDelay: 1500,
  playSoundOnCopy: false,
  notifyOnCopy: false,
};

function loadPrefs(): UIPrefs {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (!raw) return DEFAULT_PREFS;
    return { ...DEFAULT_PREFS, ...JSON.parse(raw) };
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
