/**
 * Phase 7 (CopyPaste-2hfj.8) — Token-parity + AA-contrast vitest gate.
 *
 * 1. TOKEN PARITY: verifies that every CpColors field in Color.kt maps to a CSS
 *    custom-property in tokens.css (and vice-versa for the semantic set), and that
 *    every AccentColor variant maps to a data-accent= value.
 *
 * 2. AA CONTRAST: verifies that the §3.3 text ramp (--text / --dim / --faint on
 *    --panel) meets WCAG AA in both dark and light themes.
 *    • --text, --dim  → ≥ 4.5:1 (body text)
 *    • --faint        → ≥ 3.0:1 (large/UI affordances, §7)
 *
 * Both checks read source files from the filesystem; no import of app code needed.
 */

import { readFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { describe, it, expect } from "vitest";

// Derive __dirname from import.meta.url (ESM safe)
const _thisDir = dirname(fileURLToPath(import.meta.url));

// Resolve paths relative to the repo root (this file lives in crates/copypaste-ui/src)
const REPO_ROOT = resolve(_thisDir, "../../..");
const CSS_PATH = resolve(REPO_ROOT, "crates/copypaste-ui/src/styles/tokens.css");
const KT_PATH = resolve(
  REPO_ROOT,
  "android/app/src/main/java/com/copypaste/android/ui/theme/Color.kt",
);

// ---------------------------------------------------------------------------
// Shared source text (lazy-loaded once per suite)
// ---------------------------------------------------------------------------

let _cssSrc: string | null = null;
let _ktSrc: string | null = null;

function cssSrc(): string {
  if (!_cssSrc) _cssSrc = readFileSync(CSS_PATH, "utf-8");
  return _cssSrc;
}
function ktSrc(): string {
  if (!_ktSrc) _ktSrc = readFileSync(KT_PATH, "utf-8");
  return _ktSrc;
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/** Convert camelCase Kotlin field name to CSS custom-property name. */
function camelToKebab(name: string): string {
  return name
    .replace(/([a-z])([A-Z])/g, "$1-$2")
    .replace(/([a-zA-Z])(\d)/g, "$1-$2")
    .toLowerCase();
}

function fieldToCssToken(field: string): string {
  return "--" + camelToKebab(field);
}

/** Extract all --token names defined in tokens.css. */
function parseCssTokenNames(src: string): Set<string> {
  const names = new Set<string>();
  const re = /--([a-z][a-z0-9-]*):/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) !== null) names.add("--" + m[1]);
  return names;
}

/** Extract data-accent="X" values from tokens.css. */
function parseCssAccentValues(src: string): Set<string> {
  const values = new Set<string>();
  const re = /\[data-accent="([a-z]+)"\]/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) !== null) values.add(m[1]);
  return values;
}

/** Extract field names from the CpColors data class in Color.kt. */
function parseCpColorsFields(src: string): string[] {
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
  const fields: string[] = [];
  const re = /\bval\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*:/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(body)) !== null) fields.push(m[1]);
  return fields;
}

/** Extract AccentColor variant names from Color.kt. */
function parseAccentVariants(src: string): string[] {
  const enumStart = src.indexOf("enum class AccentColor(");
  if (enumStart === -1) throw new Error("Cannot find `enum class AccentColor(` in Color.kt");
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
  const variants: string[] = [];
  const re = /\b([A-Z][A-Z]+)\s*\(/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(body)) !== null) variants.push(m[1]);
  return variants;
}

// ---------------------------------------------------------------------------
// WCAG helpers (inline — no external import)
// ---------------------------------------------------------------------------

function toLinear(c: number): number {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

function luminance([r, g, b]: [number, number, number]): number {
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
}

function contrastRatio(fg: [number, number, number], bg: [number, number, number]): number {
  const L1 = Math.max(luminance(fg), luminance(bg));
  const L2 = Math.min(luminance(fg), luminance(bg));
  return (L1 + 0.05) / (L2 + 0.05);
}

/**
 * Parse a #RRGGBB hex colour from tokens.css.
 * Looks for "--token:#RRGGBB" in the given CSS block text.
 */
function parseTokenHex(block: string, token: string): [number, number, number] | null {
  // Match --token: #RRGGBB with optional whitespace
  const re = new RegExp(`${token.replace("-", "\\-")}\\s*:\\s*#([0-9a-fA-F]{6})`, "i");
  const m = block.match(re);
  if (!m) return null;
  const hex = m[1];
  return [
    parseInt(hex.slice(0, 2), 16),
    parseInt(hex.slice(2, 4), 16),
    parseInt(hex.slice(4, 6), 16),
  ];
}

/**
 * Extract the text of a specific CSS block from tokens.css.
 * E.g. the `:root[data-theme="light"]{ ... }` block.
 */
function extractCssBlock(src: string, selector: string): string {
  const start = src.indexOf(selector);
  if (start === -1) return "";
  const braceStart = src.indexOf("{", start);
  if (braceStart === -1) return "";
  let depth = 0, i = braceStart, end = -1;
  for (; i < src.length; i++) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") { depth--; if (depth === 0) { end = i; break; } }
  }
  return end === -1 ? "" : src.slice(braceStart, end + 1);
}

// ---------------------------------------------------------------------------
// §1 Token parity tests
// ---------------------------------------------------------------------------

describe("§1 Token-name parity: CpColors ↔ tokens.css", () => {
  // Tokens that exist in CSS but are not CpColors fields (utility / computed / alias).
  const EXCLUDED = new Set([
    "--card",        // alias for --elevated
    "--hover",       // computed overlay
    "--pressed",     // computed overlay
    "--selected",    // computed from accent
    "--scrim",       // modal backdrop
    "--sh1", "--sh2", "--sh3",
    "--f-ui", "--f-mono",
    "--r-chip", "--r-pill", "--r-ctl", "--r-input", "--r-card", "--r-window",
    "--s-1","--s-2","--s-3","--s-4","--s-5","--s-6","--s-7","--s-8","--s-9",
    "--dur-fast", "--dur", "--dur-theme", "--ease",
    "--accent", "--accent-2", "--on-accent",  // accent (checked separately)
  ]);

  it("every CpColors field maps to a CSS custom property", () => {
    const fields = parseCpColorsFields(ktSrc());
    const cssNames = parseCssTokenNames(cssSrc());
    const missing = fields.filter((f) => !cssNames.has(fieldToCssToken(f)));
    expect(
      missing,
      `CpColors fields missing from tokens.css: ${missing.map((f) => `${f} → ${fieldToCssToken(f)}`).join(", ")}`,
    ).toEqual([]);
  });

  it("every semantic CSS token maps to a CpColors field", () => {
    const fields = parseCpColorsFields(ktSrc());
    const cssNames = parseCssTokenNames(cssSrc());
    const cpTokens = new Set(fields.map(fieldToCssToken));
    const missing = [...cssNames].filter(
      (t) => !EXCLUDED.has(t) && !cpTokens.has(t),
    );
    expect(
      missing,
      `tokens.css tokens without a CpColors field: ${missing.join(", ")}`,
    ).toEqual([]);
  });
});

describe("§1 Token-name parity: AccentColor ↔ data-accent values", () => {
  it("every AccentColor variant maps to a data-accent value in CSS", () => {
    const variants = parseAccentVariants(ktSrc());
    const cssAccents = parseCssAccentValues(cssSrc());
    const missing = variants.filter((v) => !cssAccents.has(v.toLowerCase()));
    expect(
      missing,
      `AccentColor variants missing from tokens.css: ${missing.map((v) => `${v} → data-accent="${v.toLowerCase()}"`).join(", ")}`,
    ).toEqual([]);
  });

  it("every data-accent value maps to an AccentColor variant", () => {
    const variants = parseAccentVariants(ktSrc());
    const cssAccents = parseCssAccentValues(cssSrc());
    const variantNames = new Set(variants.map((v) => v.toLowerCase()));
    const missing = [...cssAccents].filter((a) => !variantNames.has(a));
    expect(
      missing,
      `CSS data-accent values without an AccentColor variant: ${missing.map((a) => `data-accent="${a}" → AccentColor.${a.toUpperCase()}`).join(", ")}`,
    ).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// §2 AA contrast: §3.3 text ramp on --panel (STYLEGUIDE §7)
// ---------------------------------------------------------------------------

const AA_BODY = 4.5;   // normal body text
const AA_LARGE = 3.0;  // large / UI affordances

describe("§2 AA contrast: dark theme text ramp on --panel", () => {
  const darkBlock = (): string =>
    extractCssBlock(cssSrc(), ':root, :root[data-theme="dark"]');

  it("--text on --panel ≥ 4.5:1 (body)", () => {
    const block = darkBlock();
    const panel = parseTokenHex(block, "--panel");
    const text = parseTokenHex(block, "--text");
    expect(panel, "--panel not found in dark block").not.toBeNull();
    expect(text, "--text not found in dark block").not.toBeNull();
    const ratio = contrastRatio(text!, panel!);
    expect(ratio).toBeGreaterThanOrEqual(AA_BODY);
  });

  it("--dim on --panel ≥ 4.5:1 (body/secondary)", () => {
    const block = darkBlock();
    const panel = parseTokenHex(block, "--panel");
    const dim = parseTokenHex(block, "--dim");
    expect(panel, "--panel not found in dark block").not.toBeNull();
    expect(dim, "--dim not found in dark block").not.toBeNull();
    const ratio = contrastRatio(dim!, panel!);
    expect(ratio).toBeGreaterThanOrEqual(AA_BODY);
  });

  it("--faint on --panel ≥ 3.0:1 (large/UI affordances)", () => {
    const block = darkBlock();
    const panel = parseTokenHex(block, "--panel");
    const faint = parseTokenHex(block, "--faint");
    expect(panel, "--panel not found in dark block").not.toBeNull();
    expect(faint, "--faint not found in dark block").not.toBeNull();
    const ratio = contrastRatio(faint!, panel!);
    expect(ratio).toBeGreaterThanOrEqual(AA_LARGE);
  });
});

describe("§2 AA contrast: light theme text ramp on --panel", () => {
  const lightBlock = (): string =>
    extractCssBlock(cssSrc(), ':root[data-theme="light"]');

  it("--text on --panel ≥ 4.5:1 (body)", () => {
    const block = lightBlock();
    const panel = parseTokenHex(block, "--panel");
    const text = parseTokenHex(block, "--text");
    expect(panel, "--panel not found in light block").not.toBeNull();
    expect(text, "--text not found in light block").not.toBeNull();
    const ratio = contrastRatio(text!, panel!);
    expect(ratio).toBeGreaterThanOrEqual(AA_BODY);
  });

  it("--dim on --panel ≥ 4.5:1 (body/secondary)", () => {
    const block = lightBlock();
    const panel = parseTokenHex(block, "--panel");
    const dim = parseTokenHex(block, "--dim");
    expect(panel, "--panel not found in light block").not.toBeNull();
    expect(dim, "--dim not found in light block").not.toBeNull();
    const ratio = contrastRatio(dim!, panel!);
    expect(ratio).toBeGreaterThanOrEqual(AA_BODY);
  });

  it("--faint on --panel ≥ 3.0:1 (large/UI affordances)", () => {
    const block = lightBlock();
    const panel = parseTokenHex(block, "--panel");
    const faint = parseTokenHex(block, "--faint");
    expect(panel, "--panel not found in light block").not.toBeNull();
    expect(faint, "--faint not found in light block").not.toBeNull();
    const ratio = contrastRatio(faint!, panel!);
    expect(ratio).toBeGreaterThanOrEqual(AA_LARGE);
  });
});
