# CopyPaste design tokens

`tokens/` is the only place a value is written. `dist/` is generated and
committed.

```sh
cd design && npm install
npm run rebuild        # clean + build + check
npm run check          # contrast gate alone; --verbose lists all 636 pairs
```

`npm run build` without `npm run check` is not enough — see *Contrast* below.

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

Three of shadcn's own defaults are not used, each because it measures below the
accessibility contract. These are the only deliberate divergences; anything
else that differs from upstream is a bug.

| shadcn default | Measured | What we do instead |
|---|---|---|
| white on dark `destructive` (red-400) | **2.75:1** | dark theme takes dark ink on `--err`; light keeps white on red-600 (4.71) |
| `ring-ring/50` focus halo | **1.78–2.98:1** | the ring is the accent at full alpha (3.37 worst) |
| `--input` = `--border` as a field's boundary | **1.25:1** | `--color-input` routes to `--border-strong` (3.64 worst) |

---

## Contrast

`npm run check` composites every foreground against every surface it can
actually land on — including the alpha state layers and the `color-mix()`
selection fill, which have no ratio until they are composited — across both
themes and all six accents, and fails the build below **4.5:1 for text** and
**3:1 for control boundaries and focus indicators**. 636 pairs.

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
| `*-strong` on its own 15% tint | 4.90 | 4.5 |
| `--withheld-fg` on `--withheld` | 5.32 | 4.5 |
| content-kind glyph on `--bg` | 4.83 | 3.0 |
| status dot on `--bg` | 3.21 | 3.0 |
| focus ring on `--bg` / `--card` | 3.37 | 3.0 |
| `--border-strong` on any surface | 3.64 | 3.0 |
| `--selected-edge` on `--selected` | 3.33 | 3.0 |

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
whole accent axis hangs on. `--accent` is only guaranteed against
`--on-accent`; as text it measures 3.21–4.37 depending on accent and theme. So
`text-primary` and `text-brand` are wrong for accent-coloured text and
`text-brand-2` is right. The consequence is that the six fills are not one ramp
step — indigo, teal, green and amber sit at -500 in dark, blue and rose at
-600, and teal/green/amber take dark ink because darkening those hues far
enough to carry white reads as muddy on a near-black ground.

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
> with upstream's, which is the intended direction — but it will also restore
> `ring-ring/50` and `border-input`, so re-check the three overrides above.

---

## Unverified

Every ratio here is computed from the generated CSS. **Nothing in this token
set has been seen rendered.** WebKitGTK on this host executes no JavaScript
under headless Xvfb, so the app has never painted. What needs a human eye on a
real device:

- Whether `--border-strong` at 40% white reads as heavy-handed on dark cards.
  It is the value the 3:1 floor requires; if it looks wrong, the fix is a
  different surface relationship, not a weaker border.
- Whether dark ink on the dark theme's destructive button reads as intended or
  as a rendering fault.
- Whether the frosted chrome values land the same on WKWebView and on Android
  WebView. Only the solid fallback is guaranteed.
- Coarse-pointer sizing on a real phone. `pointer: coarse` is inferred from the
  media query, not observed.
