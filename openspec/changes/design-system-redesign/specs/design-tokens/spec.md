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
- **WHEN** any CSS rule in `src/styles/primitives.css`, `patterns.css`, `shell.css`, or
  `utilities.css` (the `@layer primitives`/`patterns`/`shell`/`utilities` source files) sets a
  `color`, `background`, `border-color`, `border-radius`, `box-shadow`, `padding`, `gap`, `margin`,
  or `transition-duration` property for a design constant (not runtime-computed geometry, see the
  design-constant-vs-runtime-geometry requirement below)
- **THEN** the value is a `var(--…)` reference (or a `color-mix(in srgb, var(--…) N%, …)`
  expression) — not a literal hex color or a literal px/ms value

#### Scenario: Reduced motion collapses all durations
- **WHEN** the user's OS has `prefers-reduced-motion: reduce` set
- **THEN** `--dur-fast`, `--dur`, and `--dur-theme` all resolve to `0ms`, and animations gated by
  those tokens (row insert/remove, toast, spinner pulse, selection glide, online-dot pulse) do not
  visibly animate

#### Scenario: Reduced motion also disables native smooth scrolling and non-token animation
- **WHEN** the user's OS has `prefers-reduced-motion: reduce` set
- **THEN** `scroll-behavior` resolves to `auto` (not `smooth`) at every scrollable element/`<html>`
  itself, and any keyframe `animation-duration`, `animation-iteration-count`, or
  `transition-duration` that does not reference a duration token is also confirmed to no-op —
  the audit is not limited to the three named duration tokens

### Requirement: Theme and accent are persisted user preferences
The system SHALL persist the user's chosen `theme` and `accent` in the same preferences store used
for all other UI preferences (`UIPrefs` in `src/store.ts`), applied on load to both the main
window and the quick-paste popup window before their first meaningful paint.

#### Scenario: Preference survives app restart
- **WHEN** the user sets theme to `light` and accent to `teal`, then quits and relaunches the app
- **THEN** both the main window and the popup open with `data-theme="light"` and
  `data-accent="teal"` already applied

#### Scenario: Preferences predating the new fields gain them at defaults (no migration)
- **WHEN** a stored preferences blob predates the `theme`/`accent`/`translucency` fields
- **THEN** all previously-saved preference values are retained unchanged, and the new fields are
  supplied at their documented defaults (`dark` / `indigo` / `true`) by the existing
  whitelist-merge-with-defaults — no migration step runs

### Requirement: A synchronous pre-paint bootstrap applies persisted theme/accent/translucency before first content paint
The system SHALL apply the user's persisted `theme`, `accent`, and `translucency` preferences to
`document.documentElement` via a synchronous **external same-origin classic script**
(`theme-bootstrap.js`, authorized by the app's `script-src 'self'` CSP — it MUST NOT be an inline
`<script>`, which that CSP blocks) referenced by both `index.html` and `popup.html` before the
React module entry, so no frame of the static default theme is ever visible to a user with a
non-default persisted preference. The script MUST contain no `import`/`eval`/`Function`. A React
effect SHALL keep the same attributes synchronized afterward for live changes.

#### Scenario: Persisted non-default theme is present before content is visible
- **WHEN** a user has persisted `theme: "light"`, `accent: "teal"`, `translucency: false` and
  opens either window
- **THEN** `document.documentElement.dataset.theme === "light"`,
  `document.documentElement.dataset.accent === "teal"`, and
  `document.documentElement.dataset.translucency === "off"` are all true before the first
  application content becomes visible, not merely after a React effect runs post-paint

#### Scenario: Missing, malformed, or unsupported persisted values fall back independently
- **WHEN** the persisted preferences value for `theme`, `accent`, or `translucency` is missing,
  is not one of the documented enum/boolean values, or the stored JSON itself fails to parse
- **THEN** each affected field falls back to its own documented default (`dark` / `indigo` /
  `true`) independently of the other fields, and the bootstrap script does not throw or block
  app startup

### Requirement: Translucency is a persisted, validated axis with a defined fallback
The system SHALL define `translucency: boolean` (default `true`) in `UIPrefs`, represented on the
DOM as `data-translucency="on"|"off"` on `document.documentElement` (and, for the gallery's scoped
preview wrapper, on `.theme-scope`), with `true`/`on` applying `backdrop-filter` frosting to chrome
surfaces only (sidebar, popup container, modal scrim, toast, tab bar) while content surfaces
(cards, rows, fields) remain solid, and `false`/`off` rendering every surface solid.

#### Scenario: Translucency on frosts chrome surfaces only
- **WHEN** `data-translucency="on"` is active
- **THEN** the sidebar, popup container, modal scrim, toast, and tab bar render with
  `backdrop-filter` frosting, while cards, list rows, and form fields render fully solid

#### Scenario: Translucency off renders every surface solid
- **WHEN** the user disables the Translucency toggle in Settings
- **THEN** `data-translucency="off"` is set and no surface in the app applies `backdrop-filter`

#### Scenario: Unsupported or reduced-transparency environments fall back to solid
- **WHEN** the runtime WebView does not support `backdrop-filter`, or the OS reports
  `prefers-reduced-transparency: reduce` and the WebView exposes that media feature
- **THEN** every chrome surface renders solid regardless of the persisted `translucency` value

### Requirement: The popup reflects current preferences on open (required); live cross-window update is best-effort
The system SHALL ensure the quick-paste popup applies the current persisted theme/accent/translucency
**every time it opens** (next-open correctness — the release-gate requirement, verified in a packaged
Tauri build because each WebView has a separate JS runtime and same-module/same-key does not by
itself prove cross-WebView storage semantics). The system SHOULD additionally update an already-open
popup live when a change is made in Settings, on a **best-effort** basis (via a Tauri
`ui-prefs-changed` event, plus a `storage` event where the WebViews share a `localStorage`
partition); a transient already-open popup that only corrects on its next open is acceptable and is
NOT a failure. Live update MUST NOT be specified as a hard `SHALL` anywhere.

#### Scenario: Popup shows current preferences when opened (required)
- **WHEN** the user changes theme/accent/translucency in Settings and then opens (or reopens) the
  popup, in a packaged Tauri build
- **THEN** the popup renders with the current values

#### Scenario: Already-open popup updates live where a channel exists (best-effort)
- **WHEN** the popup is already open and a change is made in Settings, and a live channel
  (`ui-prefs-changed` event, or `storage` event on a shared partition) reaches the popup
- **THEN** the popup's dataset attributes update without reopening; if no live channel reaches it,
  the popup instead corrects on its next open (acceptable, not a failure)

### Requirement: Persisted preferences are validated at runtime, per field, independently
The system SHALL validate every enum or boolean preference field (`theme`, `accent`,
`translucency`) individually when loading persisted preferences, defaulting only the invalid
field(s) while retaining every other valid field's stored value.

#### Scenario: One invalid field does not discard other valid fields
- **WHEN** persisted preferences contain a valid `accent` value but an unrecognized `theme` value
  (e.g. `"system"`)
- **THEN** the loaded preferences use the documented default for `theme` (`"dark"`) while
  preserving the stored, valid `accent` value unchanged

#### Scenario: Malformed JSON falls back to full defaults
- **WHEN** the persisted preferences value at the active storage key is not valid JSON
- **THEN** all preferences load as `DEFAULT_PREFS`, and the failure is logged rather than thrown
  to the caller

#### Scenario: Unknown keys are dropped
- **WHEN** persisted preferences contain a key that is not part of the current `UIPrefs` schema
- **THEN** that key is silently dropped from the loaded preferences and never re-persisted

### Requirement: No versioned-key migration or downgrade handling (additive fields only)
The system SHALL add `theme`/`accent`/`translucency` as additive fields on the existing `UIPrefs`
object at its current key (`copypaste-ui-prefs-v4`) with NO version bump, NO new migration chain, NO
dual-write, and NO downgrade handling — backward compatibility is explicitly out of scope (user
directive). A stored blob predating the fields simply gains them at their defaults via the existing
whitelist-merge-with-defaults on next load. Additionally, the system SHALL **remove the existing
legacy v1/v2/v3→v4 migration branches** from `store.ts` as an explicit part of the no-back-compat
policy; the accepted, documented impact is that a user whose prefs remain under an old v1–v3 key
(never re-saved under v4) has ALL their UI prefs reset to defaults.

#### Scenario: No migration code path exists (legacy branches removed)
- **WHEN** preferences are loaded, whatever their stored shape
- **THEN** they are read from the single current `v4` key and merged over defaults; there is no
  version-detection or legacy-key-forwarding branch (the former v1/v2/v3 branches are deleted), and
  no new versioned key is ever written

#### Scenario: A user still on an old v1–v3 key resets to defaults
- **WHEN** a user's prefs exist only under a legacy `v1`/`v2`/`v3` key and were never re-saved to v4
- **THEN** after this change the app does not read that legacy key; all UI prefs load as
  `DEFAULT_PREFS` (accepted no-back-compat impact)

### Requirement: Supported OS/WebView matrix is stated as a non-functional requirement
The system SHALL state macOS 13 (Ventura) or later — WKWebView backed by Safari 16.2+ — as the
minimum supported platform for this design system, and SHALL rely on native `color-mix()` support
without requiring a fallback rendering path for unsupported engines.

#### Scenario: color-mix() is used without a fallback layer
- **WHEN** any token or component rule uses `color-mix(in srgb, …)`
- **THEN** no fallback static-color rule is required to precede it, because the minimum supported
  WebView (Safari 16.2+ on macOS 13+) supports `color-mix()` natively

### Requirement: Design tokens and the reference file are kept in exact name-and-value parity
The system SHALL provide an automated check that every custom property defined in
`copypaste-design-reference.html`'s token block resolves to the identical value in
`crates/copypaste-ui/src/styles/tokens.css`, for both themes and all accent variants.

#### Scenario: A value drift in either file fails the check
- **WHEN** a token's value differs between the reference file and `tokens.css` for the same
  theme/accent combination, even if both files define a property of the same name
- **THEN** the parity check fails, distinguishing this from a name-only diff that would miss the
  value mismatch

### Requirement: Design-constant values are tokenized; runtime-computed geometry may remain inline
The system SHALL express every design-constant value (spacing, radius, shadow, focus-ring
width/offset, hairline width, icon sizes, control heights) as a token, while permitting
runtime-computed geometry (virtualized-list item positions, the popup row's measured height, the
glide-highlight overlay's computed position, the settings tab-bar's measured underline position)
to be expressed as inline styles or CSS custom properties set from JavaScript, since their values
are not known until layout/measurement time.

#### Scenario: A hardcoded design constant outside the tokens layer is a defect
- **WHEN** any authored CSS rule outside `tokens.css` sets a focus-ring width/offset, hairline
  width, icon size, or control height as a literal value instead of `var(--…)`
- **THEN** this is flagged as a token-policy violation

#### Scenario: Runtime-computed geometry is exempt from the no-hardcoded-pixel rule
- **WHEN** a virtualized list, the popup row, the glide-highlight overlay, or the settings
  tab-bar indicator sets an inline `style` or CSS custom property from a JavaScript-computed
  measurement
- **THEN** this is not a token-policy violation, because the value is inherently
  runtime-dependent and cannot be expressed as a static design token

### Requirement: Typography and layout tokens support text scaling and localization
The system SHALL define a typography scale (font stack(s), weight scale, line-height scale,
letter-spacing scale) using relative units where applicable, plus layout-constraint tokens for
minimum main-window dimensions and popup width, sufficient to support OS-level text scaling, 200%
browser zoom, and text-length variation from localization.

#### Scenario: 200% zoom reflows without breaking layout
- **WHEN** the browser zoom level is set to 200% or the OS text-scaling setting is increased
- **THEN** typography-driven surfaces reflow (wrap, truncate, or resize) using the relative-unit
  typography scale, without requiring two-dimensional scrolling for primary content

#### Scenario: A longer, localization-representative string does not break row layout
- **WHEN** a label or preview string approximately 40% longer than its English source string
  (representative of common European-language expansion) is rendered in place of the original
- **THEN** the containing row, label, or button reflows or truncates according to its documented
  overflow behavior without visually breaking the layout
