# CopyPaste design tokens

One token source, compiled to the app's theme. `tokens/` is the only place a
value is written; `dist/` is generated and committed.

```
design/
  tokens/                     ← the only source of truth. Edit these.
  style-dictionary.config.js  ← build configuration + the Tailwind v4 format
  dist/                       ← GENERATED. Committed, never hand-edited.
```

```sh
cd design && npm install && npm run build     # or: npm run rebuild
```

---

## The design decision

**CopyPaste v2 adopts [shadcn/ui](https://ui.shadcn.com) on Tailwind v4, with
Radix primitives underneath it. The palette is shadcn's default theme on the
`zinc` base, in OKLCH; every other hue — the six accents, the four status
colours, the eleven content-kind colours — is a step from the
[Tailwind v4 palette](https://tailwindcss.com/docs/colors).**

There are three things to justify: why a system at all, why this one, and why
not the one we already had.

### Why adopt a system rather than assemble one

v1 assembled one. Its receipt is in the port manifest: a 979-line
`design-reference.html`, a hand-written stylesheet held in lockstep with it by a
parity test, fifteen font sizes including three half-pixel steps, eleven
per-component corner radii, and a spacing ladder (2/4/6/8/11/14/16/20/24) whose
own comment insisted the odd steps were deliberate. None of that was designed in
one sitting. It is what a design system looks like after two years of local
decisions, each of which was reasonable on its own — which is the exact shape of
the failure CLAUDE.md rule 1 was written about, in CSS instead of in Rust.

A maintained system is the dependency that replaces all of it. shadcn's
components arrive with their own spacing, sizing and state styling already
decided and already consistent, so the question "what padding does a button
have" stops being asked once per button.

### Why shadcn/ui

1. **The accessibility behaviour comes with it.** Radix UI implements the parts
   of manifest 06 §4 that are pure DOM mechanics: the dialog focus trap and
   focus restoration (A11Y-4), reference-counted body scroll-lock (INV-19),
   tablist arrow-key navigation with wrap-around (A11Y-6), `aria-expanded` /
   `aria-controls` wiring on disclosures (A11Y-7). v1 hand-wrote every one of
   those and paid for the misses (CopyPaste-wrfn, g27b.36a, 5917.30). This is
   rule 1 applied to accessibility: the behaviour is a solved problem and we
   should not be solving it.
2. **It is not a runtime dependency.** shadcn components are source files copied
   into `src/components/ui/`, so there is no library to fight when a component
   needs to do something specific — and no version to be stuck behind.
3. **It is themed entirely through CSS custom properties**, which is what lets
   `design/` stay the single source. `dist/css/theme.css` is one
   `@theme inline` block mapping our tokens onto shadcn's semantic slots; every
   stock component re-tints when `[data-theme]` or `[data-accent]` changes, with
   no per-component override anywhere.
4. **It is already half-present.** Radix Dialog, Popover, Tooltip, Slot, `cva`,
   `clsx` and `tailwind-merge` were already dependencies. The alternative was to
   keep those and hand-write the components on top, which is assembling a system
   out of a system's parts.

**What was weighed against it.** [Radix Themes](https://www.radix-ui.com/themes)
is the same team's batteries-included system and would have been less work, but
it brings its own token layer and its own styling runtime, which would sit
beside Tailwind and beside `design/` — three sources of truth for a colour. Base
UI and Ark UI are primitive libraries, so they would leave the visual system
still to be designed. Material Web / MUI would make macOS look like Android.

**The cost, stated rather than waved at.** shadcn source lives in our tree, so
upstream fixes arrive only when someone re-runs the CLI for a component; the
components are ours to maintain the moment they are copied. In exchange the
components are readable and editable, which for a UI with unusual requirements
(rows that must not be `role="option"`, content that must be absent rather than
hidden) is worth more than automatic updates.

### Why not v1's palette

Because the user rejected it, and because `design/dist/` holding it
value-for-value made re-deriving it the path of least resistance — which is
precisely what `docs/rewrite/port-manifest/README.md` warns against. **No value
in `tokens/` is a v1 value.** The manifest's §8 token dump is now reference
material for *what tokens exist*, not for what they hold. Token **names** are
unchanged (`--bg`, `--r-card`, `--c-url`), so §8 still reads as a map of the
system; only the values behind them are new.

The visible difference: v1 was a blue-tinted near-black (`#0E0F14`) with Inter,
half-pixel type steps and eleven radii. v2 is Tailwind's neutral `zinc` ramp,
the platform UI face, seven type steps and one four-step radius ramp.

### Two ways it stays coherent across macOS and Android

- **No webfont.** `--f-ui` is Tailwind's default `font-sans` stack, so the UI
  face is San Francisco on macOS and Roboto on Android. One build reads as
  native on both, and there is no font file to ship or fail to ship. (v1 named
  Inter first and never bundled it, so it silently fell through to the system
  face anyway.)
- **OKLCH.** Both WebKit and Chromium interpolate it, so `color-mix()` for the
  accent-tinted selection fill lands in the same perceptual place on both
  platforms rather than muddying through sRGB.

---

## What the tokens hold

**191 tokens per theme**, across 17 files.

| File | What it holds |
|---|---|
| `tokens/color/theme.dark.json` · `theme.light.json` | Surfaces, text, state layers, status colours, the AA-corrected `*-strong` foreground variants, the eleven content-kind hues |
| `tokens/color/accents.dark.json` · `accents.light.json` | The six-accent axis |
| `tokens/color/static.json` | The inks that sit on a coloured fill, and the container tint alpha |
| `tokens/elevation.*.json` | The `--sh1/2/3` shadow ramp and the focus halo, as DTCG shadow objects |
| `tokens/typography.json` | Faces, the seven-step size ladder, weights, line-heights, tracking |
| `tokens/spacing.json` · `radius.json` · `size.json` · `layout.json` · `z-index.json` | Tailwind's 4px rhythm, shadcn's radius ramp, control/icon sizes, window geometry, stacking order |
| `tokens/motion.json` | Durations and the single easing curve |
| `tokens/translucency.json` | The solid/frosted axis |
| `tokens/semantic/tailwind.json` | **Mapping table** → Tailwind v4 / shadcn/ui |

`tokens/semantic/tailwind.json` holds no values at all. Every token in it is a
pure alias onto a core token and the generator throws if one is not — a literal
there would be the beginning of a second palette.

### Rules encoded in the tokens rather than in a stylesheet

- **Reduced motion** (A11Y-11) — the three duration tokens carry a
  `reducedMotion` extension; the CSS format emits the
  `@media (prefers-reduced-motion)` override from it.
- **Reduced transparency** (A11Y-12) — the frosted values live in their own
  group and the format emits the `@supports` block, the
  `[data-translucency="on"]` scoping and the `prefers-reduced-transparency`
  reset around them. Solid is the baseline, frosting is additive, so an engine
  without `backdrop-filter` gets the solid fallback with no separate rule.
- **`--pad-row`** is composed from `--pad-row-y` and `--pad-row-x` rather than
  written as a shorthand, because `ROW_PAD_V` in the virtualiser is derived from
  the vertical half and the two must not drift (INV-5).
- **AA contrast** (A11Y-10) — `--faint` is deliberately off-ramp in both themes
  so it clears 4.5:1 while still reading as tertiary; `--mute` is decorative and
  documented as never being the sole carrier of text; the four `*-strong`
  variants exist because a status hue that is correct as a fill fails as small
  text on its own tint.

---

## The output

One target: **CSS custom properties + a Tailwind v4 theme layer**, in
`dist/css/`.

| File | Selector |
|---|---|
| `tokens.base.css` | `:root` — everything that does not vary by theme, plus the reduced-motion and translucency blocks |
| `tokens.dark.css` | `:root, :root[data-theme="dark"], .theme-scope[data-theme="dark"]` |
| `tokens.light.css` | `:root[data-theme="light"], .theme-scope[data-theme="light"]` |
| `accents.dark.css` · `accents.light.css` | `[data-accent="…"]`, six blocks each |
| `theme.css` | one `@theme inline { … }` block: the shadcn slot contract, plus a mechanical bridge exposing every core token as a Tailwind key |
| `index.css` | imports the above in cascade order |

```css
@import "tailwindcss";
@import "@copypaste/design-tokens/css";   /* dist/css/index.css */
```

Import order is load-bearing and `index.css` gets it right: the accent blocks
must come after the theme blocks they share specificity with, and the light
accent blocks (two attribute selectors) after the dark ones (one).

`inline` is the important word in `@theme inline`: utilities emit
`var(--our-token)` rather than a copy of the value, which is why flipping
`data-theme` or `data-accent` re-tints every shadcn component at runtime.

Two deliberate non-overrides: Tailwind's own **spacing** and **text** scales are
left alone, because shadcn components are built against them (`p-2`, `text-sm`)
and redefining those would silently reshape every stock component. Ours are
additive — `p-s-4`, `text-fs-sm`.

The four axes on `<html>` are `data-theme`, `data-theme-pref`, `data-accent`
and `data-translucency`. Every themed selector is duplicated on `.theme-scope[…]`
so a preview can render a different theme in a scoped wrapper without mutating
`<html>`.

### The mapping table

One CopyPaste token, feeding one shadcn slot.

| CopyPaste token | shadcn/ui slot (`--color-…`) |
|---|---|
| `--bg` | `background` |
| `--panel` | `sidebar` |
| `--card` | `card`, `popover` |
| `--elevated` | `muted` |
| `--raised` | `secondary` |
| `--border` | `border`, `input` |
| `--divider` | `sidebar-border` |
| `--text` | `foreground`, `card-foreground`, `popover-foreground`, `secondary-foreground`, `accent-foreground`, `sidebar-accent-foreground` |
| `--dim` | `muted-foreground`, `sidebar-foreground` |
| `--accent` | `primary`, `ring`, `sidebar-primary`, `sidebar-ring`, `brand` |
| `--accent-2` | `brand-2` |
| `--on-accent` | `primary-foreground`, `sidebar-primary-foreground`, `on-brand` |
| `--hover` | `accent` |
| `--pressed` | `pressed` |
| `--selected` | `sidebar-accent`, `selected` |
| `--err` | `destructive` |
| `--on-err` | `destructive-foreground` |
| `--c-url` · `--c-code` · `--c-color` · `--c-image` · `--c-num` | `chart-1…5` |
| `--f-ui` · `--f-mono` | `font-sans`, `font-mono` |
| `--r-sm` · `--r-ctl` · `--r-row` · `--r-card` | `--radius-sm` / `--radius-md` / `--radius`,`--radius-lg` / `--radius-xl` |
| `--ease` | `--ease-cp` |

**The one trap:** shadcn's `accent` is not our accent. In shadcn, `accent` is
the neutral hover/active surface for menu and list items; the brand colour is
`primary`. So `--color-accent` maps to `--hover`. Because that is a trap,
`--color-brand` is exposed as an unambiguous alias — `bg-brand` and `bg-primary`
are the same colour, and `brand` is the one to use outside shadcn components.

`sidebar-accent` maps to `--selected`, not to `--hover`: shadcn uses that slot
for the *active* nav item, which is accent-tinted, unlike the plain hover.

---

## Two things that changed with ADR-0002

1. **The Compose/Material 3 output is gone**, along with
   `tokens/semantic/material.json` and about 200 lines of generator. It existed
   to theme a Jetpack Compose app that [ADR-0002](../docs/adr/0002-one-cross-platform-app.md)
   deleted; Android is now the same Tauri + React app and reads the same CSS.
   Keeping a generator for an app that does not exist is the kind of frozen
   abstraction manifest §9.2 says to delete. It is recoverable from git history
   if the native decision is ever taken again — but the colours would need
   re-expressing, because Style Dictionary's Compose colour transform passes
   OKLCH through untouched and would emit invalid Kotlin.
2. **There is no `copypaste-design-reference.html` to be parity-locked to.**
   v1's arrangement — a reference document, a stylesheet, and a test asserting
   they agree — is two sources of truth with a test standing between them. There
   is one source now. What is worth testing lives in the app: that the rendered
   UI meets the behaviour and accessibility requirements in manifest 06 §4, not
   that the generator agrees with a document.

---

## Working on the components

`crates/copypaste-ui/components.json` configures the shadcn CLI, so a component
is added the normal way:

```sh
cd crates/copypaste-ui && npx shadcn@latest add <component>
```

> **Note for this environment:** `ui.shadcn.com` is blocked by the egress
> policy here (the proxy answers 403 to CONNECT), so the CLI cannot fetch the
> registry. The components currently in `src/components/ui/` were written to
> match the canonical `new-york` sources rather than pulled by the CLI. Anyone
> with network access should prefer the CLI, and re-adding a component will
> overwrite ours with upstream's — which is the intended direction.

Do not add a colour to a component. If a component needs a colour that does not
exist, it needs a token here and a slot in `tokens/semantic/tailwind.json`.
