export const APPEARANCE_SERIALIZATION = {
  storageKey: "copypaste.prefs",
  themes: ["system", "dark", "light"] as const,
  accents: ["indigo", "blue", "teal", "green", "amber", "rose"] as const,
  defaults: {
    theme: "system" as const,
    accent: "indigo" as const,
    translucency: false,
  },
  systemThemeQuery: "(prefers-color-scheme: dark)",
  systemThemeFallback: "dark" as const,
} as const;

export type ThemePref = (typeof APPEARANCE_SERIALIZATION.themes)[number];
export type Accent = (typeof APPEARANCE_SERIALIZATION.accents)[number];
export type AppearanceField = "theme" | "accent" | "translucency";

export interface AppearancePrefs {
  theme: ThemePref;
  accent: Accent;
  translucency: boolean;
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
      if (typeof translucency === "boolean") {
        appearance.translucency = translucency;
      } else {
        onInvalid("translucency");
      }
    }

    return appearance;
  };

type TranslucencyAttribute = (value: boolean) => "on" | "off";

export const translucencyAttribute: TranslucencyAttribute =
  function translucencyAttribute(value) {
    return value ? "on" : "off";
  };
