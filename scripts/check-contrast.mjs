#!/usr/bin/env node
/**
 * check-contrast.mjs — WCAG AA contrast gate for CopyPaste design tokens.
 *
 * Reads palette definitions from:
 *   • crates/copypaste-ui/src/lib/liquid-tokens.ts  (macOS/Web token source)
 *   • android/.../ui/theme/Palette.kt               (Android palette source)
 *
 * For each palette it checks two critical foreground/background pairs:
 *   1. text-on-surface — the primary text color over the panel/surface background.
 *      Web: contrast profiles (balanced) text color over surfaceRgb.
 *      Android: IdeColors.text (dark text / balanced) over IdeColors.panel.
 *   2. on-accent-on-accent — the on-accent foreground over the accent background.
 *      Web:     --on-accent color over --accent color (from liquid-tokens.ts PALETTES).
 *      Android: IdeColors.accentOn over IdeColors.accent (from Palette.kt).
 *
 * WCAG 2.1 AA thresholds:
 *   • Normal text (< 18pt / < 14pt bold): contrast ratio ≥ 4.5:1
 *   • Large text (≥ 18pt / ≥ 14pt bold):  contrast ratio ≥ 3.0:1
 *
 * We apply the AA-normal (4.5:1) threshold for text-on-surface (body text)
 * and on-accent (button labels, which are typically bold but small).
 *
 * Note: The contrast profiles in liquid-tokens.ts use rgba() with alpha < 1.
 * We composite these over the surface background using the alpha-blending
 * formula (Porter-Duff src-over) before computing WCAG relative luminance.
 *
 * Exit codes:
 *   0 — all pairs pass WCAG AA (or are skipped with a warning)
 *   1 — one or more pairs fail WCAG AA
 *
 * Usage:
 *   node scripts/check-contrast.mjs            # from repo root
 *   node scripts/check-contrast.mjs --verbose  # print all pair results
 */

import { readFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dir = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dir, "..");

const TS_PATH = resolve(ROOT, "crates/copypaste-ui/src/lib/liquid-tokens.ts");
const KT_PATH = resolve(
  ROOT,
  "android/app/src/main/java/com/copypaste/android/ui/theme/Palette.kt"
);
const KT_COLOR_PATH = resolve(
  ROOT,
  "android/app/src/main/java/com/copypaste/android/ui/theme/Color.kt"
);

const VERBOSE = process.argv.includes("--verbose");

// WCAG AA thresholds
const AA_NORMAL = 4.5;
const AA_LARGE  = 3.0;

// ---------------------------------------------------------------------------
// WCAG contrast math
// ---------------------------------------------------------------------------

/** Convert an sRGB channel value (0–255) to linear light. */
function toLinear(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

/** Compute WCAG relative luminance for an [r,g,b] triplet (0–255 each). */
function luminance([r, g, b]) {
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
}

/** Compute WCAG contrast ratio for two [r,g,b] colors. */
function contrast(fg, bg) {
  const L1 = Math.max(luminance(fg), luminance(bg));
  const L2 = Math.min(luminance(fg), luminance(bg));
  return (L1 + 0.05) / (L2 + 0.05);
}

/**
 * Composite a foreground color with alpha over an opaque background.
 * fg: [r, g, b] (0–255), alpha: 0–1, bg: [r, g, b] (0–255)
 * Returns opaque [r, g, b].
 */
function composite(fg, alpha, bg) {
  return fg.map((c, i) => Math.round(c * alpha + bg[i] * (1 - alpha)));
}

// ---------------------------------------------------------------------------
// Hex / rgba parsers
// ---------------------------------------------------------------------------

/** Parse "#RRGGBB" or "#RGB" to [r, g, b]. Returns null on failure. */
function hexToRgb(hex) {
  hex = hex.replace(/^#/, "");
  if (hex.length === 3) hex = hex.split("").map((c) => c + c).join("");
  if (hex.length !== 6) return null;
  const n = parseInt(hex, 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Parse "R, G, B" or "R G B" (CSS rgb triplet) to [r, g, b]. */
function tripletToRgb(s) {
  const parts = s.trim().split(/[\s,]+/);
  if (parts.length !== 3) return null;
  return parts.map(Number);
}

/**
 * Parse "rgba(r,g,b,a)" or "rgba(r, g, b, a)" to { rgb:[r,g,b], alpha:a }.
 * Also handles "rgb(r,g,b)" (alpha=1).
 */
function parseRgba(s) {
  s = s.trim();
  const rgbaM = s.match(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)(?:\s*,\s*([\d.]+))?\s*\)/);
  if (!rgbaM) return null;
  return {
    rgb: [parseInt(rgbaM[1]), parseInt(rgbaM[2]), parseInt(rgbaM[3])],
    alpha: rgbaM[4] !== undefined ? parseFloat(rgbaM[4]) : 1,
  };
}

// ---------------------------------------------------------------------------
// TypeScript token source parser
// ---------------------------------------------------------------------------

/**
 * Parse liquid-tokens.ts and extract palette definitions.
 *
 * We import the module as text and use regex to extract the PALETTES constant.
 * Returns a map: paletteKey → { accent, onAccent, surfaceRgb, scheme, ... }
 *
 * Fields extracted:
 *   accent    — string hex (e.g. "#4d8dff")
 *   onAccent  — string hex (e.g. "#ffffff")
 *   surfaceRgb — string "R, G, B" (CSS triplet)
 *   scheme    — "dark" | "light"
 *   bg0       — hex
 */
function parseLiquidTokens(src) {
  const result = {};

  // We parse the object literal structure using regex on the known format:
  //   "key": { name: "...", scheme: "dark", ..., accent: "#hex", onAccent: "#hex", ... },
  // The PALETTES constant spans lines 73–174 in the known file format.

  // Extract the PALETTES object body
  const palettesStart = src.indexOf("export const PALETTES:");
  if (palettesStart === -1) throw new Error("Cannot find `export const PALETTES:` in liquid-tokens.ts");

  const braceStart = src.indexOf("{", palettesStart);
  if (braceStart === -1) throw new Error("Cannot find opening { after PALETTES");

  // Find matching closing brace
  let depth = 0;
  let i = braceStart;
  let end = -1;
  while (i < src.length) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") {
      depth--;
      if (depth === 0) { end = i; break; }
    }
    i++;
  }
  if (end === -1) throw new Error("Cannot find closing } for PALETTES object");

  const palettesBody = src.slice(braceStart, end + 1);

  // Split into per-palette blocks by matching the top-level keys:
  //   "liquid-blue": { ... },
  // We do this by finding all `"key":` at depth=1 of the palettesBody.
  const paletteKeyRe = /"([a-z-]+)":\s*\{/g;
  let km;
  const keyPositions = [];
  while ((km = paletteKeyRe.exec(palettesBody)) !== null) {
    keyPositions.push({ key: km[1], start: km.index });
  }

  for (let ki = 0; ki < keyPositions.length; ki++) {
    const { key, start } = keyPositions[ki];
    // Find the inner { } block for this palette
    const innerBraceStart = palettesBody.indexOf("{", start + key.length + 2);
    let d = 0, j = innerBraceStart, innerEnd = -1;
    while (j < palettesBody.length) {
      if (palettesBody[j] === "{") d++;
      else if (palettesBody[j] === "}") {
        d--;
        if (d === 0) { innerEnd = j; break; }
      }
      j++;
    }
    if (innerEnd === -1) continue;

    const block = palettesBody.slice(innerBraceStart + 1, innerEnd);
    const entry = {};

    // Extract string fields: scheme, accent, onAccent, surfaceRgb, bg0
    const stringFields = {
      scheme:       /\bscheme:\s*"([^"]+)"/,
      accent:       /\baccent:\s*"(#[0-9a-fA-F]{3,6})"/,
      onAccent:     /\bonAccent:\s*"(#[0-9a-fA-F]{3,6})"/,
      surfaceRgb:   /\bsurfaceRgb:\s*"([^"]+)"/,
      bg0:          /\bbg0:\s*"(#[0-9a-fA-F]{3,6})"/,
    };

    for (const [field, re] of Object.entries(stringFields)) {
      const m = block.match(re);
      if (m) entry[field] = m[1];
    }

    if (entry.accent && entry.onAccent && entry.surfaceRgb && entry.scheme) {
      result[key] = entry;
    }
  }

  return result;
}

/**
 * Parse the CONTRAST_PROFILES constant from liquid-tokens.ts.
 * Returns: { dark: { balanced: { text: "rgba(...)", ... } }, light: { ... } }
 */
function parseContrastProfiles(src) {
  const result = {};

  const contrastStart = src.indexOf("export const CONTRAST_PROFILES:");
  if (contrastStart === -1) {
    console.warn("WARN: Cannot find CONTRAST_PROFILES in liquid-tokens.ts; skipping text-on-surface checks");
    return result;
  }

  // Find the outer { }
  const braceStart = src.indexOf("{", contrastStart);
  let depth = 0, i = braceStart, end = -1;
  while (i < src.length) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") { depth--; if (depth === 0) { end = i; break; } }
    i++;
  }
  if (end === -1) return result;

  const body = src.slice(braceStart + 1, end);

  // Parse scheme blocks: dark: { ... }, light: { ... }
  for (const scheme of ["dark", "light"]) {
    const schemeRe = new RegExp(`\\b${scheme}\\s*:\\s*\\{`);
    const sm = schemeRe.exec(body);
    if (!sm) continue;

    // Find the scheme block
    const sStart = body.indexOf("{", sm.index + sm[0].length - 1);
    let sd = 0, si = sStart, sEnd = -1;
    while (si < body.length) {
      if (body[si] === "{") sd++;
      else if (body[si] === "}") { sd--; if (sd === 0) { sEnd = si; break; } }
      si++;
    }
    if (sEnd === -1) continue;

    const schemeBlock = body.slice(sStart + 1, sEnd);
    result[scheme] = {};

    // Parse level blocks: soft: { ... }, balanced: { ... }, high: { ... }
    for (const level of ["soft", "balanced", "high"]) {
      const levelRe = new RegExp(`\\b${level}\\s*:\\s*\\{`);
      const lm = levelRe.exec(schemeBlock);
      if (!lm) continue;

      const lStart = schemeBlock.indexOf("{", lm.index + lm[0].length - 1);
      let ld = 0, li = lStart, lEnd = -1;
      while (li < schemeBlock.length) {
        if (schemeBlock[li] === "{") ld++;
        else if (schemeBlock[li] === "}") { ld--; if (ld === 0) { lEnd = li; break; } }
        li++;
      }
      if (lEnd === -1) continue;

      const levelBlock = schemeBlock.slice(lStart + 1, lEnd);
      result[scheme][level] = {};

      // Extract individual token strings
      const tokenRe = /\b(\w+)\s*:\s*"([^"]+)"/g;
      let tm;
      while ((tm = tokenRe.exec(levelBlock)) !== null) {
        result[scheme][level][tm[1]] = tm[2];
      }
    }
  }

  return result;
}

// ---------------------------------------------------------------------------
// Kotlin parser (reuses logic from parity-check.mjs)
// ---------------------------------------------------------------------------

/** Parse Android Palette.kt + Color.kt for accent/accentOn/panel/text per palette. */
function parseKotlinPalettes(kt) {
  const result = {};

  // Build symbol table of named Color constants
  const colorValRe = /(?:private\s+)?val\s+(\w+)\s*=\s*Color\(0x([0-9a-fA-F]{6,8})\)/g;
  const symbols = {};
  let m;
  while ((m = colorValRe.exec(kt)) !== null) {
    symbols[m[1]] = ktHexToRgb("0x" + m[2]);
  }
  // Color.White and Color.Black
  symbols["Color.White"] = [255, 255, 255];
  symbols["Color.Black"] = [0, 0, 0];

  function resolveExpr(expr) {
    if (!expr) return null;
    expr = expr.trim();
    if (/^Color\.White$/.test(expr)) return [255, 255, 255];
    if (/^Color\.Black$/.test(expr)) return [0, 0, 0];
    const symM = expr.match(/^(\w+)(?:\.copy\(.*\))?$/);
    if (symM && symbols[symM[1]]) return symbols[symM[1]];
    const hexM = expr.match(/Color\(0x([0-9a-fA-F]{6,8})\)/);
    if (hexM) return ktHexToRgb("0x" + hexM[1]);
    return null;
  }

  function extractParenBlock(text, startIdx) {
    let depth = 0, i = startIdx, start = -1;
    while (i < text.length) {
      if (text[i] === "(") { if (depth === 0) start = i + 1; depth++; }
      else if (text[i] === ")") { depth--; if (depth === 0) return text.slice(start, i); }
      i++;
    }
    return null;
  }

  // Parse IdeColors constructors
  const ideColorsRe = /val\s+(\w+IdeColors)\s*=\s*IdeColors\s*\(/g;
  const ideColorsMap = {
    GraphiteMistIdeColors: "graphite-mist",
    DeepSkyIdeColors: "deep-sky",
    NordicCyanIdeColors: "nordic-cyan",
    AuroraVioletIdeColors: "aurora-violet",
    AmberNightIdeColors: "amber-night",
    CloudSilverIdeColors: "cloud-silver",
    FrostBlueIdeColors: "frost-blue",
    PorcelainIdeColors: "porcelain",
    PearlGreyIdeColors: "pearl-grey",
    DarkIdeColors: "liquid-blue",
    LightIdeColors: "_light-base",
  };

  while ((m = ideColorsRe.exec(kt)) !== null) {
    const varName = m[1];
    const palKey = ideColorsMap[varName];
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

  // AuroraDef for bg0 (for surface background)
  const auroraMap = {
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
  const auroraRe = /val\s+(\w+Aurora)\s*=\s*AuroraDef\s*\(/g;
  while ((m = auroraRe.exec(kt)) !== null) {
    const palKey = auroraMap[m[1]];
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

/** Parse Android Color(0xFFRRGGBB) hex to [r,g,b]. */
function ktHexToRgb(hex) {
  hex = hex.replace(/^0[xX]/, "");
  if (hex.length === 8) hex = hex.slice(2);
  if (hex.length !== 6) return null;
  const n = parseInt(hex, 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  let tsSrc, ktSrc, ktColorSrc = "";
  try { tsSrc = readFileSync(TS_PATH, "utf-8"); }
  catch (e) { console.error(`ERROR: cannot read ${TS_PATH}: ${e.message}`); process.exit(1); }
  try { ktSrc = readFileSync(KT_PATH, "utf-8"); }
  catch (e) { console.error(`ERROR: cannot read ${KT_PATH}: ${e.message}`); process.exit(1); }
  try { ktColorSrc = readFileSync(KT_COLOR_PATH, "utf-8"); }
  catch (e) { console.warn(`WARN: cannot read Color.kt (liquid-blue may be skipped): ${e.message}`); }

  const tsPalettes = parseLiquidTokens(tsSrc);
  const tsContrast = parseContrastProfiles(tsSrc);
  const ktPalettes = parseKotlinPalettes(ktSrc + "\n" + ktColorSrc);

  const failures = [];
  const warnings = [];
  let totalChecks = 0;
  let passedChecks = 0;

  console.log("WCAG AA contrast gate (normal text ≥ 4.5:1, large text ≥ 3.0:1)\n");

  // ── Check 1: on-accent contrast (accent button text legibility) ────────────
  // For each palette: on-accent foreground over accent background must be ≥ 4.5:1.
  // Both sources checked independently.

  console.log("─── on-accent over accent (button label contrast) ───────────────");

  const TS_PALETTE_KEYS = Object.keys(tsPalettes);

  for (const key of TS_PALETTE_KEYS) {
    const p = tsPalettes[key];
    if (!p.accent || !p.onAccent) {
      warnings.push(`  WARN [web][${key}] on-accent: missing accent or onAccent — skipped`);
      continue;
    }
    totalChecks++;
    const accentRgb = hexToRgb(p.accent);
    const onAccentRgb = hexToRgb(p.onAccent);
    if (!accentRgb || !onAccentRgb) {
      warnings.push(`  WARN [web][${key}] on-accent: cannot parse hex values — skipped`);
      totalChecks--;
      continue;
    }
    const ratio = contrast(onAccentRgb, accentRgb);
    const pass = ratio >= AA_NORMAL;
    const ratioStr = ratio.toFixed(2);
    if (pass) {
      passedChecks++;
      if (VERBOSE) console.log(`  PASS [web][${key}] on-accent: ${ratioStr}:1 (accent=${p.accent} onAccent=${p.onAccent})`);
    } else {
      failures.push(
        `  FAIL [web][${key}] on-accent: ${ratioStr}:1 < ${AA_NORMAL}:1 (AA)\n` +
        `       accent=${p.accent}  on-accent=${p.onAccent}\n` +
        `       → button/chip label is not legible enough on this accent background`
      );
    }
  }

  // Android on-accent checks
  for (const [key, p] of Object.entries(ktPalettes)) {
    if (key.startsWith("_")) continue; // skip internal keys like _light-base
    if (!p.accent || !p.accentOn) {
      if (VERBOSE) warnings.push(`  WARN [android][${key}] on-accent: missing accentOn or accent — skipped`);
      continue;
    }
    totalChecks++;
    const ratio = contrast(p.accentOn, p.accent);
    const pass = ratio >= AA_NORMAL;
    const ratioStr = ratio.toFixed(2);
    const accentHex = "#" + p.accent.map((c) => c.toString(16).padStart(2, "0")).join("").toUpperCase();
    const onHex = "#" + p.accentOn.map((c) => c.toString(16).padStart(2, "0")).join("").toUpperCase();
    if (pass) {
      passedChecks++;
      if (VERBOSE) console.log(`  PASS [android][${key}] on-accent: ${ratioStr}:1 (accent=${accentHex} accentOn=${onHex})`);
    } else {
      failures.push(
        `  FAIL [android][${key}] on-accent: ${ratioStr}:1 < ${AA_NORMAL}:1 (AA)\n` +
        `       IdeColors.accent=${accentHex}  IdeColors.accentOn=${onHex}\n` +
        `       → button/chip label is not legible enough on this accent background`
      );
    }
  }

  // ── Check 2: text-on-surface contrast ─────────────────────────────────────
  // For the web: the "balanced" contrast profile text color composited over
  // the surfaceRgb background must be ≥ 4.5:1.
  // We use the "balanced" level as the representative case (it is the shipped default).

  console.log("\n─── text-on-surface (balanced contrast profile) ─────────────────");

  for (const key of TS_PALETTE_KEYS) {
    const p = tsPalettes[key];
    if (!p.surfaceRgb || !p.scheme) {
      warnings.push(`  WARN [web][${key}] text-on-surface: missing surfaceRgb or scheme — skipped`);
      continue;
    }

    const scheme = p.scheme; // "dark" | "light"
    const level = "balanced";
    const profile = tsContrast[scheme]?.[level];
    if (!profile?.text) {
      warnings.push(`  WARN [web][${key}] text-on-surface: no ${scheme}/${level}/text in CONTRAST_PROFILES — skipped`);
      continue;
    }

    const surfaceRgb = tripletToRgb(p.surfaceRgb.replace(/,/g, " "));
    if (!surfaceRgb) {
      warnings.push(`  WARN [web][${key}] text-on-surface: cannot parse surfaceRgb "${p.surfaceRgb}" — skipped`);
      continue;
    }

    const textParsed = parseRgba(profile.text);
    if (!textParsed) {
      warnings.push(`  WARN [web][${key}] text-on-surface: cannot parse text color "${profile.text}" — skipped`);
      continue;
    }

    totalChecks++;

    // Composite text color (with alpha) over the opaque surface
    const composited = composite(textParsed.rgb, textParsed.alpha, surfaceRgb);
    const ratio = contrast(composited, surfaceRgb);
    const pass = ratio >= AA_NORMAL;
    const ratioStr = ratio.toFixed(2);

    if (pass) {
      passedChecks++;
      if (VERBOSE) {
        const surfHex = "#" + surfaceRgb.map((c) => c.toString(16).padStart(2, "0")).join("");
        console.log(`  PASS [web][${key}] text-on-surface (${scheme}/balanced): ${ratioStr}:1 (surface=${surfHex} text=${profile.text})`);
      }
    } else {
      const surfHex = "#" + surfaceRgb.map((c) => c.toString(16).padStart(2, "0")).join("");
      failures.push(
        `  FAIL [web][${key}] text-on-surface (${scheme}/balanced): ${ratioStr}:1 < ${AA_NORMAL}:1 (AA)\n` +
        `       surface=${surfHex}  text=${profile.text}\n` +
        `       composited text rgb(${composited.join(",")})\n` +
        `       → body text is not legible enough on the surface background`
      );
    }
  }

  // Android text-on-surface: IdeColors.text (already opaque — see darkText/lightText helpers)
  // over IdeColors.panel (the main surface background).
  // The text helpers (darkText, lightText) bake in alpha, so we check the composited value.
  // Since we can only resolve named Color() constants (not float-alpha helpers like darkText),
  // we skip Android text-on-surface and document the gap.
  warnings.push(
    "  NOTE [android] text-on-surface: skipped — text values use darkText()/lightText() alpha helpers\n" +
    "       that cannot be resolved without evaluating Kotlin at runtime. Verify manually in\n" +
    "       android/app/src/main/java/com/copypaste/android/ui/theme/Palette.kt (contrast profile §2)."
  );

  // ── Print results ──────────────────────────────────────────────────────────

  console.log();
  if (warnings.length > 0) {
    console.log("Warnings / skipped checks:");
    for (const w of warnings) console.log(w);
    console.log();
  }

  if (failures.length === 0) {
    console.log(
      `PASS: all ${passedChecks}/${totalChecks} contrast checks pass WCAG AA.`
    );
    process.exit(0);
  } else {
    console.log(`FAIL: ${failures.length} contrast pair(s) below WCAG AA (${passedChecks}/${totalChecks} passed):\n`);
    for (const f of failures) console.log(f);
    console.log(
      `\nFix: update the drifting accent or surface token so the ratio meets ≥ ${AA_NORMAL}:1 (normal)\n` +
      `or ≥ ${AA_LARGE}:1 (large text). Use https://webaim.org/resources/contrastchecker/ to verify.\n` +
      `Re-run: node scripts/check-contrast.mjs --verbose`
    );
    process.exit(1);
  }
}

main();
