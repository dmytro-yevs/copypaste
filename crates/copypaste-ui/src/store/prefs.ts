/**
 * The state the daemon does **not** own. React Query holds what the service is
 * the source of truth for; this holds what only this window knows.
 *
 * INV-21: an invalid `theme` must not discard a valid `accent`, so every field
 * is parsed independently. INV-22: `readPrefs` is synchronous so `main.tsx` can
 * set the `<html>` attributes before first paint — v1 needed a second copy of
 * the schema in a pre-paint script, and AT-54 exists because that copy drifted.
 */
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { z } from "zod";

import {
  DEFAULT_PREVIEW_LINES,
  MAX_PREVIEW_LINES,
  MIN_PREVIEW_LINES,
} from "@/lib/layout";

export const STORAGE_KEY = "copypaste.prefs";

export const THEMES = ["system", "dark", "light"] as const;
export const ACCENTS = ["indigo", "blue", "teal", "green", "amber", "rose"] as const;

export type ThemePref = (typeof THEMES)[number];
export type Accent = (typeof ACCENTS)[number];

export interface Prefs {
  theme: ThemePref;
  accent: Accent;
  translucency: boolean;
  previewLines: number;
  /**
   * Ask before revealing a hidden sensitive item. Default on (n9gp).
   *
   * v1 also had a "Mask sensitive data" toggle. There is deliberately no
   * equivalent here: the bridge drops a sensitive item's plaintext before it
   * crosses into the WebView, so nothing in this window *can* unmask one
   * without asking for it. A toggle that cannot be honoured is worse than no
   * toggle — see `lib/ipc.ts`.
   */
  warnBeforeReveal: boolean;
}

export const DEFAULT_PREFS: Prefs = {
  theme: "system",
  accent: "indigo",
  translucency: false,
  previewLines: DEFAULT_PREVIEW_LINES,
  warnBeforeReveal: true,
};

/**
 * `safeParse` per field rather than `.catch()` on one object schema: INV-21
 * requires a *warning* on a present-but-invalid field, and a silent fallback
 * cannot tell that apart from an absent one.
 */
const FIELD = {
  theme: z.enum(THEMES),
  accent: z.enum(ACCENTS),
  translucency: z.boolean(),
  previewLines: z.number().int().min(MIN_PREVIEW_LINES).max(MAX_PREVIEW_LINES),
  warnBeforeReveal: z.boolean(),
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
  const out = { ...DEFAULT_PREFS };

  for (const key of Object.keys(FIELD) as Array<keyof Prefs>) {
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

/** Read prefs synchronously, tolerating every way storage can fail. */
export function readPrefs(): Prefs {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === null) return { ...DEFAULT_PREFS };
    // zustand/persist wraps state as { state, version }.
    const parsed: unknown = JSON.parse(stored);
    const inner =
      typeof parsed === "object" && parsed !== null && "state" in parsed
        ? (parsed as { state: unknown }).state
        : parsed;
    return parsePrefs(inner);
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

/**
 * Subscribing to one field keeps a slider from re-rendering the list.
 *
 * **Wrap this in `useShallow`.** It returns a fresh object per call, and
 * zustand v5 reads the store through `useSyncExternalStore`, which compares
 * snapshots by reference — an unwrapped call is an infinite render loop that
 * unmounts the whole app, not a performance smell. Every other selector in
 * this file and in `store/ui.ts` returns a primitive or a value held in state,
 * which is why this is the only one that needs it.
 */
export const selectAppearance = (s: PrefsStore) => ({
  theme: s.theme,
  accent: s.accent,
  translucency: s.translucency,
});
