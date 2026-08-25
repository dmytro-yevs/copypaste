/**
 * CSS reading and colour maths over dist/, shared by check-contrast.mjs and
 * check-usage.mjs.
 *
 * Contrast reads dist/ rather than tokens/ on purpose: colour recipes are
 * compiled per product theme and alpha layers have no ratio until they are
 * composited against a real surface.
 */

import { readFileSync } from 'node:fs';
import {
  parse, converter, interpolateWithPremultipliedAlpha, wcagContrast,
} from 'culori';

const DIST = new URL('../dist/css/', import.meta.url);
const rgb = converter('rgb');

/** CSS colour-space names to culori modes. Unlisted spaces throw rather than
 *  being mixed in the wrong one — the space changes the result. */
const MIX_SPACE = {
  oklab: 'oklab',
  oklch: 'oklch',
  lab: 'lab',
  lch: 'lch',
  srgb: 'rgb',
  'srgb-linear': 'lrgb',
  hsl: 'hsl',
};

export const AA_TEXT = 4.5; // WCAG 1.4.3, small text
export const NON_TEXT = 3.0; // WCAG 1.4.11, control boundaries and focus indicators

export const SCHEMES = ['dark', 'light'];
export const PRODUCT_THEMES = ['midnight', 'aurora', 'ember', 'graphite'];

export const read = (file) => readFileSync(new URL(file, DIST), 'utf8');

export function declarations(src) {
  const out = {};
  for (const m of src.matchAll(/^\s*--([\w-]+):\s*([^;]+);/gm)) out[m[1]] = m[2].trim();
  return out;
}

/** One `[data-theme="x"]` block, which the flat scan above would collapse. */
export function productThemeBlock(file, theme) {
  const m = new RegExp(`data-theme="${theme}"\\][^{]*\\{([^}]*)\\}`).exec(read(file));
  if (!m) throw new Error(`no [data-theme="${theme}"] block in ${file}`);
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
 * Resolve a declaration to a colour. The generator uses this for authored
 * recipes and the contrast gate uses it for emitted variables; it handles only
 * the `var()` and two-argument `color-mix()` forms the token set owns.
 */
export function resolve(expr, vars, depth = 0) {
  if (depth > 8) throw new Error(`reference cycle at: ${expr}`);
  expr = expr.trim();

  const v = /^var\(--([\w-]+)\)$/.exec(expr);
  if (v) {
    if (!(v[1] in vars)) throw new Error(`--${v[1]} is referenced but never defined`);
    return resolve(vars[v[1]], vars, depth + 1);
  }

  const mix = /^color-mix\(in ([\w-]+),\s*(.+?)\s+([\d.]+)%,\s*(.+?)\s*\)$/.exec(expr);
  if (mix) {
    const c = resolve(mix[2], vars, depth + 1);
    const pct = parseFloat(mix[3]) / 100;
    if (mix[4].trim() === 'transparent') return { ...c, alpha: (c.alpha ?? 1) * pct };
    const mode = MIX_SPACE[mix[1]];
    if (!mode) throw new Error(`unsupported color-mix space: in ${mix[1]}`);
    return rgb(
      interpolateWithPremultipliedAlpha(
        [c, resolve(mix[4], vars, depth + 1)], mode,
      )(1 - pct),
    );
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
 */
export function context(scheme, theme) {
  const vars = {
    ...declarations(read('tokens.base.css')),
    ...declarations(read(`tokens.${scheme}.css`)),
    ...productThemeBlock(`themes.${scheme}.css`, theme),
  };
  const R = (e) => resolve(e, vars);
  const surface = (e) => {
    const c = R(e);
    return c.alpha === undefined || c.alpha === 1 ? c : over(c, R('var(--bg)'));
  };
  return { vars, R, surface };
}

/** `fg` may be translucent; it is composited onto `on` before measuring. */
export const ratio = (fg, on) => wcagContrast(over(fg, on), on);
