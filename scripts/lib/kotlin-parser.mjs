/**
 * kotlin-parser.mjs — Shared Kotlin palette parser for CopyPaste design-token scripts.
 *
 * Parses Android Palette.kt (+ optionally Color.kt) and extracts per-palette color data
 * needed by both parity-check.mjs and check-contrast.mjs.
 *
 * Single source of truth for the palette name maps — previously duplicated across three
 * locations (ideColorsToPaletteName, auroraToPaletteName in parity-check.mjs and
 * ideColorsMap/auroraMap in parseKotlinPalettes in check-contrast.mjs).
 *
 * Exported:
 *   IDECOLORS_MAP   — { KotlinVarName → palette-key }   (IdeColors constructors)
 *   AURORA_MAP      — { KotlinVarName → palette-key }   (AuroraDef constructors)
 *   extractParenBlock(text, startIdx) → string | null
 *   resolveColorExpr(expr, symbols)  → [r,g,b] | null
 *   parseKotlin(kt, verbose?)        → { palette-key → { accent, success, … } }
 *   parseKotlinPalettes(kt)          → { palette-key → { accent, accentOn, bg0, panel } }
 */

import { ktHexToRgb } from "./color-utils.mjs";

// ---------------------------------------------------------------------------
// Palette name maps (single source of truth)
// ---------------------------------------------------------------------------

/**
 * Maps IdeColors Kotlin variable names to CSS/shared palette keys.
 * Used by both parseKotlin (parity check) and parseKotlinPalettes (contrast check).
 */
export const IDECOLORS_MAP = {
  GraphiteMistIdeColors: "graphite-mist",
  DeepSkyIdeColors:      "deep-sky",
  NordicCyanIdeColors:   "nordic-cyan",
  AuroraVioletIdeColors: "aurora-violet",
  AmberNightIdeColors:   "amber-night",
  CloudSilverIdeColors:  "cloud-silver",
  FrostBlueIdeColors:    "frost-blue",
  PorcelainIdeColors:    "porcelain",
  PearlGreyIdeColors:    "pearl-grey",
  LightIdeColors:        "_light-base",   // not a CSS palette; used for withAccent
  DarkIdeColors:         "liquid-blue",
};

/**
 * Maps AuroraDef Kotlin variable names to CSS/shared palette keys.
 */
export const AURORA_MAP = {
  GraphiteMistAurora: "graphite-mist",
  LiquidBlueAurora:   "liquid-blue",
  DeepSkyAurora:      "deep-sky",
  NordicCyanAurora:   "nordic-cyan",
  AuroraVioletAurora: "aurora-violet",
  AmberNightAurora:   "amber-night",
  CloudSilverAurora:  "cloud-silver",
  FrostBlueAurora:    "frost-blue",
  PorcelainAurora:    "porcelain",
  PearlGreyAurora:    "pearl-grey",
};

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

/** Extract everything between the opening paren at `startIdx` and its matching close. */
export function extractParenBlock(text, startIdx) {
  let depth = 0;
  let i = startIdx;
  let start = -1;
  while (i < text.length) {
    if (text[i] === "(") {
      if (depth === 0) start = i + 1;
      depth++;
    } else if (text[i] === ")") {
      depth--;
      if (depth === 0) return text.slice(start, i);
    }
    i++;
  }
  return null;
}

/**
 * Resolve a Kotlin color expression to [r,g,b].
 * Handles:
 *   GmAccent                             → symbol lookup
 *   Color(0xFF9DB7DF)                    → direct hex
 *   Color(0xFF9DB7DF).copy(alpha = …)   → hex, ignore .copy()
 *   GmAccent.copy(alpha = …)             → symbol lookup
 *   Color.White                          → [255, 255, 255]
 *   Color.Black                          → [0, 0, 0]
 *   lightText(R, G, B, A)               → direct R,G,B
 *   darkLine(…) / darkText(…) / etc.    → null (alpha-blended, skip for comparison)
 */
export function resolveColorExpr(expr, symbols) {
  // Color.White / Color.Black — Kotlin named color constants
  if (/^\s*Color\.White\s*$/.test(expr)) return [255, 255, 255];
  if (/^\s*Color\.Black\s*$/.test(expr)) return [0, 0, 0];

  // Named symbol (with optional .copy(...))
  const symM = expr.match(/^(\w+)(?:\.copy\(.*\))?$/);
  if (symM && symbols[symM[1]]) return symbols[symM[1]];

  // Color(0xFFRRGGBB) or Color(0xRRGGBB) with optional .copy(...)
  const hexM = expr.match(/Color\(0x([0-9a-fA-F]{6,8})\)/);
  if (hexM) return ktHexToRgb("0x" + hexM[1]);

  // lightText(R, G, B, A) → [R, G, B]
  const ltM = expr.match(/lightText\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,/);
  if (ltM) return [parseInt(ltM[1]), parseInt(ltM[2]), parseInt(ltM[3])];

  // darkText, darkTextBlue, darkIcon, darkLine — all are white-ish with alpha
  // We can't extract a useful hex from these for parity comparison, so skip
  if (/dark(Text|TextBlue|Icon|Line)\(/.test(expr)) return null;

  return null;
}

// ---------------------------------------------------------------------------
// Full Kotlin parser (for parity-check.mjs — accent, bg, semantic fields)
// ---------------------------------------------------------------------------

/**
 * Parse Palette.kt (combined with Color.kt if needed) and return a map:
 *   palette-key → { accent, accentOn, success, warning, danger, info, violet,
 *                   bg, panel, elevated, bg0, bg1, bg2 }
 *
 * Strategy:
 *   1. Build symbol table of all Color() constant vals.
 *   2. Parse IdeColors(...) constructors for accent/semantic fields.
 *   3. Parse AuroraDef(...) constructors for bg0/bg1/bg2.
 *   4. Handle LiquidBlue via DarkIdeColors fallback.
 *
 * @param {string} kt    — combined Kotlin source text (Palette.kt + Color.kt)
 * @param {boolean} verbose — if true, logs symbol table count to console
 */
export function parseKotlin(kt, verbose = false) {
  const result = {};

  // ── Pass 1: build symbol table of all named Color constants ─────────────
  // Matches: private val GmAccent = Color(0xFF9DB7DF) or val GmAccent = Color(0xFF...)
  const colorValRe = /(?:private\s+)?val\s+(\w+)\s*=\s*Color\(0x([0-9a-fA-F]{6,8})\)/g;
  const symbols = {};
  let m;
  while ((m = colorValRe.exec(kt)) !== null) {
    const name = m[1];
    const rgb = ktHexToRgb("0x" + m[2]);
    if (rgb) symbols[name] = rgb;
  }

  if (verbose) {
    console.log("  [kt] Symbol table:", Object.keys(symbols).length, "named Color vals");
  }

  // ── Pass 2: parse IdeColors(...) constructors ────────────────────────────
  const ideColorsRe = /val\s+(\w+IdeColors)\s*=\s*IdeColors\s*\(/g;

  while ((m = ideColorsRe.exec(kt)) !== null) {
    const varName = m[1]; // e.g. "GraphiteMistIdeColors"
    const paletteName = IDECOLORS_MAP[varName];
    if (!paletteName) continue;

    const startIdx = m.index + m[0].length - 1; // position of the opening (
    const block = extractParenBlock(kt, startIdx);
    if (!block) continue;

    const fields = _parseIdeColorsFields(block, symbols);
    if (!result[paletteName]) result[paletteName] = {};
    Object.assign(result[paletteName], fields);
  }

  // ── Pass 3: parse AuroraDef(...) constructors for bg0/bg1/bg2 ────────────
  const auroraRe = /val\s+(\w+Aurora)\s*=\s*AuroraDef\s*\(/g;

  while ((m = auroraRe.exec(kt)) !== null) {
    const varName = m[1]; // e.g. "GraphiteMistAurora"
    const paletteName = AURORA_MAP[varName];
    if (!paletteName) continue;

    const startIdx = m.index + m[0].length - 1;
    const block = extractParenBlock(kt, startIdx);
    if (!block) continue;

    const fields = _parseAuroraFields(block, symbols);
    if (!result[paletteName]) result[paletteName] = {};
    Object.assign(result[paletteName], fields);
  }

  // ── Pass 4: handle LiquidBlue (uses DarkIdeColors — not a named IdeColors block)
  const darkM = kt.match(/val\s+DarkIdeColors\s*=\s*IdeColors\s*\(/);
  if (darkM) {
    const startIdx = darkM.index + darkM[0].length - 1;
    const block = extractParenBlock(kt, startIdx);
    if (block) {
      const fields = _parseIdeColorsFields(block, symbols);
      if (!result["liquid-blue"]) result["liquid-blue"] = {};
      // Only fill in if not already set
      for (const [k, v] of Object.entries(fields)) {
        if (!result["liquid-blue"][k]) result["liquid-blue"][k] = v;
      }
    }
  }

  // bg0/bg1/bg2 for liquid-blue from LiquidBlueAurora (already handled in Pass 3)
  if (!result["liquid-blue"]) result["liquid-blue"] = {};

  return result;
}

/**
 * Parse IdeColors(...) field list, resolving Color references.
 * Returns { accent, accentOn, success, warning, danger, info, violet,
 *           bg, panel, elevated } as [r,g,b].
 */
function _parseIdeColorsFields(block, symbols) {
  const out = {};
  const fields = ["accent", "accentOn", "success", "warning", "danger", "info", "violet", "bg", "panel", "elevated"];

  for (const field of fields) {
    const re = new RegExp(`\\b${field}\\s*=\\s*([^,\\n]+)`);
    const m = block.match(re);
    if (!m) continue;
    const expr = m[1].trim();
    const rgb = resolveColorExpr(expr, symbols);
    if (rgb) out[field] = rgb;
  }

  return out;
}

/**
 * Parse AuroraDef(...) fields for bg0/bg1/bg2.
 */
function _parseAuroraFields(block, symbols) {
  const out = {};
  for (const field of ["bg0", "bg1", "bg2"]) {
    const re = new RegExp(`\\b${field}\\s*=\\s*([^,\\n]+)`);
    const m = block.match(re);
    if (!m) continue;
    const expr = m[1].trim();
    const rgb = resolveColorExpr(expr, symbols);
    if (rgb) out[field] = rgb;
  }
  return out;
}

// ---------------------------------------------------------------------------
// Contrast-check Kotlin parser (for check-contrast.mjs — accent/accentOn/panel/bg0)
// ---------------------------------------------------------------------------

/**
 * Parse Android Palette.kt + Color.kt for accent/accentOn/panel/bg0 per palette.
 * Returns: { palette-key → { accent, accentOn, bg, panel, bg0 } }
 *
 * This is a lighter parse than parseKotlin — it only extracts the fields needed
 * for WCAG contrast checking.
 */
export function parseKotlinPalettes(kt) {
  const result = {};

  // Build symbol table of named Color constants
  const colorValRe = /(?:private\s+)?val\s+(\w+)\s*=\s*Color\(0x([0-9a-fA-F]{6,8})\)/g;
  const symbols = {};
  let m;
  while ((m = colorValRe.exec(kt)) !== null) {
    symbols[m[1]] = ktHexToRgb("0x" + m[2]);
  }

  function resolveExpr(expr) {
    if (!expr) return null;
    expr = expr.trim();
    return resolveColorExpr(expr, symbols);
  }

  // Parse IdeColors constructors
  const ideColorsRe = /val\s+(\w+IdeColors)\s*=\s*IdeColors\s*\(/g;

  while ((m = ideColorsRe.exec(kt)) !== null) {
    const varName = m[1];
    const palKey = IDECOLORS_MAP[varName];
    if (!palKey) continue;

    const startIdx = m.index + m[0].length - 1;
    const block = extractParenBlock(kt, startIdx);
    if (!block) continue;

    const fields = ["accent", "accentOn", "bg", "panel"];
    const entry = {};
    for (const field of fields) {
      const re = new RegExp(`\\b${field}\\s*=\\s*([^,\\n]+)`);
      const fm = block.match(re);
      if (!fm) continue;
      const rgb = resolveExpr(fm[1].trim());
      if (rgb) entry[field] = rgb;
    }

    if (!result[palKey]) result[palKey] = {};
    Object.assign(result[palKey], entry);
  }

  // AuroraDef for bg0 (surface background for contrast checks)
  const auroraRe = /val\s+(\w+Aurora)\s*=\s*AuroraDef\s*\(/g;
  while ((m = auroraRe.exec(kt)) !== null) {
    const palKey = AURORA_MAP[m[1]];
    if (!palKey) continue;
    const startIdx = m.index + m[0].length - 1;
    const block = extractParenBlock(kt, startIdx);
    if (!block) continue;
    const bg0m = block.match(/\bbg0\s*=\s*([^,\n]+)/);
    if (bg0m) {
      const rgb = resolveExpr(bg0m[1].trim());
      if (rgb) {
        if (!result[palKey]) result[palKey] = {};
        result[palKey].bg0 = rgb;
      }
    }
  }

  return result;
}
