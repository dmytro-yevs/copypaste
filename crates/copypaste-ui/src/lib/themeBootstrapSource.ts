import {
  APPEARANCE_SERIALIZATION,
  parseAppearanceFields,
  translucencyAttribute,
  unwrapPersistedPrefs,
} from "./appearancePrefs.ts";

/**
 * `public/theme-bootstrap.js` is generated from this contract because CSP
 * requires a classic external script, while first paint cannot wait for an
 * imported application module (INV-22 / AT-49).
 *
 * Nothing lowers what comes out of here — `public/` is copied, not built — so
 * this and every function it embeds stay inside API 24's Chromium 53.
 * `check-webview-baseline.mjs` measures the copy in `dist`.
 */
export function themeBootstrapSource(): string {
  return `/* Generated from src/lib/themeBootstrapSource.ts. */
(function () {
  "use strict";

  var contract = ${JSON.stringify(APPEARANCE_SERIALIZATION)};
  var unwrapPersistedPrefs = ${unwrapPersistedPrefs.toString()};
  var parseAppearanceFields = ${parseAppearanceFields.toString()};
  var translucencyAttribute = ${translucencyAttribute.toString()};
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
`;
}
