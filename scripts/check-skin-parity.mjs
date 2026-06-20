#!/usr/bin/env node
/**
 * check-skin-parity.mjs — Web ↔ Android skin token parity checker
 *
 * Parses structural skin tokens from:
 *   • crates/copypaste-ui/src/lib/skins.ts   (SkinTokens interface)
 *   • android/.../ui/theme/Skin.kt           (SkinTokens data class)
 *
 * Contract (§2.2 of docs/design/skins-implementation-plan.md):
 *   The web SkinTokens interface fields and the android SkinTokens data class
 *   fields are the canonical contract. Both registries MUST expose the same
 *   canonical token set per skin.
 *
 * Normalization rules (to handle intentional platform naming differences):
 *   1. Strip a trailing "Dp" suffix (android uses dp units in field names,
 *      e.g. "glassBlurDp" → canonical "glassBlur").
 *   2. Lowercase the result.
 *
 * Known intentional platform differences handled by normalization:
 *   • glassBlurDp (android) → glassBlur (web, canonical)
 *
 * Platform-only fields (neither side has any as of V1):
 *   none — both registries cover exactly the same canonical set.
 *
 * Canonical token list (19 tokens):
 *   background, elevation, fillapha, glow, glassblur, material, motionscale,
 *   navactive, radiuscard, radiuschip, radiuscontrol, radiusmodal, rowgap,
 *   rowtreatment, saturation, shadowcard, shadowfloat, sheen, tintalpha
 *
 * Usage:
 *   node scripts/check-skin-parity.mjs          # from repo root
 *   node scripts/check-skin-parity.mjs --verbose # print parsed token sets
 *
 * Exit codes:
 *   0  web and android expose the same canonical token set
 *   1  one or more tokens exist on one side but not the other (drift detected)
 */

import { readFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dir = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dir, "..");

const WEB_PATH = resolve(ROOT, "crates/copypaste-ui/src/lib/skins.ts");
const ANDROID_PATH = resolve(
  ROOT,
  "android/app/src/main/java/com/copypaste/android/ui/theme/Skin.kt"
);

const VERBOSE = process.argv.includes("--verbose");

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/**
 * Normalize a field name to its canonical token key:
 *   1. Strip trailing "Dp" (android platform unit suffix).
 *   2. Lowercase.
 *
 * Examples:
 *   "glassBlurDp" → "glassblur"
 *   "glassBlur"   → "glassblur"
 *   "radiusCard"  → "radiuscard"
 */
function normalize(name) {
  // Strip a trailing "Dp" suffix (case-sensitive — android uses "Dp" exactly)
  const stripped = name.endsWith("Dp") ? name.slice(0, -2) : name;
  return stripped.toLowerCase();
}

// ---------------------------------------------------------------------------
// Web parser — extract fields from the SkinTokens interface in skins.ts
// ---------------------------------------------------------------------------

/**
 * Parse the SkinTokens interface from skins.ts.
 * Matches lines like:
 *   fieldName: SomeType;
 *   fieldName: string;
 *   fieldName: number;
 * inside the `export interface SkinTokens { ... }` block.
 *
 * Returns an array of raw field name strings.
 */
function parseWebFields(src) {
  // Extract the interface body between `export interface SkinTokens {` and its
  // closing `}`. We use a simple brace-counting approach to handle comments.
  const interfaceStart = src.indexOf("export interface SkinTokens");
  if (interfaceStart === -1) {
    throw new Error(
      "Could not find `export interface SkinTokens` in skins.ts"
    );
  }

  let braceDepth = 0;
  let inInterface = false;
  let bodyStart = -1;
  let bodyEnd = -1;

  for (let i = interfaceStart; i < src.length; i++) {
    if (src[i] === "{") {
      braceDepth++;
      if (!inInterface) {
        inInterface = true;
        bodyStart = i + 1;
      }
    } else if (src[i] === "}") {
      braceDepth--;
      if (inInterface && braceDepth === 0) {
        bodyEnd = i;
        break;
      }
    }
  }

  if (bodyStart === -1 || bodyEnd === -1) {
    throw new Error(
      "Could not parse body of `export interface SkinTokens` in skins.ts"
    );
  }

  const body = src.slice(bodyStart, bodyEnd);

  // Match field declarations: optional leading whitespace, then an identifier,
  // then `:`. Ignore comment lines (// ...) and blank lines.
  const fieldPattern = /^\s{0,8}([a-zA-Z_][a-zA-Z0-9_]*)\s*:/gm;
  const fields = [];
  let match;
  while ((match = fieldPattern.exec(body)) !== null) {
    fields.push(match[1]);
  }

  return fields;
}

// ---------------------------------------------------------------------------
// Android parser — extract fields from the SkinTokens data class in Skin.kt
// ---------------------------------------------------------------------------

/**
 * Parse the SkinTokens data class from Skin.kt.
 * Matches parameter declarations like:
 *   val fieldName: SomeType,
 *   val fieldName: Dp,
 * inside the `data class SkinTokens(...)` block.
 *
 * Returns an array of raw field name strings.
 */
function parseAndroidFields(src) {
  // Find `data class SkinTokens(`
  const classStart = src.indexOf("data class SkinTokens(");
  if (classStart === -1) {
    throw new Error("Could not find `data class SkinTokens(` in Skin.kt");
  }

  // Find the matching closing parenthesis
  let parenDepth = 0;
  let inClass = false;
  let bodyStart = -1;
  let bodyEnd = -1;

  for (let i = classStart; i < src.length; i++) {
    if (src[i] === "(") {
      parenDepth++;
      if (!inClass) {
        inClass = true;
        bodyStart = i + 1;
      }
    } else if (src[i] === ")") {
      parenDepth--;
      if (inClass && parenDepth === 0) {
        bodyEnd = i;
        break;
      }
    }
  }

  if (bodyStart === -1 || bodyEnd === -1) {
    throw new Error(
      "Could not parse body of `data class SkinTokens(` in Skin.kt"
    );
  }

  const body = src.slice(bodyStart, bodyEnd);

  // Match `val fieldName:` patterns (Kotlin data class constructor params)
  const fieldPattern = /\bval\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*:/g;
  const fields = [];
  let match;
  while ((match = fieldPattern.exec(body)) !== null) {
    fields.push(match[1]);
  }

  return fields;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

let webSrc, androidSrc;
try {
  webSrc = readFileSync(WEB_PATH, "utf8");
} catch (e) {
  console.error(`ERROR: Could not read web skins file: ${WEB_PATH}`);
  console.error(e.message);
  process.exit(1);
}
try {
  androidSrc = readFileSync(ANDROID_PATH, "utf8");
} catch (e) {
  console.error(`ERROR: Could not read android Skin.kt file: ${ANDROID_PATH}`);
  console.error(e.message);
  process.exit(1);
}

// Parse raw field names
const webRaw = parseWebFields(webSrc);
const androidRaw = parseAndroidFields(androidSrc);

// Normalize to canonical token keys
const webCanonical = webRaw.map(normalize);
const androidCanonical = androidRaw.map(normalize);

// Build sets for comparison
const webSet = new Set(webCanonical);
const androidSet = new Set(androidCanonical);

if (VERBOSE) {
  console.log("=== Web SkinTokens fields (raw → canonical) ===");
  webRaw.forEach((raw, i) => {
    const canon = webCanonical[i];
    const suffix = raw !== canon ? ` → ${canon}` : "";
    console.log(`  ${raw}${suffix}`);
  });
  console.log();
  console.log("=== Android SkinTokens fields (raw → canonical) ===");
  androidRaw.forEach((raw, i) => {
    const canon = androidCanonical[i];
    const suffix = raw !== canon ? ` → ${canon}` : "";
    console.log(`  ${raw}${suffix}`);
  });
  console.log();
  console.log("=== Canonical token set ===");
  const allCanonical = [...new Set([...webSet, ...androidSet])].sort();
  allCanonical.forEach((t) => {
    const inWeb = webSet.has(t) ? "web" : "   ";
    const inAndroid = androidSet.has(t) ? "android" : "       ";
    console.log(`  ${inWeb}  ${inAndroid}  ${t}`);
  });
  console.log();
}

// Detect drift
const onlyInWeb = [...webSet].filter((t) => !androidSet.has(t)).sort();
const onlyInAndroid = [...androidSet].filter((t) => !webSet.has(t)).sort();

const pass = onlyInWeb.length === 0 && onlyInAndroid.length === 0;

if (pass) {
  const count = webSet.size;
  console.log(
    `PASS: web SKINS and android skinTokens expose the same ${count} canonical tokens.`
  );

  if (VERBOSE) {
    const sorted = [...webSet].sort();
    console.log(`\nCanonical token list (${count}):`);
    sorted.forEach((t) => console.log(`  ${t}`));
  }
  process.exit(0);
} else {
  console.error(
    "FAIL: skin token parity check failed — canonical token sets differ.\n"
  );

  if (onlyInWeb.length > 0) {
    console.error(
      `Tokens present in web skins.ts but MISSING from android Skin.kt (${onlyInWeb.length}):`
    );
    onlyInWeb.forEach((t) => console.error(`  - ${t}`));
    console.error();
  }

  if (onlyInAndroid.length > 0) {
    console.error(
      `Tokens present in android Skin.kt but MISSING from web skins.ts (${onlyInAndroid.length}):`
    );
    onlyInAndroid.forEach((t) => console.error(`  - ${t}`));
    console.error();
  }

  console.error(
    "To fix: update skins.ts or Skin.kt so both expose the same canonical token set."
  );
  console.error(
    "Normalization rule: strip trailing 'Dp', lowercase (e.g. glassBlurDp → glassblur)."
  );
  console.error(
    "Do NOT edit these files if this is an intentional platform-only field —"
  );
  console.error(
    "instead, document it as platform-only in scripts/check-skin-parity.mjs."
  );
  process.exit(1);
}
