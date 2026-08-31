# CopyPaste design tokens

`design/tokens/` is the only source of visual constants for the product. Style
Dictionary generates `design/dist/css/` and Android Material resources;
generated output is committed so native and web builds consume the same
artifact.

## Appearance model

Appearance has two choices:

- mode: `system`, `light`, or `dark`;
- theme: `midnight`, `aurora`, `ember`, or `graphite`.

Every theme is a coordinated palette, including surfaces, borders, selection,
focus, brand fill and text-safe brand roles. There is no separate user accent
preference and no platform colour override. The semantic variables named
`--accent`, `--accent-2`, and `--on-accent` are roles owned by the active theme,
not another appearance axis.

The eight resolved palettes live in:

- `tokens/color/themes.dark.json`;
- `tokens/color/themes.light.json`.

Scheme-wide text, content-kind and status roles live in
`tokens/color/theme.dark.json` and `theme.light.json`. Theme-picker previews are
generated from the shipping palette values; they have no duplicate token source.

The document attributes are:

```text
data-color-scheme="dark|light"              resolved mode
data-mode="system|dark|light"               stored choice
data-theme="midnight|aurora|ember|graphite" product theme
data-translucency="on|off"                  token-backed chrome treatment
```

The pre-paint bootstrap and runtime apply exactly this contract. Components
consume semantic variables only and never write colours, dimensions or motion
values at runtime.

## Token ownership

| Source | Owns |
|---|---|
| `tokens/color/` | semantic colours and the eight palettes |
| `tokens/spacing.json` | spacing rhythm |
| `tokens/size.json` | controls, icons, strokes and hit targets |
| `tokens/layout.json` | window and shell geometry |
| `tokens/radius.json` | the shared radius ramp |
| `tokens/typography.json` | families, sizes, weights, line heights and tracking |
| `tokens/motion.json` | durations and easing, including reduced-motion values |
| `tokens/translucency.json` | solid/frosted chrome with platform fallbacks |
| `tokens/elevation.*.json` | shadows and the focus halo |
| `tokens/android.json` | Android resource aliases and native-only geometry |

Feature and component styles may compose geometry with `calc()`, but colour
recipes belong in `tokens/color/recipes*.json`. Components consume the emitted
semantic variable; they never evaluate `color-mix()` or introduce a literal
palette colour. A new reusable value belongs in `tokens/` before it is consumed.

Recipe fallbacks are resolved with Culori, the package already used directly by
the contrast gate. It supports the authored OKLab interpolation and alpha
semantics, so generation and validation share one maintained colour engine
instead of carrying hand-written colour maths or another dependency.

## Generated CSS

`npm run build` emits:

- `tokens.base.css` for theme-independent roles;
- `tokens.dark.css` and `tokens.light.css` for scheme roles;
- `themes.dark.css` and `themes.light.css` for product palettes;
- `swatches.dark.css` and `swatches.light.css` for derived picker previews;
- `index.css` as the maintained import order.
- `values/colors.xml`, `values-night/colors.xml` and `values/dimens.xml` for
  Android Material resource consumers.

Dark Midnight is the safe pre-bootstrap fallback. Once the bootstrap runs, the
resolved scheme and product theme selectors take over before first paint.
Recipe CSS is generated as concrete `rgb()`/`rgba()` per palette, which keeps
the same treatment on Chrome 53/74 WebViews that do not understand
`color-mix()`.

## Validation

Run from this directory:

```sh
npm run build
npm run check:contrast
npm run check:usage
npm run check:android
```

The contrast gate reads generated CSS and evaluates all eight palettes. It
checks text at 4.5:1 and interactive boundaries/focus indicators at 3:1. The
usage gate scans shipping UI sources for literal colours, diluted focus roles,
unsafe sensitive-content treatments and required semantic roles.

`prefers-reduced-motion` collapses motion tokens. Frosted values apply only when
backdrop filtering is supported, and `prefers-reduced-transparency` restores
solid values. Sensitive content is absent rather than visually obscured, so no
clipboard-content blur token exists.

Android resource colours take the same Midnight fallback that the WebView uses
before appearance bootstrap; the WebView continues to render the selected
product theme. The Android token source aliases that fallback and semantic
scheme roles instead of owning a second palette. Its 48dp touch target is a
deliberate native Material variant of the WebView's 44px coarse-pointer floor.
The QR background is intentionally true white because its renderer's modules
are true black; it is a scanner contrast exception, not a themed surface.
