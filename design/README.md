# CopyPaste design tokens

The macOS desktop app and the Android app have to look like one product. This
directory is how that happens: **one token source, compiled to per-platform
themes.** There is no second palette to keep in step, and therefore nothing to
police with a parity test.

```
design/
  tokens/                   ← the only source of truth. Edit these.
  style-dictionary.config.js  ← build configuration + the platform formats
  dist/                     ← GENERATED. Committed, never hand-edited.
```

## What v1 did, and what changed

v1 kept a bespoke CSS system in lockstep with a 979-line
`copypaste-design-reference.html` by way of a hand-written parity test
(`src/styles/tokens.parity.test.ts`). Two sources of truth with a test standing
between them is exactly the tax this rewrite removes. Here the reference is
still the *visual* intent — `docs/rewrite/design-reference.html` — but it is no
longer a second definition of the values. The values live in `tokens/`, and
every platform theme is generated from them.

The token *names* are deliberately unchanged (`--bg`, `--r-card`, `--fs-md`,
`--c-url`), so port-manifest §8 and the design reference stay readable as
documentation without a translation step.

## Regenerating

```sh
cd design
npm install
npm run build      # or: npm run rebuild, which cleans dist/ first
```

`dist/` is committed. **Consumers do not need Node** — the desktop app imports
the CSS and the Android app compiles the Kotlin straight out of the repo.
Regenerate only when you change something in `tokens/`, and commit the
regenerated output in the same commit as the token change.

Everything under `dist/` carries a `GENERATED … DO NOT EDIT` banner. If you find
yourself editing one of those files, the change belongs in `tokens/` instead.

### Why Style Dictionary, and why v5

Per the repository rule that dependencies are the default: compiling one token
source to several platform themes is exactly what
[Style Dictionary](https://styledictionary.com) does, and hand-rolling a token
compiler is what this repo has already paid for once. The tokens are authored in
[W3C DTCG](https://tr.designtokens.org/format/) format, which Style Dictionary
consumes natively.

We are on **v5**, the current maintained line, rather than the v4 that DTCG
support first shipped in. v5 reads the same `$value`/`$type` format; there is no
reason to pin to an older major.

`style-dictionary.config.js` contains configuration plus four custom *formats*
— Style Dictionary ships no Material 3 `ColorScheme` writer and no Tailwind v4
`@theme` writer, and formats are the library's documented extension point for
exactly that. It does no parsing, reference resolution or transformation of its
own.

## The token files

**328 token definitions** across 18 files. Each theme resolves to 267 tokens
(the shared scales plus that theme's colours).

| File | What it holds |
|---|---|
| `tokens/color/theme.dark.json` · `theme.light.json` | Surfaces, text, state layers, status colours, the AA-corrected `*-strong` foreground variants, and the eleven content-kind hues. Manifest §8.1–§8.3, §8.7. |
| `tokens/color/accents.dark.json` · `accents.light.json` | The six-accent axis. §8.4. |
| `tokens/color/static.json` | Colours that do not vary by theme, plus the two inks the semantic layers need that v1 never had. |
| `tokens/elevation.*.json` | The `--sh1/2/3` shadow ramp, as DTCG shadow objects. |
| `tokens/typography.json` | Faces, the 15-step font-size ladder, weights, line-heights, tracking. |
| `tokens/spacing.json` | The `s-1…s-9` scale, gaps, component padding. |
| `tokens/radius.json` · `size.json` · `layout.json` · `z-index.json` | Corner radii, control/icon/stroke sizes, window geometry, stacking order. |
| `tokens/motion.json` | Durations and the single easing curve. |
| `tokens/translucency.json` | The solid/frosted axis. §8.6. |
| `tokens/semantic/tailwind.json` | **Mapping table** → Tailwind v4 / shadcn/ui. |
| `tokens/semantic/material.json` | **Mapping table** → Material 3 `ColorScheme`. |

The two files under `tokens/semantic/` hold no values at all. Every token in
them is a pure alias onto a core token, and the generators throw if one is not —
a literal there would be the beginning of a second palette.

### Rules that are encoded in the tokens, not in a stylesheet

- **Reduced motion** — the three duration tokens carry a `reducedMotion`
  extension; the CSS format emits the `@media (prefers-reduced-motion)` override
  from it (§8.5).
- **Translucency** — the frosted values live in their own token group and the
  format emits the `@supports` block, the `[data-translucency="on"]` scoping and
  the `prefers-reduced-transparency` reset around them (§8.6).
- **`--pad-row`** is composed from `--pad-row-y` and `--pad-row-x` rather than
  written as a shorthand, because `ROW_PAD_V = 18` in the virtualizer is derived
  from the vertical half and the two must not drift (§8.5, §5.2).
- **The selection fill** is `--alpha-selected` (0.16 dark / 0.12 light) in one
  place. CSS renders it as `color-mix(… var(--accent) …)`; Compose renders it as
  `accent.copy(alpha = …)`. Neither platform re-derives the number.

## The three outputs

### 1. CSS custom properties — `dist/css/`

For the Tauri desktop app.

| File | Selector |
|---|---|
| `tokens.base.css` | `:root` — everything that does not vary by theme, plus the reduced-motion and translucency blocks |
| `tokens.dark.css` | `:root, :root[data-theme="dark"], .theme-scope[data-theme="dark"]` |
| `tokens.light.css` | `:root[data-theme="light"], .theme-scope[data-theme="light"]` |
| `accents.dark.css` · `accents.light.css` | `[data-accent="…"]`, six blocks each |
| `index.css` | imports the above in cascade order |

The four axes v1 put on `<html>` are unchanged: `data-theme`,
`data-theme-pref`, `data-accent`, `data-translucency`. Every themed selector is
duplicated on `.theme-scope[…]` so a dev gallery can preview a theme in a
scoped wrapper without mutating `<html>`.

Import order is load-bearing and `index.css` gets it right: the accent blocks
must come after the theme blocks they share specificity with, and the light
accent blocks (two attribute selectors) after the dark ones (one).

### 2. Tailwind v4 theme — `dist/css/theme.css`

A single `@theme inline { … }` block. `inline` is the important word: utilities
emit `var(--our-token)` rather than a copy of the value, so flipping
`data-theme` or `data-accent` re-tints every shadcn component with no
per-component override.

```css
@import "tailwindcss";
@import "@copypaste/design-tokens/css";   /* dist/css/index.css */
```

It has two halves. The explicit half is the shadcn/ui semantic contract, from
`tokens/semantic/tailwind.json` — the table below. The mechanical half exposes
every remaining core token as a Tailwind theme key, so `bg-panel`,
`text-c-url`, `rounded-card`, `shadow-2` and `p-s-4` exist without anybody
maintaining a list.

Two deliberate non-overrides: Tailwind's own **spacing** and **text** scales are
left alone, because shadcn components are built against them (`p-2`, `text-sm`)
and halving those would silently reshape every stock component. Our scales are
additive — `p-s-4`, `text-fs-md`.

### 3. Kotlin / Compose — `dist/compose/`

For the Android app.

- **`Color.kt`** — `CopyPasteColors` (one data class, two instances:
  `copyPasteDarkColors`, `copyPasteLightColors`), plus the `CopyPasteAccent`
  enum and the per-theme accent palettes.
- **`ColorScheme.kt`** — `copyPasteDarkColorScheme(accent)` and
  `copyPasteLightColorScheme(accent)` returning Material 3 `ColorScheme`s, plus
  `LocalCopyPasteColors` for the tokens Material has no role for (the
  content-kind hues, the `*Strong` variants, the state layers).
- **`Dimens.kt`** — spacing, radii, sizes, layout, font sizes/weights/tracking,
  durations, stacking order, and the easing curve.

```kotlin
MaterialTheme(colorScheme = if (dark) copyPasteDarkColorScheme(accent) else copyPasteLightColorScheme(accent)) { … }
```

Not crossing to Compose, on purpose: the `--sh1/2/3` box-shadows (Material
expresses depth tonally, and the `surfaceContainer*` ramp below carries that),
the CSS padding shorthands, the translucency axis, and the font *families* —
those need font resources the design layer does not own.

## The mapping table

This is what actually makes the two platforms look alike. Read it as: one
CopyPaste token, feeding a shadcn slot on the desktop and a Material 3 role on
Android.

`*` marks a role rendered as the token at an alpha, not the token itself.

| CopyPaste token | shadcn/ui slot (`--color-…`) | Material 3 `ColorScheme` role |
|---|---|---|
| `--bg` | `background` | `background`, `surfaceDim`, `surfaceContainerLowest`, `inverseOnSurface` |
| `--panel` | `sidebar` | `surface`, `surfaceContainerLow` |
| `--elevated` | `popover`, `muted` | `surfaceContainer` |
| `--card` | `card` | — |
| `--raised` | `secondary` | `secondary`, `surfaceVariant`, `surfaceContainerHigh` |
| `--raised-2` | — | `tertiaryContainer`, `surfaceBright`, `surfaceContainerHighest` |
| `--border` | `border`, `input` | `outline` |
| `--divider` | `sidebar-border` | `outlineVariant` |
| `--text` | `foreground`, `card-foreground`, `popover-foreground`, `secondary-foreground`, `accent-foreground`, `sidebar-accent-foreground` | `onBackground`, `onSurface`, `onSecondary`, `onSecondaryContainer`, `onTertiaryContainer`, `inverseSurface` |
| `--dim` | `muted-foreground`, `sidebar-foreground` | `onSurfaceVariant` |
| `--accent` | `primary`, `ring`, `sidebar-primary`, `sidebar-ring`, `brand` | `primary`, `surfaceTint`, `secondaryContainer`\* |
| `--accent-2` | `brand-2` | `primaryContainer`, `tertiary`, `inversePrimary` |
| `--on-accent` | `primary-foreground`, `sidebar-primary-foreground`, `on-brand` | `onPrimary` |
| `--on-accent-2` | — | `onPrimaryContainer`, `onTertiary` |
| `--hover` | `accent` | — (Material's own state layer) |
| `--pressed` | `pressed` | — (Material's own state layer) |
| `--selected` | `sidebar-accent`, `selected` | via `secondaryContainer`\* |
| `--err` | `destructive` | `error`, `errorContainer`\* |
| `--on-err` | `destructive-foreground` | `onError` |
| `--err-strong` | — | `onErrorContainer` |
| `--scrim` | — | `scrim` |
| `--c-url` · `--c-code` · `--c-color` · `--c-image` · `--c-num` | `chart-1…5` | — |
| `--f-ui` · `--f-mono` | `font-sans`, `font-mono` | — |
| `--r-sm` · `--r-ctl` · `--r-row` · `--r-card` | `--radius-sm` / `--radius`,`--radius-md` / `--radius-lg` / `--radius-xl` | — |
| `--ease` | `--ease-cp` | — |

### Four choices in that table worth knowing about

1. **shadcn's `accent` is not our accent.** In shadcn, `accent` is the neutral
   hover/active surface for menu and list items; the brand colour is `primary`.
   So `--color-accent` maps to `--hover`, which also lines up with Material's
   state-layer model. Because that is a trap, `--color-brand` is exposed as an
   unambiguous alias for the real accent — `bg-brand` and `bg-primary` are the
   same colour.
2. **`sidebar-accent` maps to `--selected`, not to `--hover`.** shadcn uses that
   slot for the *active* nav item, and in v1 (`.sb__item.on`) the active item is
   the accent-tinted `--selected`. Material's `secondaryContainer` is the same
   idea, and gets the same alpha.
3. **`errorContainer` uses `--err` at `--alpha-tint` with `--err-strong` on
   top.** v1's `.btn--danger` is a 16% tint with AA-corrected text, not an
   opaque red. Using `--err` as the container foreground would reintroduce
   precisely the contrast bug §8.3 fixed.
4. **Material 3 requires a `tertiary` role CopyPaste does not have.** It aliases
   the `--accent-2` ramp rather than inventing a hue somebody would then have to
   maintain in one place only.

## Two tokens that are additions, not ports

Both are named as such in their `$description`, and both exist because a
component library needs something v1's hand-written CSS never had to express:

- **`--on-err`** (`#FFFFFF`) — v1 never rendered a solid error fill, but
  shadcn's `destructive` variant and Material's `onError` both need a foreground
  for one.
- **`--on-accent-2`** (`#1A1C22`) — foreground on the light accent ramp step,
  required by `onPrimaryContainer`. It is the light theme's `--text`, so the ink
  is one we already ship.

`--c-unknown` is a third, of a different kind: §8.7 closes the content-kind set
with `unknown`, but the v1 token dump has no `--c-unknown`. It aliases
`--c-text` so an unrecognised kind can never render blank.

## Verifying against v1

The generated CSS was checked declaration-by-declaration against the token block
in `docs/rewrite/design-reference.html`: all 116 declarations the reference
defines are value-identical. The only textual differences are formatting the
browser does not distinguish — `#FFFFFF` for `#fff`, and optional quoting and
whitespace inside the font stacks.

That was a one-off check, not a standing test. There is one source now; a test
asserting the generator agrees with the document the generator was seeded from
would be the v1 mistake again. What *is* worth testing lives in the apps:
that the rendered UI meets the contrast and behaviour requirements in
port-manifest §4.
