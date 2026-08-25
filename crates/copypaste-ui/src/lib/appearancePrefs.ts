export const APPEARANCE_SERIALIZATION = {
  storageKey: "copypaste.prefs",
  themes: ["system", "dark", "light"] as const,
  colorThemes: ["midnight", "aurora", "ember", "graphite"] as const,
  defaults: {
    theme: "system" as const,
    colorTheme: "midnight" as const,
    translucency: false,
  },
  systemThemeQuery: "(prefers-color-scheme: dark)",
  systemThemeFallback: "dark" as const,
} as const;

export type ThemePref = (typeof APPEARANCE_SERIALIZATION.themes)[number];
export type ColorTheme = (typeof APPEARANCE_SERIALIZATION.colorThemes)[number];
export type AppearanceField = "theme" | "colorTheme" | "translucency";
export type Translucency = boolean;

export interface AppearancePrefs {
  theme: ThemePref;
  colorTheme: ColorTheme;
  translucency: Translucency;
}

export interface AppearanceSerialization {
  storageKey: string;
  themes: readonly ThemePref[];
  colorThemes: readonly ColorTheme[];
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

// `Object.assign` rather than a spread below: `themeBootstrapSource` embeds
// this function's own text in a classic script no build step lowers, and object
// spread is Chromium 60 — a syntax error on API 24's WebView, which took the
// whole appearance bootstrap down with it.
export const parseAppearanceFields: ParseAppearanceFields =
  function parseAppearanceFields(raw, contract, onInvalid) {
    const appearance = Object.assign({}, contract.defaults);
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
    if (Object.prototype.hasOwnProperty.call(raw, "colorTheme")) {
      const colorTheme = Reflect.get(raw, "colorTheme");
      if (contract.colorThemes.some((candidate) => candidate === colorTheme)) {
        Reflect.set(appearance, "colorTheme", colorTheme);
      } else {
        onInvalid("colorTheme");
      }
    } else if (Object.prototype.hasOwnProperty.call(raw, "accent")) {
      const legacyAccent = Reflect.get(raw, "accent");
      if (legacyAccent === "teal" || legacyAccent === "green") {
        appearance.colorTheme = "aurora";
      } else if (legacyAccent === "amber" || legacyAccent === "rose") {
        appearance.colorTheme = "ember";
      } else if (
        legacyAccent === "system" ||
        legacyAccent === "indigo" ||
        legacyAccent === "blue"
      ) {
        appearance.colorTheme = "midnight";
      }
    }
    if (Object.prototype.hasOwnProperty.call(raw, "translucency")) {
      const translucency = Reflect.get(raw, "translucency");
      if (typeof translucency === "boolean") {
        appearance.translucency = translucency;
      } else if (
        typeof translucency === "number" &&
        Number.isFinite(translucency) &&
        translucency >= 0 &&
        translucency <= 100
      ) {
        // The intensity control was never a distinct visual system: every
        // non-zero legacy value maps to the maintained frosted token set.
        appearance.translucency = translucency > 0;
      } else {
        onInvalid("translucency");
      }
    }

    return appearance;
  };

type TranslucencyAttribute = (value: Translucency) => "on" | "off";

export const translucencyAttribute: TranslucencyAttribute =
  function translucencyAttribute(value) {
    return value ? "on" : "off";
  };
