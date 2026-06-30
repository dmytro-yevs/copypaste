#!/usr/bin/env node
/**
 * parity-check.mjs — Two-axis design-token parity checker (Phase 7, CopyPaste-2hfj.8)
 *
 * Asserts that the token NAMES in tokens.css map 1:1 to the fields of CpColors
 * in Color.kt, and that the data-accent values map 1:1 to the AccentColor
 * variant names.
 *
 * Sources:
 *   • crates/copypaste-ui/src/styles/tokens.css  (web token names)
 *   • android/.../ui/theme/Color.kt              (CpColors fields + AccentColor variants)
 *
 * Mapping rule (Kotlin → CSS):
 *   camelCase field name → kebab-case with -- prefix
 *   e.g.  cText → --c-text   raised2 → --raised-2   bg → --bg
 *
 * AccentColor enum variants map to data-accent attribute values:
 *   e.g.  INDIGO → data-accent="indigo"
 *
 * Exit codes:
 *   0  all names match (PASS)
 *   1  one or more names are missing on one side (FAIL)
 *
 * Usage:
 *   node scripts/parity-check.mjs            # from repo root
 *   node scripts/parity-check.mjs --verbose  # print all parsed names
 */

import { readFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dir = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dir, "..");

const CSS_PATH = resolve(ROOT, "crates/copypaste-ui/src/styles/tokens.css");
const KT_PATH = resolve(
  ROOT,
  "android/app/src/main/java/com/copypaste/android/ui/theme/Color.kt",
);

const VERBOSE = process.argv.includes("--verbose");

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/**
 * Convert a Kotlin camelCase field name to the CSS custom-property name.
 *   bg       → --bg
 *   cText    → --c-text
 *   raised2  → --raised-2
 *
 * Algorithm: insert hyphen before each uppercase letter and before a
 * letter→digit transition, then lowercase.
 */
function camelToKebab(name) {
  return name
    .replace(/([a-z])([A-Z])/g, "$1-$2") // camelCase → kebab
    .replace(/([a-zA-Z])(\d)/g, "$1-$2") // letter→digit
    .toLowerCase();
}

function fieldToCssToken(field) {
  return "--" + camelToKebab(field);
}

// ---------------------------------------------------------------------------
// CSS parser — extract token names present in the :root block
// ---------------------------------------------------------------------------

/**
 * Extract all custom-property names (--foo) defined in the :root / :root[data-theme="dark"]
 * block of tokens.css.  Returns a Set<string>.
 */
function parseCssTokenNames(src) {
  const names = new Set();
  // Match all --token-name: ... declarations
  const re = /--([a-z][a-z0-9-]*):/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    names.add("--" + m[1]);
  }
  return names;
}

/**
 * Extract data-accent values from tokens.css.
 * Matches: :root[data-accent="indigo"]  → "indigo"
 */
function parseCssAccentValues(src) {
  const values = new Set();
  const re = /\[data-accent="([a-z]+)"\]/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    values.add(m[1]);
  }
  return values;
}

// ---------------------------------------------------------------------------
// Kotlin parser — extract CpColors fields and AccentColor variants
// ---------------------------------------------------------------------------

/**
 * Extract field names from the CpColors data class.
 * Matches: val bg: Color, val cText: Color, etc.
 */
function parseCpColorsFields(src) {
  // Find the data class body
  const classStart = src.indexOf("data class CpColors(");
  if (classStart === -1) throw new Error("Cannot find `data class CpColors(` in Color.kt");

  let depth = 0, i = classStart, bodyStart = -1, bodyEnd = -1, inClass = false;
  for (; i < src.length; i++) {
    if (src[i] === "(") {
      depth++;
      if (!inClass) { inClass = true; bodyStart = i + 1; }
    } else if (src[i] === ")") {
      depth--;
      if (inClass && depth === 0) { bodyEnd = i; break; }
    }
  }
  if (bodyStart === -1 || bodyEnd === -1) throw new Error("Cannot parse CpColors body");

  const body = src.slice(bodyStart, bodyEnd);
  const fields = [];
  const re = /\bval\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*:/g;
  let m;
  while ((m = re.exec(body)) !== null) fields.push(m[1]);
  return fields;
}

/**
 * Extract enum variant names from AccentColor.
 * Matches: INDIGO(...), BLUE(...), etc.
 */
function parseAccentVariants(src) {
  const enumStart = src.indexOf("enum class AccentColor(");
  if (enumStart === -1) throw new Error("Cannot find `enum class AccentColor(` in Color.kt");

  // Find the { } body of the enum
  let braceDepth = 0, i = enumStart, bodyStart = -1, bodyEnd = -1;
  for (; i < src.length; i++) {
    if (src[i] === "{") {
      braceDepth++;
      if (bodyStart === -1) bodyStart = i + 1;
    } else if (src[i] === "}") {
      braceDepth--;
      if (braceDepth === 0) { bodyEnd = i; break; }
    }
  }
  if (bodyStart === -1 || bodyEnd === -1) throw new Error("Cannot parse AccentColor body");

  const body = src.slice(bodyStart, bodyEnd);
  // Match variant names: INDIGO(, BLUE (, etc.
  const variants = [];
  const re = /\b([A-Z][A-Z]+)\s*\(/g;
  let m;
  while ((m = re.exec(body)) !== null) variants.push(m[1]);
  return variants;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  let cssSrc, ktSrc;

  try { cssSrc = readFileSync(CSS_PATH, "utf-8"); }
  catch (e) { console.error(`ERROR: cannot read ${CSS_PATH}: ${e.message}`); process.exit(1); }

  try { ktSrc = readFileSync(KT_PATH, "utf-8"); }
  catch (e) { console.error(`ERROR: cannot read ${KT_PATH}: ${e.message}`); process.exit(1); }

  console.log("Checking token-name parity (web ↔ android)");
  console.log(`  CSS:     ${CSS_PATH.replace(ROOT + "/", "")}`);
  console.log(`  Kotlin:  ${KT_PATH.replace(ROOT + "/", "")}`);
  console.log();

  // Parse
  const cssTokenNames = parseCssTokenNames(cssSrc);
  const cssAccentValues = parseCssAccentValues(cssSrc);
  let cpColorsFields, accentVariants;

  try { cpColorsFields = parseCpColorsFields(ktSrc); }
  catch (e) { console.error(`ERROR: ${e.message}`); process.exit(1); }

  try { accentVariants = parseAccentVariants(ktSrc); }
  catch (e) { console.error(`ERROR: ${e.message}`); process.exit(1); }

  if (VERBOSE) {
    console.log("=== CpColors fields ===");
    cpColorsFields.forEach((f) => console.log(`  ${f}  →  ${fieldToCssToken(f)}`));
    console.log();
    console.log("=== AccentColor variants ===");
    accentVariants.forEach((v) => console.log(`  ${v}  →  data-accent="${v.toLowerCase()}"`));
    console.log();
    console.log("=== CSS token names ===");
    [...cssTokenNames].sort().forEach((t) => console.log(`  ${t}`));
    console.log();
    console.log("=== CSS accent values ===");
    [...cssAccentValues].sort().forEach((v) => console.log(`  data-accent="${v}"`));
    console.log();
  }

  const failures = [];

  // ── Check 1: every CpColors field must have a corresponding CSS token ────────
  for (const field of cpColorsFields) {
    const cssToken = fieldToCssToken(field);
    if (!cssTokenNames.has(cssToken)) {
      failures.push(
        `  FAIL  CpColors.${field} → ${cssToken}  (token absent from tokens.css)`,
      );
    } else if (VERBOSE) {
      console.log(`  PASS  CpColors.${field} → ${cssToken}`);
    }
  }

  // ── Check 2: every CSS --c-* and surface token must have a CpColors field ────
  // Build the reverse map: CSS token → expected Kotlin field
  const cpColorsCssTokens = new Set(cpColorsFields.map(fieldToCssToken));

  // Tokens in CSS that we expect to be in CpColors (surfaces + text + status + c-*)
  // These are the semantic tokens that MUST be paired.  We identify them as any
  // token from the :root dark block EXCEPT utility/computed ones (hover, pressed,
  // selected, scrim, sh1/2/3, card (alias), and font/spacing/radius/duration vars).
  const EXCLUDED = new Set([
    "--card",       // alias for --elevated; not a separate CpColors field
    "--hover",      // computed overlay; not a colour token in CpColors
    "--pressed",    // computed overlay
    "--selected",   // computed from accent
    "--scrim",      // modal backdrop — not in CpColors
    "--focus-ring", // focus affordance (box-shadow), not a colour field
    "--sh1", "--sh2", "--sh3",  // shadows
    "--f-ui", "--f-mono",       // fonts
    "--r-chip", "--r-pill", "--r-ctl", "--r-input", "--r-card", "--r-window",
    "--s-1","--s-2","--s-3","--s-4","--s-5","--s-6","--s-7","--s-8","--s-9",
    "--dur-fast", "--dur", "--dur-theme", "--ease",
    "--accent", "--accent-2", "--on-accent",  // accent tokens (checked separately)
  ]);

  for (const cssToken of cssTokenNames) {
    if (EXCLUDED.has(cssToken)) continue;
    if (!cpColorsCssTokens.has(cssToken)) {
      failures.push(
        `  FAIL  ${cssToken} present in tokens.css but no matching CpColors field`,
      );
    } else if (VERBOSE) {
      console.log(`  PASS  ${cssToken} ← CpColors has matching field`);
    }
  }

  // ── Check 3: every AccentColor variant maps to a data-accent value in CSS ────
  for (const variant of accentVariants) {
    const cssAccent = variant.toLowerCase();
    if (!cssAccentValues.has(cssAccent)) {
      failures.push(
        `  FAIL  AccentColor.${variant} → data-accent="${cssAccent}"  (absent from tokens.css)`,
      );
    } else if (VERBOSE) {
      console.log(`  PASS  AccentColor.${variant} → data-accent="${cssAccent}"`);
    }
  }

  // ── Check 4: every data-accent value maps to an AccentColor variant ──────────
  const accentVariantNames = new Set(accentVariants.map((v) => v.toLowerCase()));
  for (const cssAccent of cssAccentValues) {
    if (!accentVariantNames.has(cssAccent)) {
      failures.push(
        `  FAIL  data-accent="${cssAccent}" in tokens.css but no AccentColor.${cssAccent.toUpperCase()} variant`,
      );
    } else if (VERBOSE) {
      console.log(`  PASS  data-accent="${cssAccent}" ← AccentColor has matching variant`);
    }
  }

  // ── Report ────────────────────────────────────────────────────────────────────
  const totalFields = cpColorsFields.length + accentVariants.length;

  if (failures.length === 0) {
    console.log(
      `PASS: ${totalFields} CpColors fields + ${accentVariants.length} AccentColor variants all map 1:1 to tokens.css.`,
    );
    process.exit(0);
  } else {
    console.log(`FAIL: ${failures.length} token-name mismatch(es):\n`);
    for (const f of failures) console.log(f);
    console.log(
      `\nFix: update tokens.css or Color.kt so every CpColors field has a matching --token\n` +
      `and every AccentColor variant has a matching data-accent value.\n` +
      `Re-run: node scripts/parity-check.mjs --verbose`,
    );
    process.exit(1);
  }
}

main();
