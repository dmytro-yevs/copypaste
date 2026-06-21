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
import { fmtRgb, withinTolerance } from "./lib/color-utils.mjs";
import { parseCss } from "./lib/css-parser.mjs";
import { parseKotlin } from "./lib/kotlin-parser.mjs";

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
  const ktTokens = parseKotlin(ktCombined, VERBOSE);

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
