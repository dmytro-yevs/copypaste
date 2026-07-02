#!/usr/bin/env node
// gen-parity-tokens.mjs — generates parity/tokens.json (android-material3-
// redesign task 2.11) from crates/copypaste-ui/src/styles/tokens.css at the
// pinned desktop commit 6960539d (cross-platform-parity.md "Canonical
// machine-readable token source"). tokens.css is parsed directly (a small
// brace-depth CSS-custom-property walk below) — STYLEGUIDE §10/§11 is the
// human-readable mirror, never the generator's input.
//
// Scope note: tokens.css encodes color/radius/spacing/motion/accent custom
// properties but NOT named typography roles (STYLEGUIDE §4 gives ranges,
// e.g. "Title 21-24px", not an exact px) — the exact per-role typography
// values are decided by the android-design-system spec's frozen
// CpTypography table (S1.9), not derivable from tokens.css. The
// `typography` section below is therefore sourced from that already-
// normative Android table (Type.kt), not reverse-engineered from CSS; the
// desktop-side typography parity check is owned by the desktop epic per
// cross-platform-parity.md ("paired type fixture + font-provenance").
//
// Usage: node scripts/gen-parity-tokens.mjs

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(SCRIPT_DIR, "..");
const TOKENS_CSS = join(REPO_ROOT, "crates/copypaste-ui/src/styles/tokens.css");
const OUT_DIR = join(REPO_ROOT, "parity");
const OUT_FILE = join(OUT_DIR, "tokens.json");
const PINNED_COMMIT = "6960539d";

function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Depth-2-inside-@layer rule blocks: {selector, body}, skipping @media/@supports nested content. */
function extractRules(css) {
  const rules = [];
  let depth = 0;
  let buf = "";
  const stack = [];
  for (const ch of css) {
    if (ch === "{") {
      stack.push(buf.trim());
      depth++;
      buf = "";
    } else if (ch === "}") {
      const header = stack.pop();
      const body = buf;
      depth--;
      if (depth === 1 && !header.startsWith("@")) {
        rules.push({ selector: header, body });
      }
      buf = "";
    } else {
      buf += ch;
    }
  }
  return rules;
}

function parseVars(rawBody) {
  // Single-line accent rules (`{--accent:#fff;--on-accent:#fff}`) sometimes
  // omit the trailing `;` on the last declaration — normalize so the regex
  // below (which requires a `;` terminator) still matches it.
  const body = rawBody.trim().endsWith(";") ? rawBody : rawBody + ";";
  const vars = {};
  const re = /--([\w-]+)\s*:\s*([^;]+);/g;
  let m;
  while ((m = re.exec(body))) vars[m[1]] = m[2].trim();
  // Resolve same-block `var(--x)` references (e.g. dark's `--ok-strong: var(--ok)`
  // means "this block's --ok", not the other theme's) to their literal value.
  for (const key of Object.keys(vars)) {
    const ref = vars[key].match(/^var\(--([\w-]+)\)$/);
    if (ref && vars[ref[1]] !== undefined) vars[key] = vars[ref[1]];
  }
  return vars;
}

const css = stripComments(readFileSync(TOKENS_CSS, "utf8"));
const rules = extractRules(css);

const dark = {};
const light = {};
const scale = {};
const accentDefault = {};
const accentLightOverride = {};

for (const { selector, body } of rules) {
  const vars = parseVars(body);
  const hasTheme = selector.includes('data-theme="dark"') || selector.includes('data-theme="light"');
  const accentMatch = selector.match(/data-accent="(\w+)"/);
  if (selector.trim() === ":root" && !accentMatch && !hasTheme) {
    Object.assign(scale, vars);
  } else if (accentMatch && selector.includes('data-theme="light"')) {
    accentLightOverride[accentMatch[1]] = vars;
  } else if (accentMatch) {
    accentDefault[accentMatch[1]] = vars;
  } else if (selector.includes('data-theme="dark"')) {
    Object.assign(dark, vars);
  } else if (selector.includes('data-theme="light"')) {
    Object.assign(light, vars);
  }
}

function themeColors(vars) {
  return {
    surfaces: {
      bg: vars["bg"], panel: vars["panel"], elevated: vars["elevated"], card: vars["card"],
      raised: vars["raised"], raised2: vars["raised-2"],
    },
    lines: { border: vars["border"], divider: vars["divider"] },
    text: { text: vars["text"], dim: vars["dim"], faint: vars["faint"], mute: vars["mute"] },
    overlays: { hover: vars["hover"], pressed: vars["pressed"], scrim: vars["scrim"] },
    status: { ok: vars["ok"], warn: vars["warn"], err: vars["err"], info: vars["info"] },
    statusStrong: { okStrong: vars["ok-strong"], errStrong: vars["err-strong"], infoStrong: vars["info-strong"] },
    content: {
      cText: vars["c-text"], cUrl: vars["c-url"], cCode: vars["c-code"], cImage: vars["c-image"],
      cMail: vars["c-mail"], cColor: vars["c-color"], cNum: vars["c-num"], cPath: vars["c-path"],
      cFile: vars["c-file"], cJson: vars["c-json"], cSecret: vars["c-secret"],
    },
  };
}

const ACCENTS = ["indigo", "blue", "teal", "green", "amber", "rose"];
const accents = ACCENTS.map((name) => {
  const base = accentDefault[name] ?? {};
  const lightOverride = accentLightOverride[name] ?? {};
  return {
    name,
    dark: base["accent"],
    light: lightOverride["accent"] ?? base["accent"],
    onAccent: base["on-accent"],
    onAccentLight: lightOverride["on-accent"] ?? base["on-accent"],
    variant: base["accent-2"],
  };
});

const tokens = {
  sourceCommit: PINNED_COMMIT,
  sourceFile: "crates/copypaste-ui/src/styles/tokens.css",
  theme: {
    dark: themeColors(dark),
    light: themeColors(light),
  },
  accents,
  radii: {
    chip: scale["r-chip"], pill: scale["r-pill"], ctl: scale["r-ctl"],
    input: scale["r-input"], card: scale["r-card"],
  },
  spacing: {
    s1: scale["s-1"], s2: scale["s-2"], s3: scale["s-3"], s4: scale["s-4"], s5: scale["s-5"],
    s6: scale["s-6"], s7: scale["s-7"], s8: scale["s-8"], s9: scale["s-9"],
  },
  motion: {
    durations: { fast: scale["dur-fast"], default: scale["dur"], theme: scale["dur-theme"] },
    easing: scale["ease"],
  },
  alpha: {
    // android-design-system "Selected and disabled treatments are centrally
    // derived": selected tint alpha per theme (16% dark / 12% light — see
    // Color.kt's selectedTint), disabled control opacity 45% (DISABLED_ALPHA).
    selectedDark: 0.16, selectedLight: 0.12, disabled: 0.45,
    contentTileFill: 0.14, // STYLEGUIDE §9.4 content-type tile bg = c-* @14%
  },
  iconSizes: {
    // tokens.css's own icon-size scale (chrome/inline icons) — distinct from
    // the content-type-tile glyph sizing (CpDimensions.glyphBox/tileSm/tileMd),
    // which STYLEGUIDE §8/iconography spec's role table defines separately
    // (see LucideIcons.kt's icon-role size table, task 2.8).
    sm: scale["icon-sm"], md: scale["icon-md"], lg: scale["icon-lg"],
    toggleW: scale["sz-toggle-w"], toggleH: scale["sz-toggle-h"], toggleKnob: scale["sz-toggle-knob"],
  },
  typography: {
    // Sourced from the android-design-system frozen CpTypography table
    // (S1.9 / Type.kt) — see the header comment above for why this is not
    // derived from tokens.css's --fs-* scale.
    title: { family: "Inter", weight: 700, sp: 22, lh: 27, tracking: 0 },
    section: { family: "Inter", weight: 600, sp: 14, lh: 18, tracking: 0.01 },
    body: { family: "Inter", weight: 400, sp: 14, lh: 20, tracking: 0 },
    bodyEmphasis: { family: "Inter", weight: 500, sp: 14, lh: 20, tracking: 0 },
    bodyMono: { family: "JetBrains Mono", weight: 400, sp: 13, lh: 19, tracking: 0 },
    meta: { family: "Inter", weight: 400, sp: 11.5, lh: 16, tracking: 0 },
    micro: { family: "JetBrains Mono", weight: 500, sp: 10, lh: 10, tracking: 0.08 },
  },
};

mkdirSync(OUT_DIR, { recursive: true });
writeFileSync(OUT_FILE, JSON.stringify(tokens, null, 2) + "\n", "utf8");
console.log(`Wrote ${OUT_FILE.replace(REPO_ROOT + "/", "")}`);
