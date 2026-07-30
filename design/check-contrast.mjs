/**
 * Contrast gate over the generated CSS.
 *
 * This exists because the previous revision of design/README.md asserted "AA
 * contrast (A11Y-10)" while eight token pairs were below it — including white
 * on the dark destructive button at 2.75:1, and the focus ring at 1.78:1
 * across every accent. An assertion in prose is not a measurement.
 *
 * It reads dist/, so it checks what ships rather than what the source intends:
 * `--selected` is a color-mix on the live accent, `--hover` is an alpha layer,
 * and both only have a ratio once composited against a real surface.
 *
 * Colour maths comes from culori. Run `npm run check` after `npm run build`.
 */

import { readFileSync } from 'node:fs';
import { parse, converter, wcagContrast } from 'culori';

const DIST = new URL('./dist/css/', import.meta.url);
const rgb = converter('rgb');

const AA_TEXT = 4.5; // WCAG 1.4.3, small text
const NON_TEXT = 3.0; // WCAG 1.4.11, control boundaries and focus indicators

const ACCENTS = ['indigo', 'blue', 'teal', 'green', 'amber', 'rose'];

/* ------------------------------------------------------------- CSS reading */

const read = (file) => readFileSync(new URL(file, DIST), 'utf8');

function declarations(src) {
  const out = {};
  for (const m of src.matchAll(/^\s*--([\w-]+):\s*([^;]+);/gm)) out[m[1]] = m[2].trim();
  return out;
}

/** The `[data-accent="x"]` block, which the flat scan above would collapse. */
function accentBlock(file, accent) {
  const m = new RegExp(`data-accent="${accent}"\\][^{]*\\{([^}]*)\\}`).exec(read(file));
  if (!m) throw new Error(`no [data-accent="${accent}"] block in ${file}`);
  return declarations(m[1]);
}

/* ------------------------------------------------------------ colour maths */

/** Composite `src` (possibly translucent) over opaque `dst`. */
function over(src, dst) {
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
function resolve(expr, vars, depth = 0) {
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

/* ------------------------------------------------------------- the checks */

function pairsFor(theme, accent) {
  const base = declarations(read('tokens.base.css'));
  const vars = {
    ...base,
    ...declarations(read(`tokens.${theme}.css`)),
    ...accentBlock(`accents.${theme}.css`, accent),
  };
  const R = (e) => resolve(e, vars);
  const surface = (e) => {
    const c = R(e);
    return c.alpha === undefined || c.alpha === 1 ? c : over(c, R('var(--bg)'));
  };

  const bg = surface('var(--bg)');
  const panel = surface('var(--panel)');
  const card = surface('var(--card)');
  const elevated = surface('var(--elevated)');
  const selected = over(R('var(--selected)'), bg);
  const hover = over(R('var(--hover)'), bg);
  const withheld = surface('var(--withheld)');

  const pairs = [];
  const text = (fg, on, name) => pairs.push({ fg, on, name, floor: AA_TEXT });
  const nonText = (fg, on, name) => pairs.push({ fg, on, name, floor: NON_TEXT });

  // Body copy on every surface it can land on.
  for (const [n, s] of [['--bg', bg], ['--panel', panel], ['--card', card],
                        ['--elevated', elevated], ['--selected', selected], ['--hover', hover]]) {
    text('var(--text)', s, `--text on ${n}`);
    text('var(--dim)', s, `--dim on ${n}`);
    text('var(--faint)', s, `--faint on ${n}`);
  }

  // Status text on the container tint it is designed to sit on.
  const tintAlpha = parseFloat(base['alpha-tint']);
  for (const s of ['ok', 'warn', 'err', 'info']) {
    const tint = over({ ...R(`var(--${s})`), alpha: tintAlpha }, bg);
    text(`var(--${s}-strong)`, tint, `--${s}-strong on a ${tintAlpha * 100}% --${s} tint`);
    nonText(`var(--${s})`, bg, `--${s} as a dot/fill on --bg`);
  }

  // Content-kind glyphs are icons (non-text); --c-secret also labels text.
  for (const k of ['c-text', 'c-url', 'c-code', 'c-image', 'c-mail', 'c-color',
                   'c-num', 'c-path', 'c-file', 'c-json']) {
    nonText(`var(--${k})`, bg, `--${k} glyph on --bg`);
  }
  for (const s of [bg, elevated, selected]) text('var(--c-secret)', s, '--c-secret as text');

  // Accent: --accent is a fill, --accent-2 is the text step. Keeping these
  // apart is the whole reason --accent-2 exists (see accents.*.json).
  text('var(--on-accent)', R('var(--accent)'), '--on-accent on an --accent fill');
  text('var(--on-err)', R('var(--err)'), '--on-err on an --err fill');
  for (const [n, s] of [['--bg', bg], ['--elevated', elevated]]) {
    text('var(--accent-2)', s, `--accent-2 as text on ${n}`);
  }

  // Withheld: the treatment for content that is absent, not obscured.
  text('var(--withheld-fg)', withheld, '--withheld-fg on --withheld');
  text('var(--withheld-fg)', selected, '--withheld-fg on --selected');
  nonText('var(--withheld-border)', withheld, '--withheld-border on --withheld');

  // Selection is carried by the edge, never by the fill: --selected differs
  // from --bg by 1.09-1.32:1 and from --hover by less than that.
  nonText('var(--selected-edge)', selected, '--selected-edge on --selected');
  nonText('var(--selected-edge)', bg, '--selected-edge on --bg');

  // Focus indicator and control boundaries.
  nonText('var(--accent)', bg, 'focus ring on --bg');
  nonText('var(--accent)', card, 'focus ring on --card');
  for (const [n, s] of [['--bg', bg], ['--card', card], ['--elevated', elevated]]) {
    nonText('var(--border-strong)', s, `--border-strong on ${n}`);
  }

  return pairs.map((p) => {
    const fg = over(R(p.fg), p.on);
    return { ...p, ratio: wcagContrast(fg, p.on) };
  });
}

/* --------------------------------------------------------------- reporting */

const verbose = process.argv.includes('--verbose');
const failures = [];
let checked = 0;

for (const theme of ['dark', 'light']) {
  for (const accent of ACCENTS) {
    for (const p of pairsFor(theme, accent)) {
      checked += 1;
      const ok = p.ratio + 1e-9 >= p.floor;
      if (!ok) failures.push({ theme, accent, ...p });
      if (verbose) {
        console.log(
          `${ok ? '  ' : 'FAIL'} ${theme}/${accent.padEnd(6)} ` +
            `${p.name.padEnd(46)} ${p.ratio.toFixed(2).padStart(6)} (needs ${p.floor})`,
        );
      }
    }
  }
}

if (failures.length) {
  console.error(`\ncontrast: ${failures.length} of ${checked} pairs below floor\n`);
  for (const f of failures) {
    console.error(
      `  ${f.theme}/${f.accent} ${f.name} — ${f.ratio.toFixed(2)}:1, needs ${f.floor}:1`,
    );
  }
  console.error(
    '\nEvery pair here is a rendered combination, not a hypothetical one.\n' +
      'Fix the token, or delete the pair and say in design/README.md why it\n' +
      'is not a real combination.\n',
  );
  process.exit(1);
}

console.log(`contrast: ${checked} pairs clear AA (text 4.5:1, non-text 3:1)`);
