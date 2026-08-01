import {
  APPEARANCE_SERIALIZATION,
  parseAppearanceFields,
  translucencyAttribute,
  translucencyStyle,
  unwrapPersistedPrefs,
} from "./appearancePrefs.ts";

/**
 * `public/theme-bootstrap.js` is generated from this contract because CSP
 * requires a classic external script, while first paint cannot wait for an
 * imported application module (INV-22 / AT-49).
 */
export function themeBootstrapSource(): string {
  return `/* Generated from src/lib/themeBootstrapSource.ts. */
(function () {
  "use strict";

  var contract = ${JSON.stringify(APPEARANCE_SERIALIZATION)};
  var unwrapPersistedPrefs = ${unwrapPersistedPrefs.toString()};
  var parseAppearanceFields = ${parseAppearanceFields.toString()};
  var translucencyAttribute = ${translucencyAttribute.toString()};
  var translucencyStyle = ${translucencyStyle.toString()};
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
`;
}
