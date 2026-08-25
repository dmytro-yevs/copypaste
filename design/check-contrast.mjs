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
 * Token pairs are not the whole contract. A component can hold every token
 * value fixed and still land below the floor — by putting an alpha on the
 * focus ring, or using a fill token in a text role — so check-usage.mjs reads
 * the component tree and this file does not.
 *
 * Colour maths comes from culori. Run `npm run check` after `npm run build`.
 */

import { AA_TEXT, NON_TEXT, SCHEMES, PRODUCT_THEMES, context, over, ratio } from './lib/tokens.mjs';

function pairsFor(scheme, theme) {
  const { vars, R, surface } = context(scheme, theme);

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
  const tintAlpha = parseFloat(vars['alpha-tint']);
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

  // The theme owns separate fill and text-safe brand roles.
  text('var(--on-accent)', R('var(--accent)'), '--on-accent on an --accent fill');
  text('var(--on-err)', R('var(--err)'), '--on-err on an --err fill');

  // The hover fill. The two pairs above measured the *undiluted* fill only,
  // which is how a button could hover at 4.02 with every pair here green.
  // Mixing away from the ink means these can only sit above them — that is
  // the property, not the number. The surfaces are in the loop because both
  // fills are opaque only until somebody mixes one with `transparent`.
  for (const [n, s] of [['--bg', bg], ['--card', card], ['--elevated', elevated]]) {
    text('var(--on-accent)', over(R('var(--accent-hover)'), s), `--on-accent on --accent-hover over ${n}`);
    text('var(--on-err)', over(R('var(--err-hover)'), s), `--on-err on --err-hover over ${n}`);
  }
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

  // Focus indicator and control boundaries. --elevated is the tightest of the
  // three and was missing: shadcn's TabsList and ToggleGroup are `bg-muted`
  // with 3px of padding and their triggers draw a 3px `ring-ring` into it, and
  // the Slider thumb rings against a `bg-muted` track. index.css's global
  // :focus-visible outline is var(--accent) too, so this is the token's floor
  // and not the alias's.
  nonText('var(--accent)', bg, 'focus ring on --bg');
  nonText('var(--accent)', card, 'focus ring on --card');
  nonText('var(--accent)', elevated, 'focus ring on --elevated');
  for (const [n, s] of [['--bg', bg], ['--card', card], ['--elevated', elevated]]) {
    nonText('var(--border-strong)', s, `--border-strong on ${n}`);
  }

  const measured = pairs.map((p) => ({ ...p, ratio: ratio(R(p.fg), p.on) }));

  return measured;
}

/* --------------------------------------------------------------- reporting */

const verbose = process.argv.includes('--verbose');
const failures = [];
let checked = 0;

for (const scheme of SCHEMES) {
  for (const theme of PRODUCT_THEMES) {
    for (const p of pairsFor(scheme, theme)) {
      checked += 1;
      const ok = p.ratio + 1e-9 >= p.floor;
      if (!ok) failures.push({ scheme, theme, ...p });
      if (verbose) {
        console.log(
          `${ok ? '  ' : 'FAIL'} ${scheme}/${theme.padEnd(8)} ` +
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
      `  ${f.scheme}/${f.theme} ${f.name} — ${f.ratio.toFixed(2)}:1, needs ${f.floor}:1`,
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
