/* Generated from src/lib/themeBootstrapSource.ts. */
(function () {
  "use strict";

  var contract = {"storageKey":"copypaste.prefs","themes":["system","dark","light"],"colorThemes":["midnight","aurora","ember","graphite"],"defaults":{"theme":"system","colorTheme":"midnight","translucency":false},"systemThemeQuery":"(prefers-color-scheme: dark)","systemThemeFallback":"dark"};
  var unwrapPersistedPrefs = function unwrapPersistedPrefs(
  raw,
) {
  return typeof raw === "object" &&
    raw !== null &&
    !Array.isArray(raw) &&
    Object.prototype.hasOwnProperty.call(raw, "state")
    ? Reflect.get(raw, "state")
    : raw;
};
  var parseAppearanceFields = function parseAppearanceFields(raw, contract, onInvalid) {
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
  var translucencyAttribute = function translucencyAttribute(value) {
    return value ? "on" : "off";
  };
  var appearance = Object.assign({}, contract.defaults);

  try {
    var stored = localStorage.getItem(contract.storageKey);
    if (stored !== null) {
      appearance = parseAppearanceFields(
        unwrapPersistedPrefs(JSON.parse(stored)),
        contract,
        function () {},
      );
    }
  } catch (_) {}

  var colorScheme = appearance.theme;
  if (colorScheme === "system") {
    colorScheme = contract.systemThemeFallback;
    try {
      if (typeof window.matchMedia === "function") {
        colorScheme = window.matchMedia(contract.systemThemeQuery).matches
          ? "dark"
          : "light";
      }
    } catch (_) {}
  }

  var root = document.documentElement;
  root.dataset.colorScheme = colorScheme;
  root.dataset.mode = appearance.theme;
  root.dataset.theme = appearance.colorTheme;
  root.dataset.translucency = translucencyAttribute(appearance.translucency);
  root.dataset.themeBootstrapped = "1";
})();
