#!/usr/bin/env node
/**
 * parity-check.mjs — Web ↔ Android design-token parity checker
 *
 * Parses design tokens from:
 *   • crates/copypaste-ui/src/index.css   (html[data-palette="X"] blocks)
 *   • android/.../ui/theme/Palette.kt     (per-palette Color() / private val ramps)
 *
 * Compares per-palette × per-theme accent, bg0/bg1/bg2, and key semantic
 * (success/warning/danger) values, then exits non-zero if any value diverges
 * beyond TOLERANCE per RGB channel.
 *
 * Assumptions / known intentional deltas:
 *   • glass-opacity differs by design (web 0.40 uses Tauri vibrancy; Android 0.64
 *     uses direct RenderEffect). Excluded from comparison.
 *   • --ide-sky-rgb exists in CSS per-palette but Android IdeColors has no `sky` field
 *     (it uses `info` for both URL/sky and info badge color). Sky is parsed from CSS
 *     but not included in PALETTE_CHECKS to avoid spurious WARN noise. If a sky field
 *     is added to Android IdeColors, add "sky" to DARK_TOKENS/LIGHT_TOKENS and add a
 *     CSS_TO_KT_KEY entry { "sky": "sky" }.
 *   • Web CSS uses per-channel space-separated rgb triplets ("R G B") inside
 *     --ide-*-rgb vars AND full hex values for --accent/--bg-*.
 *   • Android uses Color(0xFFRRGGBB) hex literals.
 *   • Light-palette light-mode accents (cloud-silver etc.) come from the
 *     html[data-theme="light"][data-palette="X"] blocks in CSS and from the
 *     named IdeColors objects in Palette.kt.
 *   • LiquidBlue IdeColors in Android == DarkIdeColors (not a named palette block);
 *     the check falls back gracefully with a warning if data is missing.
 *
 * Usage:
 *   node scripts/parity-check.mjs            # from repo root
 *   node scripts/parity-check.mjs --verbose  # print all parsed values
 *
 * Exit codes:
 *   0  all comparisons within tolerance
 *   1  one or more token values diverge beyond tolerance
 */

import { readFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dir = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dir, "..");

const CSS_PATH = resolve(ROOT, "crates/copypaste-ui/src/index.css");
const KT_PATH = resolve(
  ROOT,
  "android/app/src/main/java/com/copypaste/android/ui/theme/Palette.kt"
);
// Color.kt holds DarkIdeColors (used for liquid-blue) and LightIdeColors
const KT_COLOR_PATH = resolve(
  ROOT,
  "android/app/src/main/java/com/copypaste/android/ui/theme/Color.kt"
);

const VERBOSE = process.argv.includes("--verbose");
const TOLERANCE = 5; // per RGB channel (0–255)

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Parse a CSS hex colour (#RRGGBB or #RGB) to [r,g,b]. */
function hexToRgb(hex) {
  hex = hex.replace(/^#/, "");
  if (hex.length === 3) hex = hex.split("").map((c) => c + c).join("");
  if (hex.length !== 6) return null;
  const n = parseInt(hex, 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Parse "R G B" space-separated triplet (CSS rgb channel var) to [r,g,b]. */
function tripletToRgb(s) {
  const parts = s.trim().split(/\s+/);
  if (parts.length !== 3) return null;
  return parts.map(Number);
}

/** Parse Android 0xFFRRGGBB or 0xRRGGBB hex to [r,g,b]. */
function ktHexToRgb(hex) {
  // strip 0x prefix and optional alpha byte (8-digit = AARRGGBB)
  hex = hex.replace(/^0[xX]/, "");
  if (hex.length === 8) hex = hex.slice(2); // drop AA
  if (hex.length !== 6) return null;
  const n = parseInt(hex, 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Check if two [r,g,b] triplets are within TOLERANCE per channel. */
function withinTolerance(a, b, tol = TOLERANCE) {
  if (!a || !b) return false;
  return a.every((v, i) => Math.abs(v - b[i]) <= tol);
}

/** Format [r,g,b] as #RRGGBB. */
function fmtRgb(rgb) {
  if (!rgb) return "(null)";
  return (
    "#" +
    rgb
      .map((c) => c.toString(16).padStart(2, "0").toUpperCase())
      .join("")
  );
}

// ---------------------------------------------------------------------------
// CSS parser
// ---------------------------------------------------------------------------

/**
 * Parse index.css and return a nested map:
 *   paletteKey → themeKey → tokenName → [r,g,b]
 *
 * themeKey is "dark" for html[data-palette="X"] and "light" for
 * html[data-theme="light"][data-palette="X"].
 * Neutral dark-mode tokens (`:root`) are stored under palette "" theme "dark".
 * Light neutral tokens (`:root[data-theme="light"]`) under palette "" theme "light".
 */
function parseCss(css) {
  const result = {};

  // Strip CSS block comments (/* ... */) before parsing so comment text doesn't
  // bleed into selector accumulation (e.g. a comment referencing data-palette="X"
  // would otherwise corrupt the selector match for the following rule).
  // Using a non-greedy match so we don't collapse multiple comments into one.
  css = css.replace(/\/\*[\s\S]*?\*\//g, " ");

  // Split into blocks by selector
  // Strategy: find all top-level rules by scanning for { ... } blocks
  // We process line-by-line since CSS is well-formed here.
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
        // Extract the selector part before the first {
        const selectorPart = pendingSelector.slice(
          0,
          pendingSelector.lastIndexOf("{")
        ).trim();
        if (isRelevantSelector(selectorPart)) {
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

function isRelevantSelector(sel) {
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
 * We look for:
 *   --ide-accent-rgb:   R G B   → "accent" token
 *   --ide-success-rgb:  R G B   → "success" token
 *   --ide-warning-rgb:  R G B   → "warning" token
 *   --ide-danger-rgb:   R G B   → "danger" token
 *   --bg-0: #hex  → "bg0" token
 *   --bg-1: #hex  → "bg1" token
 *   --bg-2: #hex  → "bg2" token
 *   --accent: #hex → "accent-liquid" (liquid accent, for palette blocks)
 *   --on-accent: #hex → "on-accent" token (text color drawn on accent buttons/chips)
 */
function parseTokenBlock(text, out) {
  // --ide-*-rgb: R G B (may have trailing semicolon or other tokens on same line)
  const rgbVars = {
    "--ide-accent-rgb": "accent",
    "--ide-success-rgb": "success",
    "--ide-warning-rgb": "warning",
    "--ide-danger-rgb": "danger",
    "--ide-info-rgb": "info",
    "--ide-sky-rgb": "sky",
    "--ide-violet-rgb": "violet",
    "--ide-bg-rgb": "ide-bg",
    "--ide-panel-rgb": "ide-panel",
    "--ide-elevated-rgb": "ide-elevated",
    "--ide-text-rgb": "ide-text",
    "--ide-dim-rgb": "ide-dim",
    "--ide-faint-rgb": "ide-faint",
  };
  for (const [varName, tokenName] of Object.entries(rgbVars)) {
    // Match: --ide-accent-rgb:   61 139 255  (up to semicolon or comment)
    const re = new RegExp(
      varName.replace(/[-[\]]/g, "\\$&") + "\\s*:\\s*([\\d]+\\s+[\\d]+\\s+[\\d]+)"
    );
    const m = text.match(re);
    if (m) {
      const rgb = tripletToRgb(m[1]);
      if (rgb) out[tokenName] = rgb;
    }
  }

  // --bg-0/1/2: #hex  (background canvas)
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

  // --accent: #hex (liquid accent — use as "accent-liquid" to distinguish from ide-accent-rgb)
  const accentM = text.match(/--accent\s*:\s*(#[0-9a-fA-F]{3,6})/);
  if (accentM) {
    const rgb = hexToRgb(accentM[1]);
    if (rgb) out["accent-liquid"] = rgb;
  }

  // --on-accent: #hex — the foreground color drawn on accent-colored buttons/chips.
  // This is critical for contrast parity: Android uses accentOn (Color.White / dark teal /
  // very dark amber) while the CSS uses #ffffff or #000000. Drift here breaks button legibility.
  const onAccentM = text.match(/--on-accent\s*:\s*(#[0-9a-fA-F]{3,6})/);
  if (onAccentM) {
    const rgb = hexToRgb(onAccentM[1]);
    if (rgb) out["on-accent"] = rgb;
  }
}

// ---------------------------------------------------------------------------
// Kotlin parser
// ---------------------------------------------------------------------------

/**
 * Parse Palette.kt and return a map:
 *   paletteName → { accent, success, warning, danger, bg0, bg1, bg2 }
 *
 * Strategy:
 *   1. Extract all `private val FooBar = Color(0xFFRRGGBB)` assignments.
 *   2. Map them to palette buckets by proximity to palette comment blocks.
 *   3. For `IdeColors(...)` constructors, extract accent/success/warning/danger fields.
 *   4. For `AuroraDef(...)` constructors, extract bg0/bg1/bg2.
 *
 * We use a two-pass approach: first build a symbol table of all Color vals,
 * then resolve IdeColors / AuroraDef fields.
 */
function parseKotlin(kt) {
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

  // Also handle Color(R/255f, G/255f, B/255f, A) — used by lightText() helper
  // These are already captured by the IdeColors constructor below.

  if (VERBOSE) {
    console.log("  [kt] Symbol table:", Object.keys(symbols).length, "named Color vals");
  }

  // ── Pass 2: parse IdeColors(...) constructors ────────────────────────────
  // We look for `val XxxIdeColors = IdeColors(` blocks and extract:
  //   bg, panel, elevated, accent, success, warning, danger fields.
  // Field values are either a named Color val or Color(0xFFRRGGBB).

  const ideColorsRe = /val\s+(\w+IdeColors)\s*=\s*IdeColors\s*\(/g;

  while ((m = ideColorsRe.exec(kt)) !== null) {
    const varName = m[1]; // e.g. "GraphiteMistIdeColors"
    const paletteName = ideColorsToPaletteName(varName);
    if (!paletteName) continue;

    // Extract the block from the opening ( to matching )
    const startIdx = m.index + m[0].length - 1; // position of the opening (
    const block = extractParenBlock(kt, startIdx);
    if (!block) continue;

    const fields = parseIdeColorsFields(block, symbols);
    if (!result[paletteName]) result[paletteName] = {};
    Object.assign(result[paletteName], fields);
  }

  // ── Pass 3: parse AuroraDef(...) constructors for bg0/bg1/bg2 ────────────
  const auroraRe = /val\s+(\w+Aurora)\s*=\s*AuroraDef\s*\(/g;

  while ((m = auroraRe.exec(kt)) !== null) {
    const varName = m[1]; // e.g. "GraphiteMistAurora"
    const paletteName = auroraToPaletteName(varName);
    if (!paletteName) continue;

    const startIdx = m.index + m[0].length - 1;
    const block = extractParenBlock(kt, startIdx);
    if (!block) continue;

    const fields = parseAuroraFields(block, symbols);
    if (!result[paletteName]) result[paletteName] = {};
    Object.assign(result[paletteName], fields);
  }

  // ── Pass 4: handle LiquidBlue (uses DarkIdeColors — not a named IdeColors block)
  // Extract DarkIdeColors if present
  const darkM = kt.match(/val\s+DarkIdeColors\s*=\s*IdeColors\s*\(/);
  if (darkM) {
    const startIdx = darkM.index + darkM[0].length - 1;
    const block = extractParenBlock(kt, startIdx);
    if (block) {
      const fields = parseIdeColorsFields(block, symbols);
      if (!result["liquid-blue"]) result["liquid-blue"] = {};
      // Only fill in if not already set
      for (const [k, v] of Object.entries(fields)) {
        if (!result["liquid-blue"][k]) result["liquid-blue"][k] = v;
      }
    }
  }

  // bg0/bg1/bg2 for liquid-blue from LiquidBlueAurora
  if (!result["liquid-blue"]) result["liquid-blue"] = {};
  // Already handled in Pass 3 via LiquidBlueAurora

  return result;
}

/** Map IdeColors var name to palette key used in CSS. */
function ideColorsToPaletteName(varName) {
  const map = {
    GraphiteMistIdeColors: "graphite-mist",
    DeepSkyIdeColors: "deep-sky",
    NordicCyanIdeColors: "nordic-cyan",
    AuroraVioletIdeColors: "aurora-violet",
    AmberNightIdeColors: "amber-night",
    CloudSilverIdeColors: "cloud-silver",
    FrostBlueIdeColors: "frost-blue",
    PorcelainIdeColors: "porcelain",
    PearlGreyIdeColors: "pearl-grey",
    LightIdeColors: "_light-base",  // not a CSS palette; used for withAccent
    DarkIdeColors: "liquid-blue",
  };
  return map[varName] || null;
}

/** Map Aurora var name to palette key. */
function auroraToPaletteName(varName) {
  const map = {
    GraphiteMistAurora: "graphite-mist",
    LiquidBlueAurora: "liquid-blue",
    DeepSkyAurora: "deep-sky",
    NordicCyanAurora: "nordic-cyan",
    AuroraVioletAurora: "aurora-violet",
    AmberNightAurora: "amber-night",
    CloudSilverAurora: "cloud-silver",
    FrostBlueAurora: "frost-blue",
    PorcelainAurora: "porcelain",
    PearlGreyAurora: "pearl-grey",
  };
  return map[varName] || null;
}

/** Extract everything between the opening paren at `startIdx` and its matching close. */
function extractParenBlock(text, startIdx) {
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
 * Parse IdeColors(...) field list, resolving Color references.
 * Returns { accent, success, warning, danger, bg, panel, elevated, accentOn } as [r,g,b].
 *
 * Field forms we handle:
 *   accent = GmAccent,                              ← named symbol
 *   accent = Color(0xFF9DB7DF),                     ← inline Color
 *   accent = Color(0xFF9DB7DF).copy(alpha = …),     ← inline Color with .copy()
 *   accentOn = Color.White,                         ← Color.White constant
 *   accentOn = Color(0xFF1A0D00),                   ← dark inline Color
 *   success = Color(0xFF7BE0B1),
 *   bg = GmBg0,
 */
function parseIdeColorsFields(block, symbols) {
  const out = {};
  const fields = ["accent", "accentOn", "success", "warning", "danger", "info", "violet", "bg", "panel", "elevated"];

  for (const field of fields) {
    // Look for:  <field> = <expr>,   (newline or comma terminates)
    // The field name may be followed by spaces and =
    const re = new RegExp(
      `\\b${field}\\s*=\\s*([^,\\n]+)`
    );
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
function parseAuroraFields(block, symbols) {
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

/**
 * Resolve a Kotlin color expression to [r,g,b].
 * Handles:
 *   GmAccent                             → symbol lookup
 *   Color(0xFF9DB7DF)                    → direct hex
 *   Color(0xFF9DB7DF).copy(alpha = …)   → hex, ignore .copy()
 *   GmAccent.copy(alpha = …)             → symbol lookup
 *   Color.White                          → [255, 255, 255]
 *   Color.Black                          → [0, 0, 0]
 *   Color(r/255f, g/255f, b/255f, a)    → float components (lightText helper)
 *   darkLine(…)                          → white with alpha → [255,255,255] (skip for comparison)
 *   lightText(R, G, B, A)               → direct R,G,B
 */
function resolveColorExpr(expr, symbols) {
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
// Comparison engine
// ---------------------------------------------------------------------------

/**
 * PALETTE_CHECKS defines what we compare and what tokens are compared.
 * Each entry: { palette, cssTheme, ktKey, tokens }
 *   palette   — CSS data-palette value / Android palette key
 *   cssTheme  — "dark" or "light" (which CSS block to read from)
 *   ktKey     — key used in the kt result map
 *   tokens    — list of token names to compare
 *
 * TOKEN NAME MAPPING (CSS → Kotlin):
 *   "accent"   → CSS "accent" (from --ide-accent-rgb)    / KT "accent"   (IdeColors.accent)
 *   "on-accent"→ CSS "on-accent" (from --on-accent:#hex) / KT "accentOn" (IdeColors.accentOn)
 *   "bg0/1/2"  → CSS "bg0/1/2" (from --bg-*:#hex)        / KT "bg0/1/2"  (AuroraDef.bg*)
 *   "success"  → CSS "success" (--ide-success-rgb)        / KT "success"  (IdeColors.success)
 *   etc.
 *
 * on-accent is critical: it is the text color rendered on accent-colored buttons and
 * chips. If it drifts between web and Android, button text becomes invisible on one
 * platform. The check uses a relaxed tolerance for this token since the Android
 * palette intentionally uses very dark (but not pure black) values (e.g. 0xFF031216
 * for Nordic Cyan, 0xFF1A0D00 for Amber Night) while the web CSS uses #000000.
 */
const DARK_TOKENS = ["accent", "on-accent", "bg0", "bg1", "bg2", "success", "warning", "danger", "info", "violet"];
const LIGHT_TOKENS = ["accent", "on-accent", "bg0", "bg1", "bg2", "info", "violet"];

const PALETTE_CHECKS = [
  // Dark palettes (native dark mode)
  { palette: "graphite-mist", cssTheme: "dark", ktKey: "graphite-mist", tokens: DARK_TOKENS },
  { palette: "deep-sky",      cssTheme: "dark", ktKey: "deep-sky",      tokens: DARK_TOKENS },
  { palette: "nordic-cyan",   cssTheme: "dark", ktKey: "nordic-cyan",   tokens: DARK_TOKENS },
  { palette: "aurora-violet", cssTheme: "dark", ktKey: "aurora-violet", tokens: DARK_TOKENS },
  { palette: "amber-night",   cssTheme: "dark", ktKey: "amber-night",   tokens: DARK_TOKENS },
  // Light palettes (native light mode — accent from their html[data-theme="light"] CSS blocks)
  { palette: "cloud-silver",  cssTheme: "light", ktKey: "cloud-silver", tokens: LIGHT_TOKENS },
  { palette: "frost-blue",    cssTheme: "light", ktKey: "frost-blue",   tokens: LIGHT_TOKENS },
  { palette: "porcelain",     cssTheme: "light", ktKey: "porcelain",    tokens: LIGHT_TOKENS },
  { palette: "pearl-grey",    cssTheme: "light", ktKey: "pearl-grey",   tokens: LIGHT_TOKENS },
  // liquid-blue: accent + semantic tokens (no named IdeColors — uses DarkIdeColors)
  { palette: "liquid-blue",   cssTheme: "dark", ktKey: "liquid-blue",  tokens: ["accent", "on-accent", "bg0", "info", "violet"] },
];

/**
 * Cross-platform token key mapping.
 * CSS block keys → Kotlin IdeColors field names (where they differ).
 * Used when looking up the Android-side value for a given CSS token.
 */
const CSS_TO_KT_KEY = {
  "on-accent": "accentOn",
};

/**
 * Per-token tolerance overrides (per RGB channel, 0–255).
 * Tokens not listed here use the global TOLERANCE.
 *
 * "on-accent": Android uses near-black (e.g. 0xFF031216 = rgb(3,18,22)) while
 * CSS uses #000000 = rgb(0,0,0). These are semantically equivalent (both are
 * "very dark text on accent") but numerically differ by up to 22 per channel.
 * We allow a looser 30-channel tolerance for this token so that dark-but-not-pure-
 * black values don't generate spurious failures — while still catching a drift from
 * dark to white (which would be a contrast failure, ΔR≈255).
 */
const TOKEN_TOLERANCE = {
  "on-accent": 30,
};

// CSS token name → KT token name mapping
// (CSS parses ide-accent-rgb as "accent"; KT parses IdeColors.accent as "accent" — same key)
// bg0/bg1/bg2 both parse under the same key from AuroraDef / --bg-* vars.
// For light palettes, CSS accent comes from --ide-accent-rgb in the [data-theme="light"][data-palette="X"] block.

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  let css, kt;
  try {
    css = readFileSync(CSS_PATH, "utf-8");
  } catch (e) {
    console.error(`ERROR: cannot read CSS: ${CSS_PATH}\n  ${e.message}`);
    process.exit(1);
  }
  try {
    kt = readFileSync(KT_PATH, "utf-8");
  } catch (e) {
    console.error(`ERROR: cannot read Kotlin: ${KT_PATH}\n  ${e.message}`);
    process.exit(1);
  }

  console.log("Parsing CSS tokens from:", CSS_PATH.replace(ROOT + "/", ""));
  const cssTokens = parseCss(css);

  let ktColor = "";
  try {
    ktColor = readFileSync(KT_COLOR_PATH, "utf-8");
  } catch (e) {
    console.warn(`WARN: cannot read Color.kt (liquid-blue Android accent will be skipped): ${e.message}`);
  }
  // Combine Palette.kt and Color.kt so parseKotlin can find DarkIdeColors / LightIdeColors
  const ktCombined = kt + "\n" + ktColor;

  console.log("Parsing Kotlin tokens from:", KT_PATH.replace(ROOT + "/", ""));
  if (ktColor) console.log("  (also reading)", KT_COLOR_PATH.replace(ROOT + "/", ""));
  const ktTokens = parseKotlin(ktCombined);

  if (VERBOSE) {
    console.log("\n=== CSS parsed palettes ===");
    for (const [p, themes] of Object.entries(cssTokens)) {
      for (const [t, toks] of Object.entries(themes)) {
        const keys = Object.keys(toks);
        if (keys.length > 0) {
          console.log(`  [${p || "root"}][${t}]: ${keys.join(", ")}`);
        }
      }
    }
    console.log("\n=== Kotlin parsed palettes ===");
    for (const [p, toks] of Object.entries(ktTokens)) {
      console.log(`  [${p}]: ${Object.keys(toks).join(", ")}`);
    }
  }

  const failures = [];
  const warnings = [];
  let totalChecks = 0;
  let passedChecks = 0;

  console.log(`\nRunning parity checks (tolerance ±${TOLERANCE}/255 per channel)...\n`);

  for (const { palette, cssTheme, ktKey, tokens } of PALETTE_CHECKS) {
    const cssBlock = cssTokens[palette]?.[cssTheme] ?? {};
    const ktBlock = ktTokens[ktKey] ?? {};

    for (const token of tokens) {
      totalChecks++;

      // For "accent" in dark palette blocks, CSS stores it as "accent" from
      // --ide-accent-rgb. Android stores it as "accent" from IdeColors.accent.
      // For "on-accent", CSS uses --on-accent:#hex → "on-accent" key;
      // Android uses IdeColors.accentOn → "accentOn" key. Remap via CSS_TO_KT_KEY.
      // Both are already normalised to [r,g,b].
      const cssVal = cssBlock[token];
      const ktTokenKey = CSS_TO_KT_KEY[token] ?? token;
      const ktVal = ktBlock[ktTokenKey];
      const tol = TOKEN_TOLERANCE[token] ?? TOLERANCE;

      if (!cssVal && !ktVal) {
        warnings.push(
          `  SKIP  [${palette}][${cssTheme}] ${token}: missing from both platforms`
        );
        totalChecks--; // Don't count as check if both are missing
        continue;
      }
      if (!cssVal) {
        warnings.push(
          `  WARN  [${palette}][${cssTheme}] ${token}: missing from CSS (Android.${ktTokenKey}: ${fmtRgb(ktVal)})`
        );
        totalChecks--;
        continue;
      }
      if (!ktVal) {
        warnings.push(
          `  WARN  [${palette}][${cssTheme}] ${token} (Android.${ktTokenKey}): missing from Kotlin (CSS: ${fmtRgb(cssVal)})`
        );
        totalChecks--;
        continue;
      }

      const ok = withinTolerance(cssVal, ktVal, tol);
      const label = `[${palette}][${cssTheme}] ${token}`;
      const cssHex = fmtRgb(cssVal);
      const ktHex = fmtRgb(ktVal);
      const tolNote = tol !== TOLERANCE ? ` (tol ±${tol})` : "";

      if (ok) {
        passedChecks++;
        if (VERBOSE) {
          console.log(`  PASS  ${label}${tolNote}: CSS=${cssHex} Android.${ktTokenKey}=${ktHex}`);
        }
      } else {
        failures.push(
          `  FAIL  ${label}${tolNote}:\n` +
          `          CSS    --${token}       = ${cssHex} (rgb ${cssVal.join(",")})\n` +
          `          Android IdeColors.${ktTokenKey} = ${ktHex} (rgb ${ktVal.join(",")})\n` +
          `          delta  = [${cssVal.map((v, i) => v - ktVal[i]).join(",")}]`
        );
      }
    }
  }

  // Print results
  if (warnings.length > 0) {
    console.log("Warnings (skipped checks — data not parseable from one platform):");
    for (const w of warnings) console.log(w);
    console.log();
  }

  if (failures.length === 0) {
    console.log(
      `PASS: all ${passedChecks}/${totalChecks} token comparisons within tolerance ±${TOLERANCE}.`
    );
    if (warnings.length > 0) {
      console.log(
        `      (${warnings.length} token(s) skipped — see warnings above; investigate manually)`
      );
    }
    process.exit(0);
  } else {
    console.log(`FAIL: ${failures.length} divergence(s) found (${passedChecks}/${totalChecks} passed):\n`);
    for (const f of failures) console.log(f);
    console.log(
      `\nFix: update the failing platform to match the value in docs/PARITY-SPEC.md §A.`
    );
    console.log(`Re-run: node scripts/parity-check.mjs --verbose`);
    process.exit(1);
  }
}

main();
