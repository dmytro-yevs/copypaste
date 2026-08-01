/**
 * INV-21: an invalid `theme` must not discard a valid `accent`, so every field
 * is parsed independently. INV-22: the external pre-paint bootstrap is
 * generated from the same appearance serialization contract used here.
 */
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { z } from "zod";

import {
  APPEARANCE_SERIALIZATION,
  parseAppearanceFields,
  unwrapPersistedPrefs,
  type Accent,
  type ThemePref,
} from "@/lib/appearancePrefs";
import {
  DEFAULT_PREVIEW_LINES,
  MAX_PREVIEW_LINES,
  MIN_PREVIEW_LINES,
} from "@/lib/layout";

export const STORAGE_KEY = APPEARANCE_SERIALIZATION.storageKey;

export const THEMES = APPEARANCE_SERIALIZATION.themes;
export const ACCENTS = APPEARANCE_SERIALIZATION.accents;

export type { Accent, ThemePref };

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
  theme: ThemePref;
  accent: Accent;
  translucency: boolean;
  previewLines: number;
  previewLinesPopup: number;
  sortByDevice: boolean;
  historyDisplayLimit: (typeof HISTORY_DISPLAY_LIMITS)[number];
  /** Default on (n9gp). There is deliberately no "Mask sensitive data" toggle
   *  beside it: the bridge drops the plaintext before it crosses into the
   *  WebView, so nothing here *can* unmask an item without asking, and a toggle
   *  that cannot be honoured is worse than none. */
  warnBeforeReveal: boolean;
  /** INV-35, inverted: the window is content-protected unless this is on. The
   *  default matches what the window is created with, so a preference that
   *  fails to load leaves the user protected. */
  allowScreenshots: boolean;
}

export const DEFAULT_PREFS: Prefs = {
  ...APPEARANCE_SERIALIZATION.defaults,
  previewLines: DEFAULT_PREVIEW_LINES,
  previewLinesPopup: 1,
  sortByDevice: false,
  historyDisplayLimit: 1000,
  warnBeforeReveal: true,
  allowScreenshots: false,
};

/**
 * `safeParse` per field rather than `.catch()` on one object schema: INV-21
 * requires a *warning* on a present-but-invalid field, and a silent fallback
 * cannot tell that apart from an absent one.
 */
const FIELD = {
  previewLines: z.number().int().min(MIN_PREVIEW_LINES).max(MAX_PREVIEW_LINES),
  previewLinesPopup: z
    .number()
    .int()
    .min(MIN_PREVIEW_LINES)
    .max(MAX_PREVIEW_LINES),
  sortByDevice: z.boolean(),
  historyDisplayLimit: z
    .number()
    .refine((value) =>
      (HISTORY_DISPLAY_LIMITS as readonly number[]).includes(value),
    ),
  warnBeforeReveal: z.boolean(),
  allowScreenshots: z.boolean(),
} as const;

/** Never throws. Unknown keys are dropped by construction: the result is built
 *  from the known key list, so nothing else survives to be re-persisted. */
export function parsePrefs(raw: unknown): Prefs {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    if (raw !== undefined && raw !== null) {
      console.warn("[copypaste] stored preferences were not an object; using defaults");
    }
    return { ...DEFAULT_PREFS };
  }

  const source = raw as Record<string, unknown>;
  const appearance = parseAppearanceFields(
    source,
    APPEARANCE_SERIALIZATION,
    (key) => {
      console.warn(
        `[copypaste] preference "${key}" was invalid and has been reset to its default`,
      );
    },
  );
  const out = { ...DEFAULT_PREFS, ...appearance };

  for (const key of Object.keys(FIELD) as Array<keyof typeof FIELD>) {
    if (!(key in source)) continue; // absent -> silent default
    const result = FIELD[key].safeParse(source[key]);
    if (result.success) {
      (out[key] as unknown) = result.data;
    } else {
      console.warn(
        `[copypaste] preference "${key}" was invalid and has been reset to its default`,
      );
    }
  }

  return out;
}

export function readPrefs(): Prefs {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === null) return { ...DEFAULT_PREFS };
    // zustand/persist wraps state as { state, version }.
    const parsed: unknown = JSON.parse(stored);
    return parsePrefs(unwrapPersistedPrefs(parsed));
  } catch {
    console.warn("[copypaste] preferences could not be read; using defaults");
    return { ...DEFAULT_PREFS };
  }
}

/** Private mode, a full quota and a disabled store all land here, and none of
 *  them may take the window down. */
const safeStorage = {
  getItem(name: string): string | null {
    try {
      return window.localStorage.getItem(name);
    } catch {
      return null;
    }
  },
  setItem(name: string, value: string): void {
    try {
      window.localStorage.setItem(name, value);
    } catch {
      console.warn("[copypaste] preferences could not be saved");
    }
  },
  removeItem(name: string): void {
    try {
      window.localStorage.removeItem(name);
    } catch {
      /* nothing to do */
    }
  },
};

interface PrefsStore extends Prefs {
  set<K extends keyof Prefs>(key: K, value: Prefs[K]): void;
  reset(): void;
}

export const usePrefs = create<PrefsStore>()(
  persist(
    (setState) => ({
      ...DEFAULT_PREFS,
      set: (key, value) => setState({ [key]: value } as Partial<Prefs>),
      reset: () => setState({ ...DEFAULT_PREFS }),
    }),
    {
      name: STORAGE_KEY,
      storage: createJSONStorage(() => safeStorage),
      partialize: (state) =>
        // Built from the known key list so an action can never be persisted,
        // and so a key removed from `Prefs` stops being written on the next
        // save rather than lingering in storage forever.
        Object.fromEntries(
          Object.keys(DEFAULT_PREFS).map((key) => [
            key,
            state[key as keyof Prefs],
          ]),
        ) as unknown as Prefs,
      // INV-21: validated on the way in, so a corrupt entry cannot reach a
      // component.
      merge: (persisted, current) => ({ ...current, ...parsePrefs(persisted) }),
    },
  ),
);

/** **Wrap this in `useShallow`.** It returns a fresh object per call and
 *  zustand v5 compares snapshots by reference, so an unwrapped call is an
 *  infinite render loop that unmounts the app, not a performance smell. */
export const selectAppearance = (s: PrefsStore) => ({
  theme: s.theme,
  accent: s.accent,
  translucency: s.translucency,
});
