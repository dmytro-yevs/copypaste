/**
 * css-parser.mjs — CSS design-token parser for CopyPaste parity checking.
 *
 * Parses `html[data-palette="X"]` and `:root` blocks from index.css and
 * extracts per-palette × per-theme color token values.
 *
 * Exported:
 *   parseCss(css) → { palette-key → { "dark"|"light" → { tokenName → [r,g,b] } } }
 */

import { hexToRgb, tripletToRgb } from "./color-utils.mjs";

/**
 * Parse index.css and return a nested map:
 *   paletteKey → themeKey → tokenName → [r,g,b]
 *
 * themeKey is "dark" for html[data-palette="X"] and "light" for
 * html[data-theme="light"][data-palette="X"].
 * Neutral dark-mode tokens (:root) are stored under palette "" theme "dark".
 * Light neutral tokens (:root[data-theme="light"]) under palette "" theme "light".
 */
export function parseCss(css) {
  const result = {};

  // Strip CSS block comments (/* ... */) before parsing so comment text doesn't
  // bleed into selector accumulation (e.g. a comment referencing data-palette="X"
  // would otherwise corrupt the selector match for the following rule).
  css = css.replace(/\/\*[\s\S]*?\*\//g, " ");

  const lines = css.split("\n");
  let currentSelectors = [];
  let inBlock = false;
  let depth = 0;
  let blockLines = [];

  const flushBlock = () => {
    if (currentSelectors.length === 0) return;
    const blockText = blockLines.join("\n");
    for (const sel of currentSelectors) {
      const palette = extractPalette(sel);
      const theme = sel.includes('data-theme="light"') ? "light" : "dark";
      const key = palette;
      if (!result[key]) result[key] = {};
      if (!result[key][theme]) result[key][theme] = {};
      parseTokenBlock(blockText, result[key][theme]);
    }
    blockLines = [];
    currentSelectors = [];
  };

  let pendingSelector = "";
  for (const line of lines) {
    const trimmed = line.trim();
    if (!inBlock) {
      // Accumulate selector lines (may span multiple lines before {)
      pendingSelector += " " + trimmed;
      if (trimmed.includes("{")) {
        const selectorPart = pendingSelector.slice(
          0,
          pendingSelector.lastIndexOf("{")
        ).trim();
        if (_isRelevantSelector(selectorPart)) {
          currentSelectors = [selectorPart];
        } else {
          currentSelectors = [];
        }
        inBlock = true;
        depth = 1;
        blockLines = [];
        pendingSelector = "";
      }
    } else {
      depth += (line.match(/{/g) || []).length;
      depth -= (line.match(/}/g) || []).length;
      if (depth <= 0) {
        inBlock = false;
        flushBlock();
        pendingSelector = "";
      } else {
        blockLines.push(line);
      }
    }
  }

  return result;
}

function _isRelevantSelector(sel) {
  // :root, :root[data-theme="light"], html[data-palette="X"], html[data-theme="light"][data-palette="X"]
  return (
    /^:root(\[data-theme="light"\])?$/.test(sel.trim()) ||
    /html\[data-palette=/.test(sel) ||
    /html\[data-theme="light"\]\[data-palette=/.test(sel)
  );
}

function extractPalette(sel) {
  const m = sel.match(/data-palette="([^"]+)"/);
  return m ? m[1] : ""; // "" = root (no palette)
}

/**
 * Extract token values from a CSS block body (text between { and }).
 * Writes into `out` (mutates in place).
 *
 * Tokens extracted:
 *   --ide-accent-rgb, --ide-success-rgb, --ide-warning-rgb, --ide-danger-rgb,
 *   --ide-info-rgb, --ide-sky-rgb, --ide-violet-rgb, --ide-bg-rgb, --ide-panel-rgb,
 *   --ide-elevated-rgb, --ide-text-rgb, --ide-dim-rgb, --ide-faint-rgb → triplet [r,g,b]
 *   --bg-0, --bg-1, --bg-2 → hex [r,g,b]
 *   --accent → hex [r,g,b] as "accent-liquid"
 *   --on-accent → hex [r,g,b] as "on-accent"
 */
function parseTokenBlock(text, out) {
  const rgbVars = {
    "--ide-accent-rgb":    "accent",
    "--ide-success-rgb":   "success",
    "--ide-warning-rgb":   "warning",
    "--ide-danger-rgb":    "danger",
    "--ide-info-rgb":      "info",
    "--ide-sky-rgb":       "sky",
    "--ide-violet-rgb":    "violet",
    "--ide-bg-rgb":        "ide-bg",
    "--ide-panel-rgb":     "ide-panel",
    "--ide-elevated-rgb":  "ide-elevated",
    "--ide-text-rgb":      "ide-text",
    "--ide-dim-rgb":       "ide-dim",
    "--ide-faint-rgb":     "ide-faint",
  };
  for (const [varName, tokenName] of Object.entries(rgbVars)) {
    const re = new RegExp(
      varName.replace(/[-[\]]/g, "\\$&") + "\\s*:\\s*([\\d]+\\s+[\\d]+\\s+[\\d]+)"
    );
    const m = text.match(re);
    if (m) {
      const rgb = tripletToRgb(m[1]);
      if (rgb) out[tokenName] = rgb;
    }
  }

  const bgVars = { "--bg-0": "bg0", "--bg-1": "bg1", "--bg-2": "bg2" };
  for (const [varName, tokenName] of Object.entries(bgVars)) {
    const re = new RegExp(
      varName.replace(/[-[\]]/g, "\\$&") + "\\s*:\\s*(#[0-9a-fA-F]{3,6})"
    );
    const m = text.match(re);
    if (m) {
      const rgb = hexToRgb(m[1]);
      if (rgb) out[tokenName] = rgb;
    }
  }

  // --accent: #hex (liquid accent)
  const accentM = text.match(/--accent\s*:\s*(#[0-9a-fA-F]{3,6})/);
  if (accentM) {
    const rgb = hexToRgb(accentM[1]);
    if (rgb) out["accent-liquid"] = rgb;
  }

  // --on-accent: #hex — foreground color on accent-colored buttons/chips.
  // Critical for contrast parity: drift here breaks button legibility.
  const onAccentM = text.match(/--on-accent\s*:\s*(#[0-9a-fA-F]{3,6})/);
  if (onAccentM) {
    const rgb = hexToRgb(onAccentM[1]);
    if (rgb) out["on-accent"] = rgb;
  }
}
