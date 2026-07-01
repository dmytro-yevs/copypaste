import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Exact name-and-value parity between the design reference's token block and
// tokens.css (design.md Decision 11 · task 1.8). Not a name-only diff: every
// custom property the reference defines must resolve to the identical value in
// tokens.css, for both themes and all accent variants. Directional (reference ⊆
// tokens.css) — tokens.css also carries additive tokens (translucency, focus
// ring, typography scale, …) that the reference does not.

// Resolved from the vitest root (the copypaste-ui crate dir = process.cwd()).
// jsdom rewrites import.meta.url to a non-file URL, so file-URL resolution fails.
const REF_HTML = readFileSync(
  resolve(process.cwd(), "../../copypaste-design-reference.html"),
  "utf8",
);
const TOKENS_CSS = readFileSync(
  resolve(process.cwd(), "src/styles/tokens.css"),
  "utf8",
);

const stripComments = (s: string): string => s.replace(/\/\*[\s\S]*?\*\//g, "");

/**
 * Canonicalize a value so parity survives the format-on-save hook (Prettier
 * lowercases hex, adds leading zeros, and trims trailing zeros on decimals):
 * strip whitespace, lowercase, then normalize every numeric literal via
 * parseFloat (`.30`→`0.3`, `0.3`→`0.3`, `.045`→`0.045`; integer/hex runs pass
 * through unchanged).
 */
const canon = (v: string): string =>
  v
    .replace(/\s+/g, "")
    .toLowerCase()
    .replace(/-?\d*\.?\d+/g, (m) => String(parseFloat(m)));

interface Rule {
  key: string;
  props: Record<string, string>;
}

/** Drop `.theme-scope` twins and normalize a selector list into a stable key. */
const normSelector = (sel: string): string =>
  sel
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0 && !s.includes(".theme-scope"))
    .sort()
    .join(",");

const customProps = (declBody: string): Record<string, string> => {
  const out: Record<string, string> = {};
  for (const part of declBody.split(";")) {
    const idx = part.indexOf(":");
    if (idx === -1) continue;
    const prop = part.slice(0, idx).trim();
    if (!prop.startsWith("--")) continue;
    out[prop] = part.slice(idx + 1).trim();
  }
  return out;
};

/**
 * Brace-aware walker. `@layer` is a transparent grouping (no key prefix);
 * `@media`/`@supports` prefix keys with their whitespace-normalized prelude so
 * conditional token overrides (reduced motion, reduced transparency) compare in
 * their own context. Only custom-property declarations are collected.
 */
function parseRules(css: string, atPrefix = ""): Rule[] {
  const rules: Rule[] = [];
  let buf = "";
  let i = 0;
  while (i < css.length) {
    const ch = css[i];
    if (ch !== "{") {
      buf += ch;
      i++;
      continue;
    }
    const prelude = buf.trim();
    let depth = 1;
    let j = i + 1;
    while (j < css.length && depth > 0) {
      if (css[j] === "{") depth++;
      else if (css[j] === "}") depth--;
      j++;
    }
    const body = css.slice(i + 1, j - 1);
    if (prelude.startsWith("@")) {
      const at = prelude.split(/[\s(]/)[0];
      if (at === "@layer") {
        rules.push(...parseRules(body, atPrefix));
      } else {
        const pre = (atPrefix ? atPrefix + " " : "") + prelude.replace(/\s+/g, "");
        rules.push(...parseRules(body, pre));
      }
    } else {
      rules.push({
        key: (atPrefix ? atPrefix + "||" : "") + normSelector(prelude),
        props: customProps(body),
      });
    }
    buf = "";
    i = j;
  }
  return rules;
}

/** Merge rules sharing a key into one prop→value map. */
function collect(rules: Rule[]): Record<string, Record<string, string>> {
  const map: Record<string, Record<string, string>> = {};
  for (const r of rules) {
    if (Object.keys(r.props).length === 0) continue;
    map[r.key] = { ...(map[r.key] ?? {}), ...r.props };
  }
  return map;
}

function refTokenBlock(): string {
  const l1 = REF_HTML.indexOf("LAYER 1 — TOKENS");
  const l2 = REF_HTML.indexOf("LAYER 2 — BASE");
  const firstRoot = REF_HTML.indexOf(":root", l1);
  const block = REF_HTML.slice(firstRoot, l2);
  return block.slice(0, block.lastIndexOf("}") + 1);
}

describe("tokens.css ⇄ copypaste-design-reference.html parity", () => {
  const refMap = collect(parseRules(stripComments(refTokenBlock())));
  const cssMap = collect(parseRules(stripComments(TOKENS_CSS)));

  it("parses a non-trivial number of reference token rules", () => {
    // Guards against the extraction silently yielding nothing (which would make
    // the parity assertions vacuously pass).
    expect(Object.keys(refMap).length).toBeGreaterThanOrEqual(14);
  });

  it("every reference custom property resolves to the identical value in tokens.css", () => {
    const mismatches: string[] = [];
    for (const [key, props] of Object.entries(refMap)) {
      const cssProps = cssMap[key];
      if (!cssProps) {
        mismatches.push(`missing selector block "${key}"`);
        continue;
      }
      for (const [prop, refVal] of Object.entries(props)) {
        const cssVal = cssProps[prop];
        if (cssVal === undefined) {
          mismatches.push(`${key} { ${prop} } missing in tokens.css`);
        } else if (canon(cssVal) !== canon(refVal)) {
          mismatches.push(
            `${key} { ${prop}: ${refVal} } ≠ tokens.css value ${cssVal}`,
          );
        }
      }
    }
    expect(mismatches).toEqual([]);
  });

  it("covers both themes and all six accents", () => {
    const keys = Object.keys(refMap);
    expect(keys.some((k) => k.includes('[data-theme="dark"]'))).toBe(true);
    expect(keys.some((k) => k.includes('[data-theme="light"]'))).toBe(true);
    for (const accent of ["indigo", "blue", "teal", "green", "amber", "rose"]) {
      expect(keys.some((k) => k.includes(`[data-accent="${accent}"]`))).toBe(true);
    }
  });

  it("every attributed :root selector has a matching .theme-scope twin (gallery isolation)", () => {
    // Guards design.md Decision 7/A6: the parity check above deliberately strips
    // .theme-scope, so it cannot catch a dropped twin. This asserts every themed/
    // accent/translucency `:root[…]` selector is mirrored on `.theme-scope[…]`
    // (and vice-versa). Bare `:root` (scale/defaults, which inherit) is exempt —
    // only attribute-carrying selectors are compared.
    // Strip comments first — the header prose mentions `.theme-scope[…]`.
    const css = stripComments(TOKENS_CSS);
    const chains = (re: RegExp): Set<string> => {
      const out = new Set<string>();
      for (const m of css.matchAll(re)) out.add(m[1].replace(/\s+/g, ""));
      return out;
    };
    const rootChains = chains(/:root((?:\[[^\]]*\])+)/g);
    const scopeChains = chains(/\.theme-scope((?:\[[^\]]*\])+)/g);
    expect(rootChains.size).toBeGreaterThanOrEqual(14); // 2 themes + 6 accents + 6 light-accents
    // Every attributed :root selector must be mirrored on .theme-scope …
    expect([...rootChains].filter((c) => !scopeChains.has(c))).toEqual([]);
    // … and no orphan .theme-scope twin lacks its :root counterpart.
    expect([...scopeChains].filter((c) => !rootChains.has(c))).toEqual([]);
  });
});
