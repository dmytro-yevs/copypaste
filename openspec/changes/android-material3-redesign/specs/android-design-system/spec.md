## ADDED Requirements

### Requirement: CpColors semantic token set
The design system SHALL define a `CpColors` holder providing dark and light values for every surface, line, text, overlay, status, and content-type token in STYLEGUIDE §3, with no other source of truth for these values. Values are sourced from `crates/copypaste-ui/src/styles/tokens.css` at pinned desktop commit `6960539d` (STYLEGUIDE §10/§11 is re-pinned from the same commit as the human-readable mirror; it is not the machine source). The fields SHALL be: surfaces (`bg`, `panel`, `elevated`, `card` as an explicit alias of `elevated`, `raised`, `raised2`), lines (`border`, `divider`), text (`text`, `dim`, `faint`, `mute`), overlays (`hover`, `pressed`, `scrim`), status (`ok`, `warn`, `err`, `info`, `errStrong`, `infoStrong`, `okStrong` — the three additive AA-safe text variants used where a status color is rendered as small/semibold TEXT over its own tint, rather than as a fill/dot/syntax hue), and ten content-type colors (`cText`, `cUrl`, `cMail`, `cNum`, `cCode`, `cJson`, `cColor`, `cFile`, `cImage`, `cSecret`). All values are Compose `Color` (dp/sp/alpha are not colors); alpha-derived tints are produced with `token.copy(alpha = …)`.

#### Scenario: Dark surface and text tokens match the guide
- **WHEN** `CpColors` resolves for the dark theme
- **THEN** bg is `#0E0F14`, panel is `#16181F`, text is `#E7E9EE`, and dim is `#9CA1AC`

#### Scenario: Light surface and text tokens match the guide
- **WHEN** `CpColors` resolves for the light theme
- **THEN** bg is `#F5F6F8`, panel is `#FFFFFF`, border is `#E1E4E9`, and divider is `#ECEEF1`

#### Scenario: Overlay and scrim tokens are defined
- **WHEN** `CpColors` resolves for either theme
- **THEN** `hover`, `pressed`, and `scrim` resolve to the STYLEGUIDE §3.4 values (dark hover `rgba(255,255,255,.045)`, pressed `.075`, scrim `rgba(0,0,0,.55)`; light hover `rgba(15,18,26,.045)`, pressed `.075`, scrim `rgba(20,22,30,.28)`)

#### Scenario: Status and content-type tokens are complete with defined aliases
- **WHEN** `CpColors` resolves for either theme
- **THEN** ok/warn/err/info are all defined, all ten content-type color fields resolve to the exact hex values in STYLEGUIDE §3.7 (e.g. dark cUrl `#34D1BF`, cSecret `#F2616B`)
- **AND** the twelve content kinds (resolved via `ContentVisualKind`) map onto these ten colors with PHONE aliased to `cNum` and PATH aliased to `cFile` — there is no distinct `cPath` field

#### Scenario: Strong status text variants pass AA where the base status color would not
- **WHEN** a status color is rendered as small/semibold TEXT over its own tint (e.g. the danger
  button label, the log-level badge, the verified badge) rather than as a fill/dot/syntax hue
- **THEN** the `errStrong`/`infoStrong`/`okStrong` variant is used instead of the base
  `err`/`info`/`ok` token, matching `crates/copypaste-ui/src/styles/tokens.css` (dark theme:
  identical to the base token; light theme: `errStrong #B93434`, `infoStrong #1D4ED8`,
  `okStrong #157A42`)

### Requirement: Selected and disabled treatments are centrally derived
The design system SHALL derive the selected-surface tint from the active accent — accent at 16% alpha in dark, 12% in light — and SHALL define the disabled treatment as one central rule (a `mute` foreground with the STYLEGUIDE §9.1 45% control opacity), so no screen computes its own selected or disabled color.

#### Scenario: Selected tint follows the active accent
- **WHEN** a row or control is in the selected state under a given accent and theme
- **THEN** its tint is the active accent color at 16% alpha (dark) or 12% alpha (light), recomputed when the accent changes

#### Scenario: Disabled treatment is uniform
- **WHEN** any control is disabled
- **THEN** it renders through the single central disabled rule (mute foreground / 45% opacity), not a per-screen alpha literal

### Requirement: AccentColor six-hue enum with AA-safe on-accent pairs
The design system SHALL define an `AccentColor` enum with exactly six values — indigo (default), blue, teal, green, amber, rose — each carrying a dark base, a light base, an onDark foreground, an onLight foreground, and a variant (accent-2) color sourced verbatim from STYLEGUIDE §3.5/§11.

#### Scenario: Indigo is the default accent
- **WHEN** no accent preference is set
- **THEN** `AccentColor.INDIGO` is used, with dark base `#6E5BFF`, light base `#5B49E0`, and onDark/onLight both `#FFFFFF`

#### Scenario: Deep-toned accents use dark on-accent text
- **WHEN** the active accent is teal, green, or amber
- **THEN** its onDark foreground is the guide's near-black variant (not white), holding AA contrast against that accent's dark-mode base

#### Scenario: Every accent meets AA contrast in both themes
- **WHEN** any of the six accents is active in either theme
- **THEN** its on-accent foreground meets at least 4.5:1 contrast against the accent base (3:1 for large text/UI affordances)

### Requirement: CopyPasteTheme Material3 role mapping
`CopyPasteTheme(isDark, accent, translucency, content)` MUST build the M3 `ColorScheme` by mapping `CpColors`/`AccentColor` onto roles — primary/onPrimary from the active accent, background/onBackground from bg/text, surface/onSurface from panel/text, surfaceVariant from elevated, outline/outlineVariant from border/divider, error from err — and MUST disable dynamic color and set `surfaceTint` to `Transparent` so the brand accent is authoritative.

#### Scenario: Dynamic color never overrides the brand accent
- **WHEN** the composable runs on Android 12+ with dynamic color available
- **THEN** `colorScheme.primary` still resolves to the active `AccentColor` base, not a wallpaper-derived color

#### Scenario: surfaceTint does not wash out elevated surfaces
- **WHEN** a `Card` or `Surface` elevates above the panel
- **THEN** no tint overlay is applied because `surfaceTint` is `Transparent`; elevation reads from the raised/raised2 tokens instead

### Requirement: Explicit Material3 role table
`CopyPasteTheme` SHALL map `CpColors`/`AccentColor` onto every M3 `ColorScheme` role the app consumes, per the table below, and SHALL NOT leave a consumed role at its M3 default. Roles not in the table SHALL NOT be consumed by screens; the selected/hover/pressed states use `CpColors`, never `primaryContainer`.

| M3 role | Source |
|---|---|
| primary / onPrimary | active accent base / on-accent |
| background / onBackground | bg / text |
| surface / onSurface | panel / text |
| surfaceContainerLowest | bg |
| surfaceContainerLow | panel |
| surfaceContainer | elevated (card) |
| surfaceContainerHigh | raised |
| surfaceContainerHighest | raised2 |
| surfaceVariant / onSurfaceVariant | elevated / dim |
| outline / outlineVariant | border / divider |
| error / onError | err / **`#000000` in both themes (AA ≥ 4.5:1) — NOT white** |
| errorContainer / onErrorContainer | `err @ 12%` / err |
| scrim | scrim |
| surfaceTint | Transparent |

`onError = #FFFFFF` is rejected: white on dark `err #E5645F` ≈ 3.32:1 and on light `err #D64545` ≈
4.38:1 both fail AA body text; `onError = #000000` is pinned instead. Contrast is computed via the
WCAG relative-luminance formula (linearize sRGB, `L = 0.2126R + 0.7152G + 0.0722B`,
`ratio = (L1 + 0.05) / (L2 + 0.05)`) against the `--err` values in
`crates/copypaste-ui/src/styles/tokens.css` at pinned commit `6960539d`: `#000000` on dark
`err #E5645F` ≈ **6.32:1**, `#000000` on light `err #D64545` ≈ **4.80:1** — both ≥ 4.5:1 AA.

#### Scenario: Container ladder resolves to the surface tokens
- **WHEN** a composable reads `surfaceContainer`, `surfaceContainerHigh`, or `surfaceContainerHighest`
- **THEN** it resolves to `elevated`, `raised`, and `raised2` respectively, not an M3 tonal default

#### Scenario: Non-mapped roles are not consumed
- **WHEN** the codebase is inspected for use of `primaryContainer`, `secondary`, `tertiary`, or `inverseSurface` in screen composables
- **THEN** none are found; selected/hover/pressed and status/content colors are taken from `CpColors`/`AccentColor` instead

#### Scenario: Error foreground meets AA
- **WHEN** text/icon is laid on a filled `error` surface in either theme
- **THEN** the `onError` foreground meets AA (≥ 4.5:1), and the token/contrast test covers error/onError and errorContainer/onErrorContainer

#### Scenario: No default M3 role leaks through Material components
- **WHEN** a shared Material component (chip, switch, slider, dialog, text field) renders
- **THEN** the full `ColorScheme` is mapped to canonical values for every M3 role (the single chosen
  strategy — not per-component overrides), so no default tonal/purple role is rendered; a component
  gallery golden covering every used M3 component detects leakage

### Requirement: Content visual-kind resolver (Android has no HistoryEntry.kind)
The design/history layer SHALL define a Kotlin presentation enum (e.g. `ContentVisualKind`) resolved
from the ACTUAL data model — `ClipboardItem.contentType`, the Rust `TextKind.classify` sub-kinds, and
the orthogonal `ClipboardItem.isSensitive` — because there is no stored `HistoryEntry.kind`. Resolver
precedence SHALL be: (1) `isSensitive` → SECRET visual kind (the approved new lock treatment); (2)
image/file from canonical content-type predicates; (3) text subtype from `TextKind.classify`; (4)
unknown/stub → TEXT. Stored/sync content-type and the Rust classifier contracts SHALL NOT change.

#### Scenario: Sensitive overrides to SECRET visual kind
- **WHEN** an item has `isSensitive = true`
- **THEN** its visual kind resolves to SECRET (lock tile + `cSecret`), regardless of its text subtype

#### Scenario: Non-sensitive falls through to type/subtype
- **WHEN** an item is not sensitive
- **THEN** the resolver yields image/file from content-type, else the `TextKind.classify` subtype, else TEXT

#### Scenario: Stored contracts unchanged
- **WHEN** the resolver runs
- **THEN** it reads existing fields only; `ClipboardItem.contentType` storage and the `TextKind` FFI are not modified

### Requirement: CpShapes fixed corner radii
`CpShapes` SHALL expose fixed corner-radius tokens with no per-theme or per-skin variation: chip = 7dp, ctl = 8dp, input = 9dp, card = 13dp, pill = 999dp, matching STYLEGUIDE §5.

#### Scenario: Shape tokens are constant across themes
- **WHEN** `CpShapes` is read in either dark or light theme
- **THEN** chip/ctl/input/card/pill resolve to 7/8/9/13/999 dp respectively in both cases

### Requirement: CpTypography semantic type roles
`CpTypography` SHALL map the STYLEGUIDE §4 semantic roles onto Inter (UI) and JetBrains Mono (machine-shaped) with tabular figures for mono numerics, using these exact frozen values (no ranges):

| Role | Family | Weight | Size sp | Line-height sp | Tracking | M3 role |
|---|---|---|---|---|---|---|
| title | Inter | 700 | 22 | 27 | 0 | headlineSmall |
| section | Inter | 600 | 14 | 18 | 0.01em | titleSmall |
| body | Inter | 400 | 14 | 20 | 0 | bodyLarge |
| body-emphasis | Inter | 500 | 14 | 20 | 0 | bodyLarge (emphasis) |
| body-mono | JetBrains Mono | 400 | 13 | 19 | 0 | bodyMedium |
| meta | Inter | 400 | 11.5 | 16 | 0 | bodySmall |
| micro | JetBrains Mono | 500 | 10 | 10 | 0.08em (UPPERCASE) | labelSmall |

To keep cross-platform parity with STYLEGUIDE §4 (Title 700), **S1 bundles a real Inter 700 face +
license** — the one weight not currently in `res/font`; all other roles use the existing bundled faces
(Inter 400/500/600, JetBrains Mono 400/500). NO synthetic weights. A font-resource test SHALL assert
every role resolves to a real bundled face (no synthesis/system fallback). Title is NOT downgraded to
600 (that would silently diverge from the shared guide/desktop).

#### Scenario: Exact per-role values (no ranges)
- **WHEN** `CpTypography` is defined
- **THEN** each semantic role fixes an exact sp size, weight, line-height, and tracking (not a range),
  mapped to M3 Typography roles where Material components consume them, using bundled Inter/JetBrains
  Mono weights (no synthetic weights)

#### Scenario: Machine-shaped text uses the mono family
- **WHEN** a clip preview, fingerprint, or IP value is rendered
- **THEN** its `TextStyle` resolves to the JetBrains Mono `FontFamily` loaded from `res/font`

#### Scenario: Numeric mono text does not shift width
- **WHEN** a timestamp or RTT value updates on screen
- **THEN** digit widths remain fixed via JetBrains Mono's fixed-width figures (or a verified
  `FontFeatureSetting` such as `tnum`), expressed in Compose — not a CSS `font-variant` setting

### Requirement: CpMotion durations and system-driven reduced motion
`CpMotion` SHALL define fixed durations — fast = 120ms, default = 200ms, theme = 300ms — using the STYLEGUIDE §6 easing curve, and its reduced state MUST be derived from the system animator-duration-scale signal rather than any in-app user setting.

#### Scenario: System reduced-motion collapses durations
- **WHEN** the OS animator duration scale is 0 (system "remove animations" is on)
- **THEN** `CpMotion.reduced` is true and all three durations resolve to 0ms

#### Scenario: No in-app motion toggle exists
- **WHEN** a user opens Settings → Display
- **THEN** there is no user-facing control for animation speed or motion reduction

### Requirement: Translucency and blur rendering policy
Chrome and sheet surfaces SHALL render a real **backdrop** blur — sampling the content behind the surface via a captured-layer `RenderNode`/`RenderEffect` strategy (per the S0/S1 spike), or a window-level blur for own-window surfaces — when the device is API 31+ AND translucency is enabled, and MUST fall back to an opaque canonical surface on API 26–30 or when translucency is disabled. Own-layer blur (`Modifier.blur` on the surface itself) is REJECTED for frosted chrome because it blurs the surface's own children; own-layer blur is permitted only where blurring the content itself is intended (e.g. sensitive masking). This decision MUST NOT delay first paint.

#### Scenario: Modern device with translucency on
- **WHEN** the app runs on API 31+ with translucency enabled
- **THEN** the floating tab bar and bottom sheets render with real background blur over a semi-transparent surface

#### Scenario: Legacy device or translucency off
- **WHEN** the app runs on API 26–30, or translucency is disabled regardless of API level
- **THEN** chrome renders as an opaque solid surface using the canonical fallback color, never a flat reduced-alpha layer over arbitrary content

#### Scenario: Blur never blocks first paint
- **WHEN** a screen with translucent chrome first composes
- **THEN** the underlying content draws before or concurrently with the blur effect, never gated behind it

### Requirement: Spacing, elevation, and component-dimension tokens
The design system SHALL provide `CpSpacing` (the 2/4/6/8/11/14/16/20/24 dp scale), `CpElevation`
(Compose shadow approximations for sh1/sh2/sh3 — CSS blur/offset/spread cannot be copied verbatim, so
Android equivalents are defined), a `CpDimensions` set of exact role constants (below), and named semantic alpha/state tokens beyond
selected/disabled. Raw dp/sp/alpha for these SHALL live only in token or shared-component files, never
in screens. Frozen `CpDimensions` (no ranges):

| Token | Value | Used for |
|---|---|---|
| `tileSm` / `tileMd` | 32dp / 36dp | content-type tile container (list default `tileMd`) |
| `glyphBox` | 18dp | glyph inside a tile |
| `navGlyph` | 24dp | bottom-nav icon |
| `iconMeta` | 20dp | inline meta/action icon |
| `toggleW`×`toggleH` / `knob` | 38×22dp / 18dp | switch |
| `navPillW`×`navPillH` | 50×38dp | active-tab pill |
| `qr` / `qrQuiet` | 220dp / 16dp | pairing QR + quiet zone |
| `sasCell` | 44dp | one of six SAS digit cells |
| `touchMin` | 48dp | minimum touch target (separate from visual size) |
| `navBottomClearance` | 12dp | floating nav pill clearance above the resolved bottom system-bar/gesture inset |
| width classes | compact <600 · medium 600–840 · expanded ≥840 dp | WindowSizeClass breakpoints |

#### Scenario: Spacing scale is tokenized
- **WHEN** a screen needs a gap/padding from the canonical scale
- **THEN** it uses a `CpSpacing` token, not a raw dp literal

#### Scenario: Elevation is an Android approximation
- **WHEN** a card/popup/modal needs sh1/sh2/sh3
- **THEN** it uses `CpElevation`'s Compose approximation, documented as not a verbatim CSS shadow

#### Scenario: Component geometry is centralized
- **WHEN** a toggle, tile, or nav pill is built
- **THEN** its fixed dimensions come from `CpDimensions`, not per-screen dp literals

### Requirement: Token-only screens with one encoding per fact
Screens SHALL consume only the token holders (`CpColors`/`AccentColor`/`CpShapes`/`CpTypography`/
`CpMotion`/`CpSpacing`/`CpElevation`/`CpDimensions`) and MUST NOT hardcode raw hex colors, raw dp/sp
outside those tokens, or arbitrary alpha; documented component geometry lives in `CpDimensions` or the
shared-component file that owns it (not in screens). A clip's content-type SHALL be encoded exactly
once (the tile), never redundantly repeated on the row background, border, or text color.

#### Scenario: No raw hex in screen source
- **WHEN** a screen composable outside `ui/theme/*` is inspected for `Color(0x...)` literals
- **THEN** none are found; all colors resolve through `LocalCpColors`/`LocalAccent` or `MaterialTheme.colorScheme`

#### Scenario: Content-type color is not redundantly applied
- **WHEN** a history row renders a CODE clip
- **THEN** only the tile is tinted with `cCode`; the row background, border, and preview text remain neutral
