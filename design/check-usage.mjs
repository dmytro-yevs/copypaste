/**
 * Usage gate over the component tree and the generated CSS structure.
 *
 * check-contrast.mjs reads token values. A component can hold every one of
 * them fixed and still ship a defect: `ring-ring/50` puts the alpha back at
 * the call site, `text-primary` uses a fill token in a text role,
 * `hover:bg-primary/90` invents a colour that is in no token file. All three
 * are shadcn defaults, and `npx shadcn@latest add` restores them.
 *
 * It also asserts the absences that are requirements — no blur over withheld
 * content, no hand-rolled opacity mask — and the one deliberate absence that
 * looks like an oversight: rows have no coarse-pointer variant on purpose.
 *
 * Run `npm run check`. `--verbose` lists what passed and the exemptions.
 */

import { readdirSync, readFileSync, statSync } from 'node:fs';
import {
  AA_TEXT, SCHEMES, PRODUCT_THEMES, context, over, productThemeBlock, ratio, read,
} from './lib/tokens.mjs';
import {
  SRC, SCANNED, NOT_SCANNED, CLASS_RULES, REQUIRED_UTILITIES, ALPHA_UTILITIES,
  COARSE_TOKENS, EXEMPTIONS, exemptions, isExempt,
} from './lib/component-usage.mjs';

const problems = [];
const notes = [];
const fail = (where, what, fix, why) => problems.push({ where, what, fix, why });

/* ---------------------------------------------------------------- the tree */

/**
 * The consuming source is a sibling package, so a missing tree means the check
 * silently stopped checking. That is louder as a crash than as a pass.
 */
function sources() {
  const root = new URL(SRC, import.meta.url);
  let entries;
  try {
    entries = readdirSync(root, { recursive: true, encoding: 'utf8' });
  } catch (e) {
    throw new Error(`cannot read ${SRC} — the component tree this gate exists to check: ${e.message}`);
  }
  const out = [];
  for (const rel of entries.sort()) {
    const path = rel.split('\\').join('/');
    if (!SCANNED.test(path) || NOT_SCANNED.test(path)) continue;
    const abs = new URL(path, root);
    if (!statSync(abs).isFile()) continue;
    out.push({ path, lines: readFileSync(abs, 'utf8').split('\n') });
  }
  if (!out.length) throw new Error(`${SRC} matched no source files`);
  return out;
}

const files = sources();

/* CSS literals belong in generated design tokens. The one source-level
 * exemption is the breakpoint contract, which PostCSS expands before shipping. */
const RAW_CSS_LITERAL = /#[\da-fA-F]{3,8}\b|\b\d+(?:px|rem|ms|s)\b/g;
let rawCssHits = 0;
for (const { path, lines } of files) {
  if (!path.endsWith('.css') || path === 'styles/media.css') continue;
  for (const [i, line] of lines.entries()) {
    for (const match of line.matchAll(RAW_CSS_LITERAL)) {
      rawCssHits += 1;
      fail(`${path}:${i + 1}`, match[0], 'use a generated design token or a named media token',
        'Shipping CSS must not carry one-off colours, dimensions or motion literals.');
    }
  }
}
notes.push(`raw-css-literals              ${rawCssHits ? `${rawCssHits} hit(s)` : 'clear'} (media token source exempt)`);

/* ------------------------------------------------------- class-level rules */

for (const rule of CLASS_RULES) {
  let hits = 0;
  let waived = 0;
  for (const { path, lines } of files) {
    if (isExempt(rule.id, path)) { waived += 1; continue; }
    for (const [i, line] of lines.entries()) {
      if (rule.applies && !rule.applies(path, line)) continue;
      for (const m of line.matchAll(rule.test)) {
        hits += 1;
        fail(`${path}:${i + 1}`, m[0], rule.fix, rule.why);
      }
    }
  }
  notes.push(`${rule.id.padEnd(32)} ${hits ? `${hits} hit(s)` : 'clear'}`
    + (waived ? `, ${waived} file(s) exempt` : ''));
}

/** An exemption for a file that has moved is an exemption for nothing, and the
 *  rule it silences comes back unannounced. */
for (const e of EXEMPTIONS) {
  for (const f of e.files ?? []) {
    if (!files.some((s) => s.path === f)) {
      fail('design/lib/component-usage.mjs', `exemption for ${e.rule} names ${f}, which is not in the tree`,
        'delete the exemption, or point it at the file that replaced it',
        'A stale exemption silences a rule for a path nothing matches, so the rule reads as '
        + 'satisfied everywhere it now applies.');
    }
  }
}

/* --------------------------------------------- absences that are contracts */

const corpus = files.map((f) => f.lines.join('\n')).join('\n');
for (const { util, token, why } of REQUIRED_UTILITIES) {
  const utility = new RegExp(`(?<![\\w-])${util}(?![\\w-])`);
  const variable = new RegExp(`var\\(--${token}\\)`);
  if (!utility.test(corpus) && !variable.test(corpus)) {
    fail(SRC, `no component uses ${util}`, `use ${util}`,
      `${why}. A token nothing reaches is a token whose job something else has quietly taken over.`);
  }
}
notes.push(`required-utilities            ${REQUIRED_UTILITIES.length} present`);

function tokenSources(dir = new URL('./tokens/', import.meta.url), rel = 'tokens/', out = []) {
  for (const name of readdirSync(dir).sort()) {
    const child = new URL(name, dir);
    if (statSync(child).isDirectory()) {
      tokenSources(new URL(`${name}/`, dir), `${rel}${name}/`, out);
      continue;
    }
    if (!name.endsWith('.json')) continue;
    out.push({ path: `${rel}${name}`, source: readFileSync(child, 'utf8') });
  }
  return out;
}

const tokenFiles = tokenSources();
const blurAllowed = exemptions('no-blur-token').map((e) => e.token);
for (const { path, source } of tokenFiles) {
  for (const m of source.matchAll(/"([\w-]*blur[\w-]*)"\s*:\s*\{/g)) {
    if (blurAllowed.includes(m[1])) continue;
    fail(path, `token ${m[1]}`, 'delete it; the content is absent, not filtered',
      'A blur token is the treatment INV-10 rules out — it asserts the plaintext is present '
      + 'behind a filter.');
  }
}
notes.push(`no-blur-token                 clear (exempt: ${blurAllowed.join(', ')})`);

let recipeSourceMixes = 0;
const dynamicTokenPaths = (value, path = [], out = []) => {
  if (!value || typeof value !== 'object') return out;
  if (typeof value.$value === 'string' && value.$value.includes('color-mix(')) {
    out.push(path.join('.'));
  }
  for (const [key, child] of Object.entries(value)) {
    if (!key.startsWith('$')) dynamicTokenPaths(child, [...path, key], out);
  }
  return out;
};
for (const { path, source } of tokenFiles) {
  const mixes = dynamicTokenPaths(JSON.parse(source));
  recipeSourceMixes += mixes.length;
  if (mixes.length && !path.startsWith('tokens/color/recipes')) {
    fail(path, `color-mix() outside the recipe sources: ${mixes.join(', ')}`,
      'move the expression to tokens/color/recipes*.json and consume its generated variable',
      'Only the recipe compiler produces a concrete value for legacy WebViews. A mix elsewhere '
      + 'can reach generated CSS without that compatibility step.');
  }
}
notes.push(`recipe-source-mixes           ${recipeSourceMixes} centralized`);

const recipeFile = (path) => {
  const file = tokenFiles.find((candidate) => candidate.path === path);
  if (!file) throw new Error(`missing recipe source: ${path}`);
  return JSON.parse(file.source).recipe;
};
const commonRecipes = recipeFile('tokens/color/recipes.json');
const recipeNames = (group) => Object.keys(group ?? {});
const hasConcreteRecipe = (source, name) => {
  const values = [...source.matchAll(new RegExp(`^\\s*--${name}:\\s*([^;]+);`, 'gm'))]
    .map((match) => match[1]);
  return values.some((value) => !/\b(?:color-mix|var)\(/.test(value));
};

let generatedRecipeContexts = 0;
for (const scheme of SCHEMES) {
  const schemeRecipes = recipeFile(`tokens/color/recipes.${scheme}.json`);
  const allNames = [
    ...recipeNames(commonRecipes.scheme), ...recipeNames(commonRecipes.theme),
    ...recipeNames(schemeRecipes.scheme), ...recipeNames(schemeRecipes.theme),
  ];
  const schemeCss = read(`tokens.${scheme}.css`);
  generatedRecipeContexts += 1;
  for (const name of allNames) {
    if (!hasConcreteRecipe(schemeCss, name)) {
      fail(`dist/css/tokens.${scheme}.css`, `--${name} has no concrete scheme fallback`,
        'emit the recipe through css/copypaste for this scheme',
        'The scheme block is the pre-product-theme fallback and must work before bootstrap finishes.');
    }
  }

  const themeNames = [...recipeNames(commonRecipes.theme), ...recipeNames(schemeRecipes.theme)];
  for (const theme of PRODUCT_THEMES) {
    const block = productThemeBlock(`themes.${scheme}.css`, theme);
    const source = Object.entries(block).map(([name, value]) => `--${name}: ${value};`).join('\n');
    generatedRecipeContexts += 1;
    for (const name of themeNames) {
      if (!hasConcreteRecipe(source, name)) {
        fail(`dist/css/themes.${scheme}.css [data-theme="${theme}"]`,
          `--${name} has no concrete product-theme fallback`,
          'emit the recipe through css/copypaste-themes for every product theme',
          'A scheme-default value would make this theme inherit Midnight colour maths.');
      }
    }
  }
}
notes.push(`recipe-fallback-contexts      ${generatedRecipeContexts} concrete palette block(s)`);

/* ---------------------------------------------- coarse pointer, not width */

const base = read('tokens.base.css');
const coarse = /@media \(pointer: coarse\)\s*\{([^}]*)\}/.exec(base);
if (!coarse) {
  fail('dist/css/tokens.base.css', 'no @media (pointer: coarse) block',
    're-emit --tap-min, --hit-slop and --sz-iconbtn there',
    'Touch targets key off the pointer, not the viewport: a narrow window on a Mac is still a mouse.');
} else {
  const declared = [...coarse[1].matchAll(/--([\w-]+):/g)].map((m) => m[1]).sort();
  const want = [...COARSE_TOKENS].sort();
  if (declared.join(',') !== want.join(',')) {
    const extra = declared.filter((d) => !want.includes(d));
    const missing = want.filter((w) => !declared.includes(w));
    fail('dist/css/tokens.base.css', `@media (pointer: coarse) declares [${declared}]`,
      `declare exactly [${want}]`,
      (extra.length ? `${extra.map((e) => `--${e}`).join(', ')} must not vary by pointer. ` : '')
      + (missing.length ? `${missing.map((m) => `--${m}`).join(', ')} is the coarse floor and is missing. ` : '')
      + exemptions('coarse-pointer-set').map((e) => `${e.token}: ${e.why}`).join(' '));
  }
}

for (const file of ['tokens.base.css', 'tokens.dark.css', 'tokens.light.css']) {
  for (const m of read(file).matchAll(/@media \(([^)]*width[^)]*)\)\s*\{([^}]*)\}/g)) {
    const clash = COARSE_TOKENS.filter((t) => new RegExp(`--${t}:`).test(m[2]));
    if (clash.length) {
      fail(`dist/css/${file}`, `@media (${m[1]}) redefines ${clash.map((c) => `--${c}`).join(', ')}`,
        'key touch sizing off (pointer: coarse)',
        'A 380px window on a Mac is driven by a mouse, and a phone in landscape is not.');
    }
  }
}
notes.push(`coarse-pointer-set            exactly [${COARSE_TOKENS}], --pad-row-y deliberately absent`);

/* A modern dynamic enhancement is allowed only after the same property has a
 * concrete declaration. Current generated output intentionally needs none. */
const generatedCss = [
  'tokens.base.css', 'tokens.dark.css', 'tokens.light.css',
  'themes.dark.css', 'themes.light.css', 'swatches.dark.css', 'swatches.light.css',
];
let emittedMixes = 0;
for (const file of generatedCss) {
  const lines = read(file).split('\n');
  for (const [index, line] of lines.entries()) {
    if (!line.includes('color-mix(')) continue;
    emittedMixes += 1;
    const dynamic = /^\s*--([\w-]+):\s*(.+);\s*$/.exec(line);
    let previous = index - 1;
    while (previous >= 0 && !lines[previous].trim()) previous -= 1;
    const fallback = previous >= 0
      ? /^\s*--([\w-]+):\s*(.+);\s*$/.exec(lines[previous])
      : null;
    if (!dynamic || !fallback || dynamic[1] !== fallback[1]
        || /\b(?:color-mix|var)\(/.test(fallback[2])) {
      fail(`dist/css/${file}:${index + 1}`, line.trim(),
        'emit the same custom property with a concrete colour immediately before this declaration',
        'Chrome 53/74 ignores color-mix(). Its cascade needs a concrete declaration that survives '
        + 'when the optional modern enhancement is rejected.');
    }
  }
}
notes.push(`generated-color-mix           ${emittedMixes} optional enhancement(s)`);

/* ------------------------------- fills a component dilutes at the call site */

const found = new Map();
for (const { path, lines } of files) {
  for (const [i, line] of lines.entries()) {
    for (const m of line.matchAll(/(?<![\w-])(?:bg|text|border|ring|fill|stroke|decoration)-[a-z][a-z0-9-]*\/\d+(?![\w-])/g)) {
      if (!found.has(m[0])) found.set(m[0], []);
      found.get(m[0]).push(`${path}:${i + 1}`);
    }
  }
}

for (const [util, sites] of [...found].sort()) {
  const known = ALPHA_UTILITIES.find((u) => u.util === util);
  if (!known) {
    fail(sites.join(', '), util,
      'measure it in design/lib/component-usage.mjs, or drop the /N',
      'An alpha on a colour utility is a colour that exists in no token file, so it has a ratio '
      + 'only once composited. Add it to ALPHA_UTILITIES with either a `measure` or a stated '
      + 'reason it carries no floor.');
    continue;
  }
  if (!known.measure) continue;

  const { fill, alpha, fg, on } = known.measure;
  let worst = null;
  for (const scheme of SCHEMES) {
    for (const theme of PRODUCT_THEMES) {
      const { R, surface } = context(scheme, theme);
      for (const n of on) {
        const composited = over({ ...R(fill), alpha }, surface(`var(${n})`));
        const r = ratio(R(fg), composited);
        if (!worst || r < worst.r) worst = { r, scheme, theme, n };
      }
    }
  }
  if (worst.r + 1e-9 < AA_TEXT) {
    fail(sites.join(', '), `${util} puts ${fg.slice(4, -1)} at ${worst.r.toFixed(2)}:1`,
      'do not dilute a fill that carries a label — the hover state needs its own token, '
      + 'not an alpha at the call site',
      `Worst over ${worst.n} on ${worst.scheme}/${worst.theme}, against a ${AA_TEXT} floor. `
      + `${fg.slice(4, -1)} was measured against the undiluted fill only, so the token gate is green.`);
  }
  notes.push(`${util.padEnd(32)} worst ${worst.r.toFixed(2)}:1 (${worst.scheme}/${worst.theme} over ${worst.n})`);
}

/* --------------------------------------------------------------- reporting */

if (process.argv.includes('--verbose')) {
  for (const n of notes) console.log(`  ${n}`);
  for (const e of ALPHA_UTILITIES.filter((u) => u.decorative || u.covered)) {
    console.log(`  ${e.util.padEnd(32)} ${e.decorative ? 'no floor' : `covered by ${e.covered}`}`);
  }
  for (const e of EXEMPTIONS) {
    console.log(`  exempt: ${e.rule} — ${(e.files ?? [e.token]).join(', ')}\n            ${e.why}`);
  }
}

if (problems.length) {
  console.error(`\nusage: ${problems.length} problem(s) in ${SRC}\n`);
  for (const p of problems) {
    console.error(`  ${p.where}\n    ${p.what}\n    → ${p.fix}\n    ${p.why}\n`);
  }
  console.error(
    'Every line above is in the shipping component tree, not in a token file, so\n'
      + 'check-contrast.mjs is green while it is wrong. Fix the component, or record\n'
      + 'the exemption in design/lib/component-usage.mjs with the reason.\n',
  );
  process.exit(1);
}

console.log(
  `usage: ${files.length} files clear ${CLASS_RULES.length} class rules, `
    + `${found.size} alpha utilities measured, coarse sizing keys off the pointer`,
);
