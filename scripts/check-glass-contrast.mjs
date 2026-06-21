#!/usr/bin/env node
/**
 * check-glass-contrast.mjs — WCAG AA glass-overlay contrast gate (CopyPaste-ojas.11)
 *
 * For each palette in light mode, computes the APPROXIMATE rendered glass
 * background by blending the glass fill colour (--surface-rgb at --glass-opacity)
 * over the canvas colour (--bg-0), then checks that --ide-text-rgb and
 * --ide-dim-rgb meet WCAG AA thresholds over that blended background.
 *
 * Thresholds applied:
 *   --ide-text : WCAG AA normal text → ≥ 4.5:1
 *   --ide-dim  : WCAG AA large/secondary text → ≥ 3.0:1
 *
 * Glass background approximation:
 *   The real macOS NSVisualEffectView blurs the desktop content behind the
 *   window. We cannot simulate that here. Instead we approximate the rendered
 *   surface as:
 *
 *     glass_bg = composite(surface_rgb, glass_opacity, bg_0)
 *
 *   where surface_rgb is the glass panel fill (white in light mode), glass_opacity
 *   is the fill alpha (0.82 in light mode per index.css :root[data-theme="light"]),
 *   and bg_0 is the canvas / background colour for the palette. This is a
 *   conservative LOWER BOUND on contrast — blur and saturation only make the
 *   background lighter/more pastel in light mode, not darker, so if text fails
 *   this approximation it definitely fails on the real surface.
 *
 * Data source:
 *   • crates/copypaste-ui/src/index.css  — glass-opacity, ide-text-rgb, ide-dim-rgb,
 *     and per-palette bg-0 / surface-rgb overrides for light mode.
 *   • crates/copypaste-ui/src/lib/liquid-tokens.ts — PALETTES (scheme flags).
 *
 * Exit codes:
 *   0 — all checks pass WCAG AA (or are skipped with a warning)
 *   1 — one or more checks fail WCAG AA
 *
 * Usage:
 *   node scripts/check-glass-contrast.mjs            # from repo root
 *   node scripts/check-glass-contrast.mjs --verbose  # print all palette results
 */

import { readFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dir = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dir, "..");

const CSS_PATH = resolve(ROOT, "crates/copypaste-ui/src/index.css");
const TS_PATH  = resolve(ROOT, "crates/copypaste-ui/src/lib/liquid-tokens.ts");

const VERBOSE = process.argv.includes("--verbose");

// WCAG AA thresholds
const AA_NORMAL = 4.5;  // normal body text (ide-text)
const AA_LARGE  = 3.0;  // large / secondary text (ide-dim)

// ---------------------------------------------------------------------------
// WCAG contrast math (same as check-contrast.mjs)
// ---------------------------------------------------------------------------

function toLinear(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

function luminance([r, g, b]) {
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
}

function contrastRatio(fg, bg) {
  const L1 = Math.max(luminance(fg), luminance(bg));
  const L2 = Math.min(luminance(fg), luminance(bg));
  return (L1 + 0.05) / (L2 + 0.05);
}

/**
 * Porter-Duff src-over composite: opaque src at `alpha` over opaque `bg`.
 * Returns an opaque [r,g,b].
 */
function composite(src, alpha, bg) {
  return src.map((c, i) => Math.round(c * alpha + bg[i] * (1 - alpha)));
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/** Parse "#RRGGBB" to [r, g, b]. Returns null on failure. */
function hexToRgb(hex) {
  hex = hex.replace(/^#/, "");
  if (hex.length === 3) hex = hex.split("").map((c) => c + c).join("");
  if (hex.length !== 6) return null;
  const n = parseInt(hex, 16);
  if (isNaN(n)) return null;
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Parse "R G B" or "R, G, B" triplet (CSS custom property form). */
function tripletToRgb(s) {
  if (!s) return null;
  const parts = s.trim().split(/[\s,]+/).filter(Boolean);
  if (parts.length !== 3) return null;
  const nums = parts.map(Number);
  if (nums.some(isNaN)) return null;
  return nums;
}

// ---------------------------------------------------------------------------
// Read light-palette data from index.css
// ---------------------------------------------------------------------------
// We extract the following per palette (for light mode):
//   bg0       — from html[data-theme="light"][data-palette="X"] { --bg-0: ... }
//   surfaceRgb — from the same block (--surface-rgb); falls back to "255, 255, 255"
//   glassOpacity — from the same block (--glass-opacity); falls back to 0.82
//
// And shared light-mode tokens (from :root[data-theme="light"]):
//   ideTextRgb  — --ide-text-rgb
//   ideDimRgb   — --ide-dim-rgb

/**
 * Extract a CSS custom-property value from a block of text.
 * Handles: "--prop: value;"  and  "--prop:value;"
 * Returns null when not found.
 */
function extractCssProp(blockText, propName) {
  // Match `--propName: <value>;` — value may contain spaces, commas, hashes
  const re = new RegExp(`--${propName}\\s*:\\s*([^;\\n]+)(?:;|$)`, "m");
  const m = blockText.match(re);
  if (!m) return null;
  return m[1].trim();
}

/**
 * Extract the body of a CSS selector block.
 * Finds `selectorFragment {` then extracts everything up to the matching `}`.
 */
function extractCssBlock(src, selectorFragment) {
  const idx = src.indexOf(selectorFragment);
  if (idx === -1) return null;
  // Find the { after the selector
  const braceOpen = src.indexOf("{", idx);
  if (braceOpen === -1) return null;
  // Find the matching }
  let depth = 0, i = braceOpen, end = -1;
  while (i < src.length) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") {
      depth--;
      if (depth === 0) { end = i; break; }
    }
    i++;
  }
  if (end === -1) return null;
  return src.slice(braceOpen + 1, end);
}

/**
 * Parse the shared light-theme tokens from :root[data-theme="light"].
 */
function parseLightRootTokens(css) {
  // Try :root[data-theme="light"] first, then fall back to [data-theme="light"]
  const block = extractCssBlock(css, ':root[data-theme="light"]') ||
                extractCssBlock(css, 'root[data-theme="light"]');
  if (!block) {
    return { ideTextRgb: null, ideDimRgb: null, glassFillAlpha: null, surfaceRgb: null, bg0: null };
  }

  const ideTextRaw   = extractCssProp(block, "ide-text-rgb");
  const ideDimRaw    = extractCssProp(block, "ide-dim-rgb");
  const glassOpacity = extractCssProp(block, "glass-opacity");
  const surfaceRaw   = extractCssProp(block, "surface-rgb");
  const bg0Raw       = extractCssProp(block, "bg-0");

  return {
    ideTextRgb:     ideTextRaw ? tripletToRgb(ideTextRaw) : null,
    ideDimRgb:      ideDimRaw  ? tripletToRgb(ideDimRaw)  : null,
    glassFillAlpha: glassOpacity ? parseFloat(glassOpacity) : null,
    surfaceRgb:     surfaceRaw  ? tripletToRgb(surfaceRaw.replace(/,/g, " ")) : null,
    bg0:            bg0Raw      ? hexToRgb(bg0Raw) : null,
  };
}

/**
 * Parse per-palette light-mode overrides.
 * Returns a map: paletteKey → { bg0, surfaceRgb, glassFillAlpha }
 */
function parseLightPaletteOverrides(css) {
  const result = {};

  // The per-palette light blocks look like:
  //   html[data-theme="light"][data-palette="cloud-silver"] { ... }
  const re = /html\[data-theme="light"\]\[data-palette="([a-z-]+)"\]/g;
  let m;
  while ((m = re.exec(css)) !== null) {
    const palKey = m[1];
    const block = extractCssBlock(css, m[0]);
    if (!block) continue;

    const bg0Raw       = extractCssProp(block, "bg-0");
    const surfaceRaw   = extractCssProp(block, "surface-rgb");
    const glassOpRaw   = extractCssProp(block, "glass-opacity");

    result[palKey] = {
      bg0:            bg0Raw     ? hexToRgb(bg0Raw.trim()) : null,
      surfaceRgb:     surfaceRaw ? tripletToRgb(surfaceRaw.replace(/,/g, " ")) : null,
      glassFillAlpha: glassOpRaw ? parseFloat(glassOpRaw) : null,
    };
  }

  return result;
}

/**
 * Parse the PALETTES constant from liquid-tokens.ts.
 * Returns a map: paletteKey → { scheme: "dark"|"light" }
 * We only need the scheme to know which palettes to check.
 */
function parseTsPaletteSchemes(src) {
  const result = {};
  const schemeRe = /"([a-z-]+)":\s*\{[^}]*?scheme:\s*"(dark|light)"/gs;
  let m;
  while ((m = schemeRe.exec(src)) !== null) {
    result[m[1]] = m[2];
  }
  return result;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  let css, tsSrc;
  try { css    = readFileSync(CSS_PATH, "utf-8"); }
  catch (e) { console.error(`ERROR: cannot read ${CSS_PATH}: ${e.message}`); process.exit(1); }
  try { tsSrc  = readFileSync(TS_PATH, "utf-8"); }
  catch (e) { console.error(`ERROR: cannot read ${TS_PATH}: ${e.message}`); process.exit(1); }

  // 1. Shared light-mode tokens (fallback / default for palettes that don't override them)
  const lightRoot = parseLightRootTokens(css);

  const defaultTextRgb  = lightRoot.ideTextRgb  ?? [29, 29, 31];   // from :root[data-theme="light"]
  const defaultDimRgb   = lightRoot.ideDimRgb   ?? [91, 91, 96];
  const defaultGlassAlpha = lightRoot.glassFillAlpha ?? 0.82;
  const defaultSurfaceRgb = lightRoot.surfaceRgb ?? [255, 255, 255];
  const defaultBg0        = lightRoot.bg0        ?? [232, 232, 238]; // #e8e8ee

  if (VERBOSE) {
    console.log("Shared light-mode tokens:");
    console.log(`  --ide-text-rgb:  rgb(${defaultTextRgb.join(", ")})`);
    console.log(`  --ide-dim-rgb:   rgb(${defaultDimRgb.join(", ")})`);
    console.log(`  --glass-opacity: ${defaultGlassAlpha}`);
    console.log(`  --surface-rgb:   rgb(${defaultSurfaceRgb.join(", ")})`);
    console.log(`  --bg-0 (root):   rgb(${defaultBg0.join(", ")})\n`);
  }

  // 2. Per-palette light overrides
  const paletteOverrides = parseLightPaletteOverrides(css);

  // 3. Palette schemes from TS
  const schemes = parseTsPaletteSchemes(tsSrc);

  // Determine which palettes are light-scheme
  // Any palette with scheme:"light" in liquid-tokens.ts, PLUS the default (no palette = dark
  // graphite-mist, but we check all light palettes explicitly).
  const lightPalettes = Object.entries(schemes)
    .filter(([, scheme]) => scheme === "light")
    .map(([key]) => key);

  if (lightPalettes.length === 0) {
    console.error("ERROR: No light palettes found in liquid-tokens.ts — check TS_PATH");
    process.exit(1);
  }

  // Also add the "default light" palette (no data-palette attribute, just data-theme="light")
  // using the :root[data-theme="light"] values.
  const PALETTES_TO_CHECK = [
    { key: "_default-light", label: "default light (no palette override)" },
    ...lightPalettes.map((k) => ({ key: k, label: k })),
  ];

  const failures = [];
  const warnings = [];
  let totalChecks = 0;
  let passedChecks = 0;

  console.log("Glass-overlay contrast gate — light mode (WCAG AA)\n");
  console.log(
    "  Approximation: glass_bg = composite(surface_rgb @ glass_opacity, bg_0)\n" +
    `  Text threshold:  ≥ ${AA_NORMAL}:1 (ide-text — WCAG AA normal)\n` +
    `  Dim threshold:   ≥ ${AA_LARGE}:1  (ide-dim  — WCAG AA large/secondary)\n`
  );
  console.log("─── Per-palette checks ──────────────────────────────────────────────");

  for (const { key, label } of PALETTES_TO_CHECK) {
    const ov = key === "_default-light" ? {} : (paletteOverrides[key] ?? {});

    const bg0        = ov.bg0        ?? defaultBg0;
    const surfaceRgb = ov.surfaceRgb ?? defaultSurfaceRgb;
    const glassAlpha = ov.glassFillAlpha ?? defaultGlassAlpha;

    if (!bg0) {
      warnings.push(`  WARN [${label}] cannot resolve --bg-0 — skipped`);
      continue;
    }
    if (!surfaceRgb) {
      warnings.push(`  WARN [${label}] cannot resolve --surface-rgb — skipped`);
      continue;
    }

    // Approximate rendered glass background
    const glassBg = composite(surfaceRgb, glassAlpha, bg0);

    const textRgb = defaultTextRgb;
    const dimRgb  = defaultDimRgb;

    // ── ide-text check (normal text, ≥ 4.5:1) ─────────────────────────
    totalChecks++;
    const textRatio = contrastRatio(textRgb, glassBg);
    const textPass  = textRatio >= AA_NORMAL;
    const textRatioStr = textRatio.toFixed(2);
    const glassBgHex = "#" + glassBg.map((c) => Math.max(0, Math.min(255, c)).toString(16).padStart(2, "0")).join("");
    const textHex    = "#" + textRgb.map((c) => c.toString(16).padStart(2, "0")).join("");
    const dimHex     = "#" + dimRgb.map((c) => c.toString(16).padStart(2, "0")).join("");
    const bg0Hex     = "#" + bg0.map((c) => c.toString(16).padStart(2, "0")).join("");

    if (textPass) {
      passedChecks++;
      if (VERBOSE) {
        console.log(
          `  PASS [${label}] ide-text: ${textRatioStr}:1 ≥ ${AA_NORMAL}:1\n` +
          `       text=${textHex}  glass_bg≈${glassBgHex} (surface=${surfaceRgb.join(",")}@${glassAlpha} over bg0=${bg0Hex})`
        );
      }
    } else {
      failures.push(
        `  FAIL [${label}] ide-text: ${textRatioStr}:1 < ${AA_NORMAL}:1 (AA normal)\n` +
        `       text=${textHex}  glass_bg≈${glassBgHex}\n` +
        `       → blend: surface=rgb(${surfaceRgb.join(",")}) @ α=${glassAlpha} over bg0=${bg0Hex}\n` +
        `       → primary body text is not legible enough over the glass overlay`
      );
    }

    // ── ide-dim check (large/secondary text, ≥ 3.0:1) ─────────────────
    totalChecks++;
    const dimRatio = contrastRatio(dimRgb, glassBg);
    const dimPass  = dimRatio >= AA_LARGE;
    const dimRatioStr = dimRatio.toFixed(2);

    if (dimPass) {
      passedChecks++;
      if (VERBOSE) {
        console.log(
          `  PASS [${label}] ide-dim: ${dimRatioStr}:1 ≥ ${AA_LARGE}:1\n` +
          `       dim=${dimHex}  glass_bg≈${glassBgHex}`
        );
      }
    } else {
      failures.push(
        `  FAIL [${label}] ide-dim: ${dimRatioStr}:1 < ${AA_LARGE}:1 (AA large/secondary)\n` +
        `       dim=${dimHex}  glass_bg≈${glassBgHex}\n` +
        `       → blend: surface=rgb(${surfaceRgb.join(",")}) @ α=${glassAlpha} over bg0=${bg0Hex}\n` +
        `       → secondary/dim text is not legible enough over the glass overlay`
      );
    }
  }

  // ── Print results ──────────────────────────────────────────────────────────

  console.log();
  if (warnings.length > 0) {
    console.log("Warnings / skipped checks:");
    for (const w of warnings) console.log(w);
    console.log();
  }

  if (failures.length === 0) {
    console.log(
      `PASS: all ${passedChecks}/${totalChecks} glass-contrast checks pass WCAG AA.`
    );
    process.exit(0);
  } else {
    console.log(
      `FAIL: ${failures.length} glass-contrast pair(s) below WCAG AA (${passedChecks}/${totalChecks} passed):\n`
    );
    for (const f of failures) console.log(f);
    console.log(
      `\nNote: fix is out of scope for this script — open a follow-up bd issue.\n` +
      `These values are approximations: actual rendered contrast may differ due to\n` +
      `backdrop blur and NSVisualEffectView vibrancy, but this lower-bound check\n` +
      `is the minimum bar any production palette should clear.\n` +
      `Re-run: node scripts/check-glass-contrast.mjs --verbose`
    );
    process.exit(1);
  }
}

main();
