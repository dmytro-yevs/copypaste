/**
 * What the component tree may contain.
 *
 * The contrast gate reads token *values*. It cannot see a component that stops
 * using a corrected token — `ring-ring/50` puts the alpha back at the call
 * site, `text-primary` uses a fill token in a text role, and both leave every
 * token in dist/ unchanged. These rules are the half of the contract that
 * lives in the consuming source, and `npx shadcn@latest add` is one command
 * away from reintroducing the first two.
 */

/** Relative to design/. */
export const SRC = '../crates/copypaste-ui/src/';

/** `.test.` files are excluded: a test that asserts a class is absent has to
 *  name it, and flagging that is how a check teaches people to disable it. */
export const SCANNED = /\.(?:tsx?|css)$/;
export const NOT_SCANNED = /(?:^|\/)(?:__snapshots__|node_modules)\/|\.test\.|\.d\.ts$/;

const historyOrWithheld = (file, line) =>
  file.startsWith('components/history/') || /withheld|sensitive|masked/i.test(line);

/**
 * `test` is matched per line. `applies(file, line)` narrows a rule to the
 * surface it is about; without it the rule is tree-wide.
 */
export const CLASS_RULES = [
  {
    id: 'focus-ring-alpha',
    test: /(?<![\w-])(?:ring|outline|border|shadow)-(?:ring|sidebar-ring|primary|brand|accent|destructive)\/\d+(?![\w-])/g,
    fix: 'drop the /N — the focus indicator is the accent at full alpha (`ring-ring`)',
    why:
      "shadcn's stock `ring-ring/50` measures 1.51-2.98 over --bg, --card and --elevated across " +
      'the six accents; full alpha measures 3.12 against a 3:1 floor (WCAG 1.4.11). --color-ring ' +
      'is already correct, so nothing in dist/ changes and the contrast gate stays green.',
  },
  {
    id: 'accent-as-text',
    test: /(?<![\w-])text-(?:primary|brand|sidebar-primary|accent)(?![\w-])/g,
    fix: '`text-brand-2` — the text-safe accent step',
    why:
      '--accent is a fill, guaranteed only against --on-accent and against the 3:1 ring floor. ' +
      'As text it measures 3.12-9.22 across accent, theme and surface, so on its worst surface ' +
      'it is not AA. `text-accent` is worse still: shadcn spends the word `accent` on its ' +
      'neutral hover surface, so that utility paints --hover, not the brand.',
  },
  {
    id: 'decorative-border-on-a-control',
    test: /(?<![\w-])border-border(?![\w-])/g,
    applies: (_file, line) =>
      /focus-visible:|focus-within:|aria-invalid:|disabled:|data-\[state=/.test(line),
    fix: '`border-border-strong`, or `border-input` which routes to it',
    why:
      '--border is 1.25:1 and decorative. A control whose only boundary is --border has no ' +
      'boundary (WCAG 1.4.11). This flags only class strings that also carry an interactive ' +
      'variant, so a card edge or a separator is left alone.',
  },
  {
    id: 'blur-over-clipboard-content',
    test: /(?<![\w-])(?:backdrop-)?blur-(?:xs|sm|md|lg|xl|2xl|3xl|none|\[[^\]]+\])(?![\w-])|(?:backdrop-)?filter:\s*[^;"'`]*\bblur\(/g,
    applies: historyOrWithheld,
    fix: 'render the --withheld slot: bg-withheld / text-withheld-fg / border-withheld-border',
    why:
      'INV-10: a sensitive item\'s plaintext is absent, not obscured — the bridge sends ' +
      'content: null. A blur asserts the content is present behind a filter, which for a ' +
      'screenshot, a screen recording or a shoulder is both a lie and a leak. v1 carried ' +
      '--mask-blur: 6px; there is no blur token here and there must not be one.',
  },
  {
    id: 'hand-rolled-mask',
    test: /(?<![\w-])opacity-(?:[0-4]?[0-9])(?![\w-])/g,
    applies: historyOrWithheld,
    fix: 'render the --withheld slot rather than painting the content faintly',
    why:
      'The row hand-rolled `opacity-25` over the preview before --withheld existed. Faint ' +
      'text is still the plaintext: it is in the DOM, in a screenshot and in the ' +
      'accessibility tree. opacity >= 50 is left alone, so `disabled:opacity-50` is fine.',
  },
  {
    id: 'literal-colour',
    test: /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})(?![0-9a-zA-Z_])|\b(?:rgba?|hsla?|oklch|oklab|color-mix)\(/g,
    fix: 'add a token in design/tokens/ and use the utility it generates',
    why:
      'A colour in a component is the beginning of a second palette, and it cannot follow ' +
      '[data-theme] or [data-accent].',
  },
];

/** Tokens that must be reachable from a component, or the treatment they
 *  replaced has probably come back by another route. */
export const REQUIRED_UTILITIES = [
  { util: 'bg-withheld', why: 'the slot standing in for absent content (INV-10)' },
  { util: 'text-withheld-fg', why: 'its label' },
  { util: 'border-withheld-border', why: 'the reveal affordance is a real button (A11Y-3)' },
  { util: 'bg-selected-edge', why: 'selection is carried by the edge, never by the fill' },
];

/**
 * Every alpha-modified colour utility the tree is allowed to contain.
 *
 * A `/N` on a fill is a colour that exists in no token file, so it has a ratio
 * only once composited. `measure` entries are composited across both themes
 * and all six accents; the rest record why a combination carries no floor.
 * Anything found in the source and absent here fails, which is what stops the
 * list going stale.
 */
export const ALPHA_UTILITIES = [
  {
    util: 'bg-secondary/80',
    where: 'Button, secondary variant, hover',
    measure: { fill: 'var(--raised)', alpha: 0.8, fg: 'var(--text)', on: ['--bg', '--card'] },
  },
  {
    util: 'bg-card/60',
    where: 'PairCreateDialog, the reveal overlay over the hidden pairing code',
    measure: { fill: 'var(--card)', alpha: 0.6, fg: 'var(--text)', on: ['--elevated', '--card'] },
  },
  {
    util: 'bg-ok/15',
    where: 'Badge ok',
    covered: 'the --ok-strong on a 15% --ok tint pairs',
  },
  { util: 'bg-warn/15', where: 'Badge warn, Banners, HistoryView', covered: 'the --warn-strong tint pairs' },
  { util: 'bg-err/15', where: 'Badge error, Banners, PairAcceptDialog, QuickPasteApp', covered: 'the --err-strong tint pairs' },
  { util: 'bg-info/15', where: 'Badge info', covered: 'the --info-strong tint pairs' },
  {
    util: 'border-warn/20',
    where: 'Banners, HistoryView',
    decorative:
      'A banner is a region, not a control, and its text carries the meaning (A11Y-5 gives it ' +
      'role="alert"). WCAG 1.4.11 does not apply to a container edge.',
  },
  {
    util: 'border-err/20',
    where: 'Banners',
    decorative: 'As border-warn/20.',
  },
];

/**
 * Deliberate absences and deliberate exceptions, each stated so that removing
 * one is a decision rather than an oversight.
 */
export const EXEMPTIONS = [
  {
    rule: 'no-blur-token',
    token: 'scrim-blur',
    why:
      'A modal scrim over the whole app, not a mask over a preview. It obscures nothing that is ' +
      'meant to be read, and prefers-reduced-transparency turns it off.',
  },
  {
    rule: 'coarse-pointer-set',
    token: '--pad-row-y',
    why:
      'Rows deliberately have no coarse variant. A one-line row already computes to 63px, and ' +
      '--pad-row-y is read by the virtualiser (INV-5) — a media query on it would change the ' +
      'row-height model without the virtualiser knowing, which is CopyPaste-g27b.30 again.',
  },
];

/** Exactly these, under `@media (pointer: coarse)`, and nothing else. */
export const COARSE_TOKENS = ['tap-min', 'hit-slop', 'sz-iconbtn'];

export const exemptions = (rule) => EXEMPTIONS.filter((e) => e.rule === rule);

export const isExempt = (rule, file) =>
  exemptions(rule).some((e) => e.files?.includes(file));
