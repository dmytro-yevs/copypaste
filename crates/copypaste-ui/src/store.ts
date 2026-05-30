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
