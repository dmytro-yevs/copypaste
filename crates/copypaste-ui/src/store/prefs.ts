/**
 * Client-owned preferences: appearance and list display.
 *
 * This is the half of the state the **daemon does not own**, which is exactly
 * the split CLAUDE.md's architecture note draws — React Query holds everything
 * the service is the source of truth for (history, status, peers), and zustand
 * holds what only this window knows. Neither is a copy of the other.
 *
 * INV-21 — *prefs corruption defaults per field, never wholesale.* An invalid
 * `theme` must not discard a valid `accent`. That is `parsePrefs` below: every
 * field is parsed independently with its own fallback, unknown keys are dropped
 * and never re-persisted, a present-but-invalid field logs a warning, and an
 * absent one defaults silently. Malformed JSON, a non-object payload, or a
 * throwing `localStorage` all fall back to full defaults without throwing.
 *
 * INV-22 — *first paint already carries the persisted appearance.* `readPrefs`
 * is synchronous, and `main.tsx` calls it before `createRoot().render`, so the
 * `<html>` attributes are set before React puts anything in the body. v1 needed
 * a separate dependency-free pre-paint script because its store was inside the
 * app bundle; reading a few bytes of `localStorage` at module scope is the same
 * guarantee without the second copy of the schema (AT-54's whole risk was that
 * copy drifting).
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
 * One schema per field, parsed independently. Manifest §9.1 names a zod schema
 * with a per-field fallback as the replacement for v1's hand-written
 * `validateTheme` / `validateAccent` / whitelist merge; `safeParse` per field
 * rather than `.catch()` on a single object schema, because INV-21 also
 * requires *warning* on a present-but-invalid field, and a fallback that
 * silently substitutes cannot tell the two apart.
 */
const FIELD = {
  theme: z.enum(THEMES),
  accent: z.enum(ACCENTS),
  translucency: z.boolean(),
  previewLines: z.number().int().min(MIN_PREVIEW_LINES).max(MAX_PREVIEW_LINES),
  warnBeforeReveal: z.boolean(),
} as const;

/**
 * Parse a stored blob into prefs. Never throws.
 *
 * Present-but-invalid warns (the user's setting was discarded and they should
 * be able to find out why); absent defaults silently (a new field on an old
 * install is not a problem). Unknown keys are dropped by construction — the
 * result is built from the known key list, so nothing else can survive to be
 * re-persisted.
 */
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
      // The per-key schemas are 1:1 with the field types; the cast is the
      // price of iterating a heterogeneous record.
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

/** A storage that cannot throw. Private mode, a full quota and a disabled
 *  store all end up here, and none of them may take the window down. */
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
      // Only the data is persisted; the actions are not state.
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
      // INV-21: the stored blob is validated per field on the way in, so a
      // corrupt entry can never reach a component.
      merge: (persisted, current) => ({ ...current, ...parsePrefs(persisted) }),
    },
  ),
);

/** Selector helpers — subscribing to one field keeps a slider from re-rendering
 *  the history list. */
export const selectAppearance = (s: PrefsStore) => ({
  theme: s.theme,
  accent: s.accent,
  translucency: s.translucency,
});
