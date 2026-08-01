/* Generated from src/lib/themeBootstrapSource.ts. */
(function () {
  "use strict";

  var contract = {"storageKey":"copypaste.prefs","themes":["system","dark","light"],"accents":["system","indigo","blue","teal","green","amber","rose"],"defaults":{"theme":"system","accent":"indigo","translucency":0},"systemThemeQuery":"(prefers-color-scheme: dark)","systemThemeFallback":"dark"};
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
      appearance.translucency = translucency ? 100 : 0;
    } else {
      onInvalid("translucency");
    }
  }

  return appearance;
};
  var translucencyAttribute = function translucencyAttribute(value) {
  return value > 0 ? "on" : "off";
};
  var translucencyStyle = function translucencyStyle(value) {
  const level = Math.min(100, Math.max(0, value));
  return {
    "--translucency-chrome-opacity": `${100 - level * 0.28}%`,
    "--translucency-app-opacity": `${100 - level * 0.16}%`,
    "--translucency-blur": `${Math.round(level * 0.2)}px`,
    "--translucency-saturation": `${100 + Math.round(level * 0.8)}%`,
  };
};
  var appearance = { ...contract.defaults };

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

  var resolvedTheme = appearance.theme;
  if (resolvedTheme === "system") {
    resolvedTheme = contract.systemThemeFallback;
    try {
      if (typeof window.matchMedia === "function") {
        resolvedTheme = window.matchMedia(contract.systemThemeQuery).matches
          ? "dark"
          : "light";
      }
    } catch (_) {}
  }

  var root = document.documentElement;
  root.dataset.theme = resolvedTheme;
  root.dataset.themePref = appearance.theme;
  root.dataset.accent = appearance.accent;
  root.dataset.translucency = translucencyAttribute(appearance.translucency);
  var translucencyValues = translucencyStyle(appearance.translucency);
  for (var name in translucencyValues) {
    root.style.setProperty(name, translucencyValues[name]);
  }
  root.dataset.themeBootstrapped = "1";
})();
