import { APPEARANCE_SERIALIZATION } from "./appearancePrefs.ts";
import { DEFAULT_PREVIEW_LINES } from "./previewDensity.ts";

export const STORAGE_KEY = APPEARANCE_SERIALIZATION.storageKey;
export const PREFERENCES_VERSION = APPEARANCE_SERIALIZATION.version;

export const HISTORY_DISPLAY_LIMITS = [
  100,
  250,
  500,
  1000,
  2500,
  5000,
  10_000,
  100_000,
] as const;
export const UNLIMITED_HISTORY_DISPLAY = 100_000;

export interface Prefs {
  theme: (typeof APPEARANCE_SERIALIZATION.themes)[number];
  colorTheme: (typeof APPEARANCE_SERIALIZATION.colorThemes)[number];
  translucency: boolean;
  previewLines: number;
  previewLinesPopup: number;
  sortByDevice: boolean;
  historyDisplayLimit: (typeof HISTORY_DISPLAY_LIMITS)[number];
  /** Default on (n9gp). There is deliberately no "Mask sensitive data" toggle
   * beside it: the bridge drops plaintext before it crosses into the WebView. */
  warnBeforeReveal: boolean;
  /** INV-35, inverted: the window remains content-protected unless this is on. */
  allowScreenshots: boolean;
  onboardingComplete: boolean;
}

export const DEFAULT_PREFS: Prefs = {
  ...APPEARANCE_SERIALIZATION.defaults,
  previewLines: DEFAULT_PREVIEW_LINES,
  previewLinesPopup: DEFAULT_PREVIEW_LINES,
  sortByDevice: false,
  historyDisplayLimit: 1000,
  warnBeforeReveal: true,
  allowScreenshots: false,
  onboardingComplete: false,
};
