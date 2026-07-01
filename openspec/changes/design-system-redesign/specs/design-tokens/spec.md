## ADDED Requirements

### Requirement: Theme axis (dark/light) via `data-theme`
The system SHALL define a complete set of surface, line, text, overlay, and status CSS custom
properties for exactly two theme values, `dark` (default) and `light`, selected by a
`data-theme` attribute on the document root (`<html>`), with values copied verbatim from
`copypaste-design-reference.html`'s `:root[data-theme="dark"]` / `:root[data-theme="light"]`
blocks. No third theme value, palette, or skin axis SHALL exist.

#### Scenario: Root element carries the active theme
- **WHEN** the main window or the quick-paste popup window loads
- **THEN** `document.documentElement.dataset.theme` equals either `"dark"` or `"light"`, and every
  themed token (`--bg`, `--panel`, `--elevated`, `--card`, `--raised`, `--raised-2`, `--border`,
  `--divider`, `--text`, `--dim`, `--faint`, `--mute`, `--hover`, `--pressed`, `--selected`,
  `--scrim`, `--ok`, `--warn`, `--err`, `--info`, `--sh1`, `--sh2`, `--sh3`) resolves to the value
  defined for that theme

#### Scenario: Switching theme updates every surface without a reload
- **WHEN** the user changes the theme control in Settings → Appearance
- **THEN** `data-theme` on `<html>` updates immediately and every surface in the currently visible
  view re-renders with the new theme's token values, without a page reload

### Requirement: Accent axis (6 hues) via `data-accent`
The system SHALL define exactly six accent variants — `indigo` (default), `blue`, `teal`, `green`,
`amber`, `rose` — each providing `--accent`, `--accent-2`, and `--on-accent`, selected
independently of theme by a `data-accent` attribute on `<html>`, including the light-theme
accent-value overrides needed to keep AA contrast on white surfaces (per
`copypaste-design-reference.html` lines 41–46 / `STYLEGUIDE.md` §3.5).

#### Scenario: Accent is independent of theme
- **WHEN** the user selects any of the 6 accents while in either theme
- **THEN** `--accent`/`--accent-2`/`--on-accent` update to that accent's values (using the
  theme-specific override when in `light` theme) while all non-accent tokens remain unchanged

#### Scenario: Every accent maintains on-accent contrast
- **WHEN** any accent is active and a `--accent`-filled surface (e.g. a primary button) is
  rendered
- **THEN** the text/icon color on that surface (`--on-accent`) meets WCAG AA contrast against
  `--accent` in both themes, matching the values specified in `STYLEGUIDE.md` §3.5

### Requirement: Content-type color tokens
The system SHALL define one color token per clipboard content kind (`--c-text`, `--c-url`,
`--c-mail`, `--c-num` [shared by `PHONE`/`NUMBER`], `--c-code`, `--c-json`, `--c-color`,
`--c-file` [shared by `PATH`/`FILE`], `--c-image`, `--c-secret`), themed for both `dark` and
`light`, and used **only** for the content-type tile/glyph/meta-word — never applied as
whole-row or whole-card background tint (`STYLEGUIDE.md` §1 rule 1–2).

#### Scenario: A content-type token colors exactly the tile, glyph, and meta word
- **WHEN** a history row of a given `kind` is rendered
- **THEN** the row's tile background is `color-mix(in srgb, var(--c-<kind>) 14%, transparent)`,
  the tile glyph (or swatch/thumbnail for `COLOR`/`IMAGE`) uses `var(--c-<kind>)` at full
  strength, and the meta "type word" is tinted with the same token — and no other part of the row
  (background, border, title text) carries that color

### Requirement: Spacing, radius, shadow, and motion scale tokens
The system SHALL define fixed, non-themed scale tokens — spacing (`--s-1`…`--s-9`), radius
(`--r-chip`, `--r-pill`, `--r-ctl`, `--r-input`, `--r-card`, `--r-window`), elevation (`--sh1`,
`--sh2`, `--sh3`, themed), and motion (`--dur-fast`, `--dur`, `--dur-theme`, `--ease`) — matching
`copypaste-design-reference.html` Layer 1 and `STYLEGUIDE.md` §5–6. No component or view SHALL
declare a hardcoded pixel, color, or duration value that duplicates one of these tokens.

#### Scenario: No magic numbers in component CSS
- **WHEN** any CSS rule in `src/index.css`'s primitives/patterns/app-shell/utilities layers sets a
  `color`, `background`, `border-color`, `border-radius`, `box-shadow`, `padding`, `gap`, `margin`,
  or `transition-duration` property
- **THEN** the value is a `var(--…)` reference (or a `color-mix(in srgb, var(--…) N%, …)`
  expression) — not a literal hex color or a literal px/ms value

#### Scenario: Reduced motion collapses all durations
- **WHEN** the user's OS has `prefers-reduced-motion: reduce` set
- **THEN** `--dur-fast`, `--dur`, and `--dur-theme` all resolve to `0ms`, and animations gated by
  those tokens (row insert/remove, toast, spinner pulse, selection glide, online-dot pulse) do not
  visibly animate

### Requirement: Theme and accent are persisted user preferences
The system SHALL persist the user's chosen `theme` and `accent` in the same preferences store used
for all other UI preferences (`UIPrefs` in `src/store.ts`), applied on load to both the main
window and the quick-paste popup window before their first meaningful paint.

#### Scenario: Preference survives app restart
- **WHEN** the user sets theme to `light` and accent to `teal`, then quits and relaunches the app
- **THEN** both the main window and the popup open with `data-theme="light"` and
  `data-accent="teal"` already applied

#### Scenario: Existing preferences migrate without data loss
- **WHEN** a user upgrades from a build whose persisted preferences predate the `theme`/`accent`
  fields
- **THEN** all previously-saved preference values are retained unchanged, and `theme`/`accent` are
  initialized to their documented defaults (`dark` / `indigo`)
