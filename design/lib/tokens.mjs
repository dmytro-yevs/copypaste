/**
 * CSS reading and colour maths over dist/, shared by check-contrast.mjs and
 * check-usage.mjs.
 *
 * dist/ rather than tokens/ on purpose: `--selected` is a color-mix() on the
 * live accent and `--hover` is an alpha layer, so neither has a ratio until it
 * is composited against a real surface.
 */

import { readFileSync } from 'node:fs';
import { parse, converter, wcagContrast } from 'culori';

const DIST = new URL('../dist/css/', import.meta.url);
const rgb = converter('rgb');

export const AA_TEXT = 4.5; // WCAG 1.4.3, small text
export const NON_TEXT = 3.0; // WCAG 1.4.11, control boundaries and focus indicators

export const THEMES = ['dark', 'light'];
export const ACCENTS = ['indigo', 'blue', 'teal', 'green', 'amber', 'rose'];

export const read = (file) => readFileSync(new URL(file, DIST), 'utf8');

export function declarations(src) {
  const out = {};
  for (const m of src.matchAll(/^\s*--([\w-]+):\s*([^;]+);/gm)) out[m[1]] = m[2].trim();
  return out;
}

/** The `[data-accent="x"]` block, which the flat scan above would collapse. */
export function accentBlock(file, accent) {
  const m = new RegExp(`data-accent="${accent}"\\][^{]*\\{([^}]*)\\}`).exec(read(file));
  if (!m) throw new Error(`no [data-accent="${accent}"] block in ${file}`);
  return declarations(m[1]);
}

/** Composite `src` (possibly translucent) over opaque `dst`. */
export function over(src, dst) {
  const a = src.alpha ?? 1;
  return {
    mode: 'rgb',
    r: src.r * a + dst.r * (1 - a),
    g: src.g * a + dst.g * (1 - a),
    b: src.b * a + dst.b * (1 - a),
  };
}

/**
 * Resolve a declaration to a colour. Handles the two indirections the token
 * set actually uses — `var()` and a two-argument `color-mix()` — rather than
 * being a general CSS evaluator.
 */
export function resolve(expr, vars, depth = 0) {
  if (depth > 8) throw new Error(`reference cycle at: ${expr}`);
  expr = expr.trim();

  const v = /^var\(--([\w-]+)\)$/.exec(expr);
  if (v) {
    if (!(v[1] in vars)) throw new Error(`--${v[1]} is referenced but never defined`);
    return resolve(vars[v[1]], vars, depth + 1);
  }

  const mix = /^color-mix\(in [\w-]+,\s*(.+?)\s+([\d.]+)%,\s*(.+?)\s*\)$/.exec(expr);
  if (mix) {
    const c = resolve(mix[1], vars, depth + 1);
    const pct = parseFloat(mix[2]) / 100;
    if (mix[3].trim() === 'transparent') return { ...c, alpha: (c.alpha ?? 1) * pct };
    const d = resolve(mix[3], vars, depth + 1);
    return {
      mode: 'rgb',
      r: c.r * pct + d.r * (1 - pct),
      g: c.g * pct + d.g * (1 - pct),
      b: c.b * pct + d.b * (1 - pct),
    };
  }

  const parsed = parse(expr);
  if (!parsed) throw new Error(`cannot parse colour: ${expr}`);
  const c = rgb(parsed);
  if (parsed.alpha !== undefined) c.alpha = parsed.alpha;
  return c;
}

/**
 * Everything one (theme, accent) pair resolves to.
 *
 * `aliases` is theme.css's `@theme inline` block, kept out of `vars` for the
 * core pairs and merged only where an alias is what is under test: the point
 * of checking `--color-input` is that a component says `border-input` and
 * never names the token it lands on.
 */
export function context(theme, accent) {
  const vars = {
    ...declarations(read('tokens.base.css')),
    ...declarations(read(`tokens.${theme}.css`)),
    ...accentBlock(`accents.${theme}.css`, accent),
  };
  const withAliases = { ...declarations(read('theme.css')), ...vars };

  const R = (e) => resolve(e, vars);
  const surface = (e) => {
    const c = R(e);
    return c.alpha === undefined || c.alpha === 1 ? c : over(c, R('var(--bg)'));
  };
  return { vars, R, surface, alias: (e) => resolve(e, withAliases) };
}

/** `fg` may be translucent; it is composited onto `on` before measuring. */
export const ratio = (fg, on) => wcagContrast(over(fg, on), on);
