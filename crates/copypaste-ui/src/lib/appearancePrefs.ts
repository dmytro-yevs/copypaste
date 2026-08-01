export const APPEARANCE_SERIALIZATION = {
  storageKey: "copypaste.prefs",
  themes: ["system", "dark", "light"] as const,
  accents: ["system", "indigo", "blue", "teal", "green", "amber", "rose"] as const,
  defaults: {
    theme: "system" as const,
    accent: "indigo" as const,
    translucency: 0,
  },
  systemThemeQuery: "(prefers-color-scheme: dark)",
  systemThemeFallback: "dark" as const,
} as const;

export type ThemePref = (typeof APPEARANCE_SERIALIZATION.themes)[number];
export type Accent = (typeof APPEARANCE_SERIALIZATION.accents)[number];
export type AppearanceField = "theme" | "accent" | "translucency";
export type Translucency = number;

export const TRANSLUCENCY_MIN = 0;
export const TRANSLUCENCY_MAX = 100;

export interface AppearancePrefs {
  theme: ThemePref;
  accent: Accent;
  translucency: Translucency;
}

export interface AppearanceSerialization {
  storageKey: string;
  themes: readonly ThemePref[];
  accents: readonly Accent[];
  defaults: AppearancePrefs;
  systemThemeQuery: string;
  systemThemeFallback: "dark" | "light";
}

type UnwrapPersistedPrefs = (raw: unknown) => unknown;

/** Zustand persist wraps preferences as `{ state, version }`. */
export const unwrapPersistedPrefs: UnwrapPersistedPrefs = function unwrapPersistedPrefs(
  raw,
) {
  return typeof raw === "object" &&
    raw !== null &&
    !Array.isArray(raw) &&
    Object.prototype.hasOwnProperty.call(raw, "state")
    ? Reflect.get(raw, "state")
    : raw;
};

type ParseAppearanceFields = (
  raw: unknown,
  contract: AppearanceSerialization,
  onInvalid: (field: AppearanceField) => void,
) => AppearancePrefs;

export const parseAppearanceFields: ParseAppearanceFields =
  function parseAppearanceFields(raw, contract, onInvalid) {
    const appearance = { ...contract.defaults };
    if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
      return appearance;
    }

    if (Object.prototype.hasOwnProperty.call(raw, "theme")) {
      const theme = Reflect.get(raw, "theme");
      if (contract.themes.some((candidate) => candidate === theme)) {
        Reflect.set(appearance, "theme", theme);
      } else {
        onInvalid("theme");
      }
    }
    if (Object.prototype.hasOwnProperty.call(raw, "accent")) {
      const accent = Reflect.get(raw, "accent");
      if (contract.accents.some((candidate) => candidate === accent)) {
        Reflect.set(appearance, "accent", accent);
      } else {
        onInvalid("accent");
      }
    }
    if (Object.prototype.hasOwnProperty.call(raw, "translucency")) {
      const translucency = Reflect.get(raw, "translucency");
      if (
        typeof translucency === "number" &&
        Number.isInteger(translucency) &&
        translucency >= 0 &&
        translucency <= 100
      ) {
        appearance.translucency = translucency;
      } else if (typeof translucency === "boolean") {
        // v2 alpha stored this as a switch. Keep an existing user's enabled
        // frost at the former visual strength when the control becomes a range.
        appearance.translucency = translucency ? 100 : 0;
      } else {
        onInvalid("translucency");
      }
    }

    return appearance;
  };

type TranslucencyAttribute = (value: Translucency) => "on" | "off";

export const translucencyAttribute: TranslucencyAttribute =
  function translucencyAttribute(value) {
    return value > 0 ? "on" : "off";
  };

export function translucencyStyle(value: Translucency): Record<string, string> {
  const level = Math.min(100, Math.max(0, value));
  return {
    "--translucency-chrome-opacity": `${100 - level * 0.28}%`,
    "--translucency-app-opacity": `${100 - level * 0.16}%`,
    "--translucency-blur": `${Math.round(level * 0.2)}px`,
    "--translucency-saturation": `${100 + Math.round(level * 0.8)}%`,
  };
}
