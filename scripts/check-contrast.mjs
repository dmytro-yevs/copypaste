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
import { toLinear, luminance, contrast, composite } from "./lib/wcag-math.mjs";
import { hexToRgb, tripletToRgb, parseRgba } from "./lib/color-utils.mjs";
import { parseKotlinPalettes } from "./lib/kotlin-parser.mjs";

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
