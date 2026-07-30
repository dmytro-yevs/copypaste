# CopyPaste design tokens

`tokens/` is the only place a value is written. `dist/` is generated and
committed.

```sh
cd design && npm install
npm run rebuild        # clean + build + check
npm run check          # both gates
npm run check:contrast # token values; --verbose lists all 828 pairs
npm run check:usage    # the component tree; --verbose lists rules and exemptions
```

`npm run build` without `npm run check` is not enough — see *Contrast* and
*The component tree* below. The two gates cover different failures and neither
subsumes the other: `check:contrast` reads `dist/` and knows nothing about
which utility a component picked, `check:usage` reads
`crates/copypaste-ui/src/` and does not re-derive the palette.

---

## The decision

**shadcn/ui on Tailwind v4, Radix underneath, the `zinc` neutral base in
OKLCH.** Status hues, content-kind hues and the six accents are Tailwind v4
palette steps.

**What was compared.** [Radix Themes](https://www.radix-ui.com/themes) is the
same team's batteries-included system and would have been less work, but it
brings its own token layer and its own styling runtime, which would sit beside
Tailwind and beside `design/` — three places a colour could be defined. Base UI
and Ark UI are unstyled primitives, so choosing one leaves the visual system
still to be designed, which is the thing being avoided. Material Web / MUI
would make macOS look like Android; the product is both.

**Why shadcn specifically.** Radix implements the parts of manifest 06 §4 that
are pure DOM mechanics — dialog focus trap and restoration (A11Y-4),
ref-counted scroll lock (INV-19), tablist arrow-key wrap (A11Y-6),
`aria-expanded`/`aria-controls` (A11Y-7). v1 hand-wrote all of them and paid
for the misses (CopyPaste-wrfn, g27b.36a, 5917.30). Its components are source
files in our tree rather than a runtime dependency, and it themes entirely
through CSS custom properties, which is what lets `design/` stay the only
source.

**The cost.** shadcn source in our tree means upstream fixes arrive only when
someone re-runs the CLI, and the components are ours to maintain from the
moment they are copied.

### Where the adopted system is overridden

Four of shadcn's own defaults are not used, each because it measures below the
accessibility contract. These are the only deliberate divergences; anything
else that differs from upstream is a bug.

| shadcn default | Measured | What we do instead |
|---|---|---|
| white on dark `destructive` (red-400) | **2.75:1** | dark theme takes dark ink on `--err`; light keeps white on red-600 (4.71) |
| `ring-ring/50` focus halo | **1.51–2.98:1** | the ring is the accent at full alpha (3.12 worst) |
| `--input` = `--border` as a field's boundary | **1.25:1** | `--color-input` routes to `--border-strong` (3.64 worst) |
| an alpha on a filled button's hover (`/90`) | **4.02:1** primary, **4.45:1** destructive | `--accent-hover` / `--err-hover`, mixed away from the ink (5.77 worst) |

Three of the four are one `npx shadcn@latest add` away from coming back, and a
token gate cannot see any of them: `ring-ring/50` puts the alpha at the call
site and leaves every token value untouched. `npm run check:usage` is what
notices — see *The component tree*.

**The hover fill is mixed away from `--on-accent`, never toward the surface.**
An alpha dilutes a fill toward whatever is behind it, and on one theme or the
other that is toward the label: shadcn's `/90` puts `--on-accent` at 4.02
(dark/blue on `--bg`) and `--on-err` at 4.45 (light/indigo). Raising the alpha
until it passes is arithmetic, not a fix — `/97` and `/94` clear 4.5 by 0.04.
`--accent-hover` mixes `--accent` 12% toward `--accent-away`, the pole opposite
the ink, so the label's ratio can only rise on hover; 12% is about one Tailwind
palette step. The cost is stated rather than hidden: in dark, the two accents
with white ink darken on hover, so the fill falls from 3.12 to 2.35 against
`--elevated`. A filled button is identified by its rest state, and the hover
state is not its boundary.

---

## Contrast

`npm run check:contrast` composites every foreground against every surface it
can actually land on — including the alpha state layers and the `color-mix()`
selection fill, which have no ratio until they are composited — across both
themes and all six accents, and fails the build below **4.5:1 for text** and
**3:1 for control boundaries and focus indicators**. 828 pairs.

The last 108 measure the shadcn aliases (`--color-input`, `--color-ring`,
`--color-sidebar-ring`) rather than the tokens beneath them. A component says
`border-input` and never names `--border-strong`, so pointing the alias back at
the 1.25:1 `--border` would leave every other pair green.

It exists because the previous revision of this file asserted AA compliance
while eight combinations were below it. Do not replace a measurement here with
a claim.

Worst case in each category, over both themes and all six accents:

| | Worst | Floor |
|---|---|---|
| `--text` on any surface | 14.26 | 4.5 |
| `--dim` on any surface | 5.66 | 4.5 |
| `--faint` on any surface | 4.59 | 4.5 |
| `--accent-2` as text | 4.81 | 4.5 |
| `--on-accent` on an `--accent` fill | 4.55 | 4.5 |
| `--on-err` on an `--err` fill | 4.71 | 4.5 |
| `--on-accent` on `--accent-hover` | 5.77 | 4.5 |
| `--on-err` on `--err-hover` | 6.23 | 4.5 |
| `*-strong` on its own 15% tint | 4.90 | 4.5 |
| `--withheld-fg` on `--withheld` | 5.32 | 4.5 |
| content-kind glyph on `--bg` | 4.83 | 3.0 |
| status dot on `--bg` | 3.21 | 3.0 |
| focus ring on `--bg` / `--card` / `--elevated` | 3.12 | 3.0 |
| `--color-input` through the alias | 3.64 | 3.0 |
| `--border-strong` on any surface | 3.64 | 3.0 |
| `--selected-edge` on `--selected` | 3.75 | 3.0 |

**`--elevated` is the focus ring's tightest surface, and it was the pair that
was missing.** shadcn's `TabsList` and `ToggleGroup` are `bg-muted` with 3 px
of padding that a 3 px `ring-ring` fills exactly, and the `Slider` thumb rings
against a `bg-muted` track — so the ring's adjacent colour there is
`--elevated`, not `--bg`. Adding the pair put the dark theme's blue accent at
2.83:1. blue is now blue-500 with dark ink (4.01 as a ring, 4.78 as a fill)
rather than blue-600 with white (2.83 and 5.26). Every accent now clears the
floor on all three surfaces.

Two tokens are deliberately below AA and are checked by nothing: `--mute`
(4.12 dark, 2.63 light) and `--border` (1.25 dark, 1.27 light). Both are
decorative — `--mute` must never be the sole carrier of text (A11Y-10) and
`--border` must never be a control's only boundary. Using either for meaning
is the defect the `--border-strong` and `--faint` tokens exist to prevent.

**Selection is carried by an edge, not by the fill.** `--selected` differs from
`--bg` by only 1.09–1.32:1, and from `--hover` by less than that — a selected
row and a merely hovered row look alike, and neither is distinguishable at
1.4.11's 3:1. Raising the tint that far turns every selected row into a
coloured block. So the state indicator is `--selected-edge` (the accent at full
strength, `--sel-bar-w` wide), and `border-border` is not a substitute at
1.25:1.

**Offline, degraded and error share `--err`, deliberately.** Manifest 06 gives
all three `role="alert"` at error severity, so a fourth hue would encode a
distinction the product does not make. What separates them is copy and the
recovery action, not colour — and none of the three may carry a filesystem path
(INV-12).

**`--accent` is a fill, `--accent-2` is text.** This is the one distinction the
whole accent axis hangs on. `--accent` is only guaranteed against `--on-accent`
and against the 3:1 ring floor; as text it measures 3.12–9.22 depending on
accent, theme and surface, so it is not AA text on its worst one. So
`text-primary` and `text-brand` are wrong for accent-coloured text and
`text-brand-2` is right — `check:usage` fails on the first two.

The six fills are not one ramp step, because each has to clear `--on-accent` at
4.5 *and* the ring at 3:1 on `--bg`, `--card` and `--elevated`. Five sit at -500
in dark and rose at -600; teal, green, amber and blue take dark ink, because at
-500 those hues are too light to carry white and darkening them far enough
reads as muddy on a near-black ground.

---

## Two form factors, one token set

macOS is a resizable window plus a 380 px menu-bar popover; Android is a
full-screen activity. Three things follow.

**Touch targets vary by pointer, not by width.** `--tap-min`, `--hit-slop` and
`--sz-iconbtn` are re-emitted under `@media (pointer: coarse)`; nothing else
is. A narrow window on a Mac is still driven by a mouse, so a width query
would grow the wrong targets. Apple HIG asks 44 pt and Material 3 asks 48 dp
against WCAG 2.2's 24 px floor — 44 is the coarse `--tap-min` and 48 is the
coarse `--sz-iconbtn`, which is the control that actually fails on a phone.

Any interactive element takes `min-block-size`/`min-inline-size` of
`--tap-min`. A glyph too small to grow takes `--hit-slop` as a negatively
inset `::after`, which expands the target without moving the layout.

Rows need no coarse variant: a one-line row already computes to 63 px from
`--pad-row-y` and the two line heights. That is deliberate — `--pad-row-y` is
read by the virtualiser (INV-5), and a media query on it would change the
row-height model without the virtualiser knowing.

`check:usage` asserts the coarse block declares **exactly** those three, so
adding `--pad-row-y` to it fails with that reason rather than being read as an
omission and "fixed". It also fails any width query that redefines one of the
three.

**There is no breakpoint token.** The compact/expanded boundary is Tailwind's
own `sm` (640 px). Every phone in portrait is below it and every supported
macOS window is above it, and a second boundary beside Tailwind's is how two
scales start.

**Safe areas are tokens**, not raw `env()` at each call site: `env()` with no
fallback resolves to nothing rather than to `0px` and silently invalidates the
`calc()` around it.

### The interaction rules that follow

Three rules, because each one has a way of being got wrong that ships a control
Android users cannot reach.

**No affordance may exist only on hover.** Android has no hover, so a
hover-revealed control does not exist there. Row actions are always visible.

**Navigation moves rather than shrinks.** Below `sm` the rail becomes a bottom
bar (`--tabbar-h` + `--inset-bottom`) — the reachable band on a phone is the
bottom of the display, and a 200 px rail on a 380 px viewport leaves no content.
It is the same `<nav>`, reordered.

**One click never destroys.** In the history list a click selects and a
double-click copies; copying overwrites the system clipboard, and a list where
pointing at something overwrites it is a destructive default. The keyboard path
is Enter on the focused list, and every row carries an explicit Copy button —
which is what a screen reader and a finger both use.

---

## Withheld content

A sensitive item's content is **absent**, not obscured — the bridge sends
`content: null` (INV-10, CLAUDE.md rule 4, manifest 06). `--withheld`,
`--withheld-fg` and `--withheld-border` style a slot standing in for content
that was never delivered.

**There is no blur token and there must not be one.** v1's `--mask-blur: 6px`
is exactly the treatment the manifest README rules out: a blur says the content
is present behind a filter, which for a screenshot, a screen recording or a
shoulder is both a lie and a leak.

An absence nobody checks is an absence that returns, so `check:usage` checks
this one three ways: no token whose name contains `blur` other than
`--scrim-blur`, which backs a modal scrim and obscures nothing meant to be
read; no blur or backdrop-filter utility in the history components or on any
line mentioning withheld content; and no `opacity-` below 50 there either,
which is what the row hand-rolled before `--withheld` existed. Faint text is
still the plaintext — in the DOM, in a screenshot and in the accessibility
tree.

The pairing code is the one blur that stays, and it is recorded as an
exemption: it is a credential the user asks to see, and manifest 06 requires it
blurred by default and re-blurred before a regenerate lands (INV-13, AT-35).

---

## The component tree

`npm run check:usage` reads `crates/copypaste-ui/src/`. It exists because the
contrast gate reads token *values*, and a component can hold every one of them
fixed while shipping the defect: `ring-ring/50` puts the alpha back at the call
site, `text-primary` uses a fill token in a text role, `hover:bg-primary/90`
invents a colour that is in no token file. All three are shadcn defaults, and
re-running the CLI restores the first two.

What it fails on:

| | Instead |
|---|---|
| an alpha on a focus indicator (`ring-ring/50`) | `ring-ring` |
| `text-primary` · `text-brand` · `text-accent` | `text-brand-2` |
| `border-border` in a class string that also carries `focus-visible:`, `disabled:` or `data-[state=` | `border-border-strong` |
| a blur or `opacity-` under 50 on withheld content | the `--withheld` slot |
| a colour literal in a component | a token here |
| an alpha-modified colour utility that is not measured | add it to `lib/component-usage.mjs` |
| `bg-withheld`, `text-withheld-fg`, `border-withheld-border` or `bg-selected-edge` reaching no component | use it |
| `@media (pointer: coarse)` declaring anything but the three touch tokens | see *Two form factors* |
| an exemption naming a file that has moved | delete or repoint it |

The last two rows are the ones worth keeping. A check that only forbids things
lets a deliberate absence be deleted as an oversight, and an exemption whose
file has been renamed silences a rule for a path nothing matches.

Every alpha-modified utility found in the tree is composited across both themes
and all six accents and measured, because `bg-primary/90` has no ratio until it
lands on a surface. `lib/component-usage.mjs` holds the table: an entry either
carries a measurement or states why it has no floor.

---

## What you may and may not change

**May.** Add a token. Add a slot in `tokens/semantic/tailwind.json`. Change a
value and re-run `npm run rebuild` — if the gate passes, the change is fine.

**May not:**

- Put a colour in a component. If a component needs a colour that does not
  exist, it needs a token here.
- Put a literal in `tokens/semantic/tailwind.json`. Every entry is a pure alias
  and the generator throws otherwise; a literal there is the beginning of a
  second palette.
- Redefine Tailwind's own `spacing` or `text` scales. shadcn components are
  built against `p-2` and `text-sm`; ours are additive (`p-s-4`, `text-fs-sm`).
- Weaken a floor in `check-contrast.mjs` to make a value pass.
- Silence a rule in `check-usage.mjs` by narrowing its pattern. Record an
  exemption with its reason instead; a stale one is itself a failure.
- Reintroduce v1's palette. `docs/rewrite/port-manifest/README.md` explains why
  manifest 06 §8 is a map of which tokens exist, not of what they hold.

**Read the `$description` before changing a value.** Several carry a measured
ratio or a downstream dependency — `--pad-row-y` (INV-5), `--lh-normal` and
`--fs-sm`/`--fs-xs` (the row-height model), `alpha-tint` (invalidates the four
`*-strong` ratios).

---

## Output

`dist/css/` — custom properties per theme and accent, plus `theme.css`, one
`@theme inline` block. `index.css` imports them in cascade order, which is
load-bearing: accent blocks must follow the theme blocks they share specificity
with, and light accents (two attribute selectors) must follow dark (one).

```css
@import "tailwindcss";
@import "@copypaste/design-tokens/css";
```

`inline` is the load-bearing word: utilities emit `var(--our-token)` rather
than a copy, so flipping `data-theme` or `data-accent` re-tints every shadcn
component at runtime. The four axes on `<html>` are `data-theme`,
`data-theme-pref`, `data-accent`, `data-translucency`; every themed selector is
duplicated on `.theme-scope[…]` so a preview can render a different theme
without mutating `<html>`.

**The one trap in the mapping.** shadcn's `accent` is its neutral menu/list
hover surface, not the brand colour — so `--color-accent` maps to `--hover`,
and the brand is `--color-primary`. `--color-brand` exists as an unambiguous
alias for use outside shadcn components. `sidebar-accent` maps to `--selected`
rather than `--hover`, because shadcn uses that slot for the *active* nav item.

Reduced motion, reduced transparency and coarse pointer are emitted from token
`$extensions` rather than written in a stylesheet, so the rule travels with the
value it governs.

---

## Components

```sh
cd crates/copypaste-ui && npx shadcn@latest add <component>
```

> `ui.shadcn.com` is blocked by the egress policy here (the proxy answers 403
> to CONNECT), so the components in `src/components/ui/` were written to match
> the canonical `new-york` sources rather than pulled by the CLI. Anyone with
> network access should prefer the CLI; re-adding a component overwrites ours
> with upstream's, which is the intended direction.

Re-adding a component restores `ring-ring/50` and `border-input`. Run
`npm run check:usage` afterwards: it fails on the first and measures the
alias behind the second, which is the check that used to be "re-read the table
above and remember".

---

## Unverified

Every ratio here is computed from the generated CSS. **Nothing in this token
set has been seen rendered.** An earlier revision of this section blamed the
engine — it said WebKitGTK on this host executes no JavaScript under headless
Xvfb. That is not true and `e2e/README.md` says so: WebKitGTK 2.52 runs
JavaScript and computes layout under plain Xvfb, and the e2e suite drives the
real wry WebView through it. The accurate statement is narrower and worse: the
pixels have never been looked at, by anyone, on any engine. Nothing here is
blocked on tooling.

What a machine cannot settle:

- Whether `--border-strong` at 40% white reads as heavy-handed on dark cards.
  It is the value the 3:1 floor requires; if it looks wrong, the fix is a
  different surface relationship, not a weaker border.
- Whether dark ink on a coloured fill reads as intended or as a rendering
  fault. This now covers the dark theme's destructive button and four of the
  six accents — teal, green, amber and, since the ring was measured on
  `--elevated`, blue.
- Whether the frosted chrome values land the same on WKWebView and on Android
  WebView. Only the solid fallback is guaranteed.
- Coarse-pointer sizing on a real phone. `pointer: coarse` is inferred from the
  media query, not observed.
- Whether a filled button that *darkens* on hover reads as a hover or as the
  button receding. In dark, indigo and rose take white ink, so away from the
  ink is toward black; the shift is 12%, about one palette step, and the
  direction is not negotiable — the alternative is a hover that lowers the
  label's contrast.
