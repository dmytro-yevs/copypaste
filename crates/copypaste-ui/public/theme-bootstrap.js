/*
 * Pre-paint appearance bootstrap (redesign, Slice 1 — design.md Decision 4).
 *
 * WHY AN EXTERNAL CLASSIC SCRIPT: the packaged Tauri CSP is `script-src 'self'`
 * (no 'unsafe-inline', no nonce/hash), so an INLINE <script> is blocked. This
 * file is emitted verbatim from `public/` to a stable un-hashed path and loaded
 * via `<script src="./theme-bootstrap.js"></script>` (RELATIVE — safe under
 * Tauri's packaged asset protocol) BEFORE the module entry in both index.html
 * and popup.html, so the persisted theme/accent/translucency is applied to
 * <html> before the first content paint (no default-theme flash).
 *
 * CONSTRAINTS (do not violate — enforced by themeBootstrap.test.ts):
 *   - Synchronous classic script. NO import / eval / Function.
 *   - The KEY, the three defaults, the allowed enum values, and the
 *     translucency→"on"/"off" mapping MUST stay in exact parity with
 *     src/lib/theme/prefsSchema.ts (they are duplicated here because this asset
 *     cannot import). The parity test fails loudly on any drift (task 1.14).
 *   - try/catch around storage so a localStorage exception (private mode) falls
 *     back to defaults instead of throwing before app code runs.
 */
(function () {
  "use strict";

  var KEY = "copypaste-ui-prefs-v4";
  var THEMES = ["dark", "light"];
  var ACCENTS = ["indigo", "blue", "teal", "green", "amber", "rose"];
  var DEFAULT_THEME = "dark";
  var DEFAULT_ACCENT = "indigo";
  var DEFAULT_TRANSLUCENCY = true;

  var theme = DEFAULT_THEME;
  var accent = DEFAULT_ACCENT;
  var translucency = DEFAULT_TRANSLUCENCY;

  try {
    var raw = localStorage.getItem(KEY);
    if (raw) {
      var parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object") {
        // Validate each axis independently — one invalid field never discards
        // the others (mirrors the store's per-field validation).
        if (THEMES.indexOf(parsed.theme) !== -1) {
          theme = parsed.theme;
        }
        if (ACCENTS.indexOf(parsed.accent) !== -1) {
          accent = parsed.accent;
        }
        if (typeof parsed.translucency === "boolean") {
          translucency = parsed.translucency;
        }
      }
    }
  } catch (e) {
    // Missing/malformed JSON or a storage-access exception → keep defaults.
  }

  var el = document.documentElement;
  el.dataset.theme = theme;
  el.dataset.accent = accent;
  el.dataset.translucency = translucency ? "on" : "off";

  // Ordering marker (design.md task 1.15 / round-5 M1): set synchronously so the
  // React module entry can assert the bootstrap ALREADY ran at startup, proving
  // script order without pixel-level paint timing.
  el.dataset.themeBootstrapped = "1";
})();
