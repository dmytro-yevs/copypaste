## ADDED Requirements

### Requirement: Multiple authored CSS source files compiled into one emitted stylesheet, with native cascade layers
The system SHALL author CSS as multiple source files under
`crates/copypaste-ui/src/styles/{reset,tokens,base,primitives,patterns,shell,utilities}.css`,
imported in that order by one entry module (`src/styles/index.css`) that both the main window and
popup entry points import, declaring native `@layer reset, tokens, base, primitives, patterns,
shell, utilities` so cascade order is enforced by the browser rather than by file or import order
alone. No Tailwind utility classes, CSS-in-JS, CSS Modules, or per-component stylesheet files
SHALL be introduced. Gallery-only CSS SHALL be excluded from the production stylesheet.

#### Scenario: One emitted stylesheet serves both windows, from multiple authored source files
- **WHEN** the main window (`index.html`) and the quick-paste popup window (`popup.html`) each
  load
- **THEN** both resolve to one emitted stylesheet built from `src/styles/index.css`'s ordered
  `@import`s, and no other project-authored production CSS file is loaded

#### Scenario: Cascade order is enforced by native layers, not file order alone
- **WHEN** a rule in an earlier-imported file and a rule of equal selector specificity in a
  later-imported file both target the same element
- **THEN** the later-declared `@layer` wins per the `@layer reset, tokens, base, primitives,
  patterns, shell, utilities` declaration order, regardless of the physical order the underlying
  `@import` statements happen to appear in

#### Scenario: No selector escalates specificity outside its owning layer
- **WHEN** any view-specific override is authored
- **THEN** it uses a low-specificity class selector and lives within its semantically correct
  layer (e.g. a `patterns`-layer override, not a higher-specificity `shell`-layer rule stacked to
  win the cascade) — no ID selector or `!important` is used to force priority

#### Scenario: Gallery-only CSS is excluded from the production stylesheet
- **WHEN** the production build's emitted stylesheet is inspected
- **THEN** it contains no selector that only the gallery view renders (e.g. matrix-grid layout,
  forced-state helper classes), because that CSS lives in a separate file imported only by the
  DEV-gated gallery module

#### Scenario: No Tailwind or CSS-in-JS remains
- **WHEN** the `copypaste-ui` source tree is searched for Tailwind utility classes, a
  `tailwind.config.*` file, or CSS-in-JS usage (`styled-components`, emotion, inline `style={{}}`
  used for anything other than dynamically computed per-instance values already required by
  existing logic, e.g. `--ct` custom-property wiring, or the runtime-computed-geometry cases
  documented in the `design-tokens` capability)
- **THEN** none are found

### Requirement: Reusable components are chosen by semantic reuse criteria, not occurrence count
The system SHALL reuse a shared React component when call sites align on anatomy, semantics,
interaction contract, accessibility contract, and supported variants, and SHALL reuse only shared
tokens/CSS primitives (not a shared component) when call sites align on presentation alone but
differ in semantics, lifecycle, or expected future change direction. Behavior-heavy patterns —
modal/dialog, toggle, segmented control, banner-with-actions, and expandable disclosure rows —
SHALL always be implemented as a typed React primitive, never as CSS-class-only reuse, because a
CSS class cannot guarantee keyboard, focus, or ARIA behavior. At minimum, the shared primitive set
covers: buttons (`.btn` + `--primary`/`--secondary`/`--ghost`/`--danger` variants, `.sm` size,
`disabled` state), icon buttons (`.iconbtn` incl. `.danger`), the toggle switch (`.toggle` /
`.off`), the segmented control (`.seg`), the search/text field (`.field`), chips (`.chip` /
`.chip.on` / `.chip--ct`), transport/identity pills (`.tpill--p2p` / `.tpill--cloud` /
`.tpill--this`), the verified badge (`.badge--verified`) and count badge (`.badge--count`), the
content-type tile (`.tile`, `.tile--swatch`, `.tile--thumb`), the keycap (`.kbd`), the presence dot
(`.dot-stat` / `.off`), and the shared `Dialog` primitive.

#### Scenario: Two visually similar elements with different semantics are not forced into one component
- **WHEN** two call sites render visually similar markup but differ in semantics, interaction
  contract, or accessibility contract (e.g. they are expected to diverge as a future feature
  lands)
- **THEN** they may share tokens/CSS classes for presentation without being forced into a single
  shared React component

#### Scenario: A behavior-heavy pattern is always a typed primitive, even at a single call site
- **WHEN** a modal/dialog, toggle, segmented control, banner-with-actions, or expandable
  disclosure row is implemented anywhere in the app
- **THEN** it is implemented as a typed React primitive with its documented accessibility
  contract, not merely a CSS class applied to ad hoc markup

#### Scenario: Every button variant renders from the shared class set
- **WHEN** `ActionButton` (or any raw `<button>` in the codebase) renders a primary, secondary,
  danger, or ghost action anywhere in the app (History bulk bar, Settings, device pairing/confirm
  dialogs, banners)
- **THEN** it applies the shared `.btn`/`.btn--<variant>` classes and no component defines its own
  competing button styling

#### Scenario: Disabled and pending states are visually consistent everywhere
- **WHEN** any `.btn` is `disabled` (including `ActionButton`'s `pending` state)
- **THEN** it renders at the token-defined disabled opacity with pointer events suppressed,
  identically across every call site

### Requirement: Button-shaped controls use their designated allowed primitive, not a forced `.btn`
The system SHALL define distinct allowed primitives for button-shaped controls whose anatomy and
interaction differ from a standalone action button: the `.btn` family (standalone primary/
secondary/ghost/danger actions), `.iconbtn` (icon-only actions), `.set-tab` (settings/segmented
tabs, using native tab semantics), a disclosure header (expandable-row triggers, using
`aria-expanded`/`aria-controls`, not `.btn` styling), `.chip` (filter/selection chips), and
per-row action icon buttons (hover-revealed row actions). No component SHALL be forced to render
through the `.btn` family solely because it is interactive.

#### Scenario: A tab, disclosure header, or chip does not use `.btn` styling
- **WHEN** a settings tab, a device-row disclosure trigger, or a filter chip is rendered
- **THEN** it uses its own designated primitive (`.set-tab`, the disclosure-header pattern, or
  `.chip` respectively), not the `.btn` family's classes

### Requirement: Naming and state contract is consistent across variants and states
The system SHALL express component variants as modifier classes (e.g. `.btn--small`,
`.btn--danger`), express state using native platform states where one exists (`:disabled`,
`[aria-selected="true"]`, `[aria-expanded="true"]`), and express state with no native equivalent
via explicit `data-state` attributes (e.g. `[data-state="removing"]`, `[data-kind="secret"]`).
React component state/props SHALL be the authoritative source of truth; ARIA attributes SHALL be
derived from that state; CSS SHALL read ARIA/`data-*` attributes and SHALL NOT independently track
state that can desynchronize from the DOM's exposed ARIA state.

#### Scenario: State is never tracked by a CSS-only mechanism independent of ARIA/data attributes
- **WHEN** any component exposes an interactive state (selected, expanded, removing, disabled)
- **THEN** that state is reflected in a native attribute, ARIA attribute, or `data-state`
  attribute driven by React state, and no CSS rule keys off a class that a script could apply
  without also updating the corresponding ARIA/data attribute

### Requirement: Clipboard history row covers every content kind with one component
`HistoryRow.tsx` and `PopupRow.tsx` SHALL remain separate layout wrappers, each composed from
shared clipboard-presentation units — `normalizeContentKind()`, a typed kind→token/icon/label map,
`ContentTile`, `ClipPreview`, and `ClipMetadata` — so content-kind interpretation is defined once
and cannot drift between the two rows, while each row keeps its own anatomy (tile, single-line
ellipsized preview, meta line `kind · sourceApp · relTime · originDevice`, pin affordance, and
hover-revealed actions).

#### Scenario: normalizeContentKind() handles the full kind space, not just the 11 named kinds
- **WHEN** `normalizeContentKind()` is called with each of the 11 named kinds (case-insensitively
  and with known aliases), an unrecognized string, `undefined` (since `HistoryEntry.kind` is
  `kind?: string`, not a closed union), or a value only a future daemon version would emit
- **THEN** the 11 named kinds normalize to their documented kind (including `PATH`/`FILE` sharing
  the `file` token and `PHONE`/`NUMBER` sharing the `num` token), and every other input —
  unrecognized, undefined, or future — normalizes to an explicit `"unknown"` kind with its own
  fallback icon, token, and label, never to a runtime error or a silently blank tile

#### Scenario: kind takes precedence over content_type, except for the image case
- **WHEN** an entry has both a `kind` and a `content_type` that disagree
- **THEN** `kind` is used, except when `content_type` indicates an image MIME type and `kind` is
  absent or contradictory, in which case the entry normalizes to `"image"`

#### Scenario: Sensitive content is masked visually only; copy/paste is unaffected
- **WHEN** a `secret`-kind row is rendered and masking is enabled
- **THEN** the preview text is visually masked (blurred) but occupies its real rendered width (no
  length masking — an accepted, documented trade-off), text selection remains unrestricted, and
  copying the item returns the real item data exactly as a non-sensitive item would — never text
  read from the visually-masked DOM span

#### Scenario: The accessible name does not leak the secret while masked
- **WHEN** a `secret`-kind row is masked and not revealed
- **THEN** its accessible name (e.g. `aria-label`) is a placeholder describing the hidden state
  (e.g. "Sensitive item, hidden — activate to reveal"), never the underlying plaintext; once
  revealed, the accessible name updates to reflect the now-visible content

#### Scenario: Masked content re-hides on window blur and optionally after a reveal timeout
- **WHEN** a sensitive item is revealed and the window loses focus
- **THEN** it re-masks automatically (existing `useSensitiveReveal` behavior, unchanged); if the
  optional reveal-timeout preference is enabled, the item also re-masks after the configured
  duration of inactivity even without a focus-loss event

#### Scenario: Pin and delete actions are reachable via hover, keyboard focus, and touch
- **WHEN** a row is hovered by a fine pointer, receives keyboard focus (`:focus-within`), or is
  rendered on a coarse/touch pointer where hover cannot occur
- **THEN** its pin and delete affordances are visible/focusable via hover, via keyboard focus, and
  always-visible on touch/coarse-pointer devices respectively — each a real `<button>` with an
  `aria-label` naming the action and the item, never a click-only `<div>`, and never focusable
  while visually hidden

#### Scenario: Multi-select replaces the tile with a checkbox
- **WHEN** History enters selection mode
- **THEN** every row's tile position is replaced by a checkbox control, and per-row hover actions
  (pin/delete) are hidden while selection-mode actions (bulk pin/delete) are shown in the bulk
  action bar instead

#### Scenario: The source-app slot always reserves its layout space with a defined fallback
- **WHEN** any row is rendered (the daemon does not currently emit a source-app icon field)
- **THEN** the row unconditionally reserves the source-app icon's layout space and renders the
  generic type-glyph fallback with an accessible label taken from the existing source-app name
  text field, not a per-app icon and not a generic "unknown" label

### Requirement: Device list uses one expandable row component for both own device and peers
`DeviceCard.tsx`'s `ThisDeviceCard` and `PeerRow` SHALL render through the same expandable
row/field-grid pattern (`.devrow`/`.cfields` per `copypaste-design-reference.html`'s live app
demo), using the shared disclosure-header primitive (`aria-expanded`/`aria-controls`), sharing one
`StatusDot`, one aligned metadata grid, and one danger-button footer pattern whose availability is
governed by the following state table — no device state SHALL render a destructive action that is
invalid for that state:

| Device state | Unpair | Revoke | Notes |
|---|---|---|---|
| Own device | not shown | not shown | no destructive footer |
| Paired peer, online | shown | shown | equal-width danger buttons |
| Paired peer, offline | shown | shown | Unpair is best-effort (peer may not receive notice); Revoke is unconditional; the distinction is surfaced via tooltip/label, not by hiding either action |
| Discovered (unpaired) device | not shown | not shown | only a Pair affordance applies |
| Pending action | disabled + spinner | disabled + spinner | the row's other destructive action is also disabled while one is in flight |
| Failed action | re-enabled + inline error | re-enabled + inline error | no silent retry |

The reference file's `.devcard`/`.dmeta` grid-card variant is documented here as an available
primitive (for a possible future grid layout of `DevicesView`) but is **not** wired into
`DevicesView` by this change and does not ship in the production stylesheet — it is rendered only
in the `preview-gallery` capability's reference section, per that capability's devcard scenario.

#### Scenario: Own device and a paired peer share identical field styling
- **WHEN** the own-device row and any paired-peer row are both expanded
- **THEN** every metadata field (Model/OS/Version/Local IP/Public IP/…) uses the same label/value
  typography and tap-to-copy affordance, differing only in which fields are present

#### Scenario: Transport is shown by chip, never by row color
- **WHEN** a peer's transport is P2P, Relay, or Supabase (cloud)
- **THEN** a `.tpill` chip labeled accordingly is shown next to the device name, and the row's
  background/border does not change based on transport

#### Scenario: Destructive actions are equal-width and require confirmation
- **WHEN** the user clicks Unpair or Revoke on a paired peer's row
- **THEN** a confirm modal (the shared `Dialog` primitive, backed by `.modal`/`.scrim`) names the
  specific device and requires an explicit confirm click before the action proceeds

#### Scenario: No device state renders an invalid destructive action
- **WHEN** a device row is in any of the six documented states (own device, paired-online,
  paired-offline, discovered, pending, failed)
- **THEN** it shows exactly the Unpair/Revoke availability documented in the state table above —
  a discovered device never shows a danger button, and a device with a pending action disables
  both destructive buttons on that row

#### Scenario: A discovery-only device offers no destructive action
- **WHEN** a discovered, not-yet-paired device row is rendered
- **THEN** only a "Pair" affordance is shown; neither Unpair nor Revoke is rendered, since no
  trust relationship exists yet to revoke or unpair

### Requirement: Banners share one component for all four severities
The system SHALL render every conditional banner in the app (accessibility permission,
protocol-version mismatch, stale-daemon, storage/sync validation messages, cloud-account mismatch)
through one `.banner`/`.banner--{ok|info|warn|err}` pattern: leading icon, message text, optional
trailing action button(s), shown only when actionable, dismissible only where safe to ignore.

#### Scenario: Every banner variant uses its designated status token
- **WHEN** a banner of severity `ok`, `info`, `warn`, or `err` is rendered
- **THEN** its icon, text tint, and background tint all derive from the same status token
  (`--ok`/`--info`/`--warn`/`--err`) and no banner hardcodes a color outside that token

#### Scenario: Non-dismissible banners have no close control
- **WHEN** the daemon-spawn-error banner (installation-incomplete, non-dismissible per existing
  `App.tsx` logic) is shown
- **THEN** it renders with no dismiss button, while the protocol-mismatch and stale-daemon banners
  (dismissible per existing logic) each render a Dismiss button

### Requirement: Empty states share one component across every surface
`EmptyState.tsx` SHALL be the only empty/error/offline hero pattern used by History (no items, no
search results), Devices (no paired devices), and the popup — the popup has exactly **four**
documented empty states (offline, starting up, no matches, nothing copied yet), all rendered
through the same `EmptyState` public component API (no separate component or prop shape for
startup/offline versus the other two) — each rendering a centered icon + one-line title + one-line
body + optional action, per `STYLEGUIDE.md` §9.10.

#### Scenario: Empty state adapts message and action per context
- **WHEN** History has zero items vs. zero search results vs. Devices has zero paired devices vs.
  the popup is in any of its four states (offline, starting up, no matches, nothing copied yet)
- **THEN** each context supplies its own title/body/action text through `EmptyState`'s existing
  props, and all render with identical layout/spacing/icon treatment through the same component API

### Requirement: Modal/confirm dialogs share one Dialog primitive with a defined behavior contract
`ConfirmModal.tsx`, `SasPairingModal.tsx`, `RevokeConfirmDialog.tsx`, and `DetailsModal.tsx` SHALL
each compose one shared `Dialog` primitive responsible for: portal-to-`document.body` rendering;
`role="dialog"` + `aria-modal="true"`; caller-supplied `aria-labelledby`/`aria-describedby` wiring;
initial focus on the first focusable descendant (or the container itself as a fallback); a focus
trap cycling Tab/Shift+Tab within the dialog; a configurable Escape-and-backdrop-click dismissal
policy; restoration of focus to the triggering element on close; and scroll-locking the underlying
view while open. The visual `.scrim`/`.modal` styling (centered panel, title at 600 weight, body
text in `--dim`, right-aligned ghost-cancel + primary/danger-confirm actions) is a presentation of
this shared behavioral contract, not a substitute for it. This is not new interaction behavior in
most respects: `useFocusTrap` already implements initial focus, the focus trap, Escape delegation,
and focus restoration, and `ConfirmModal` already composes it — the `Dialog` primitive extracts and
formalizes that existing contract for reuse by the other three components, and adds scroll-lock as
the one genuinely new behavior.

#### Scenario: Escape and backdrop click dismiss non-destructive modals
- **WHEN** a dismissible modal is open and the user presses Escape or clicks the scrim outside the
  modal panel
- **THEN** the modal closes without performing its action, consistent across all four modal
  components

#### Scenario: Focus is trapped inside an open modal
- **WHEN** a modal is open
- **THEN** Tab/Shift+Tab cycle focus only among the modal's own focusable elements until it closes

#### Scenario: Initial focus lands inside the dialog and is restored to the trigger on close
- **WHEN** any of the four Dialog-composed components opens
- **THEN** focus moves to the first focusable element inside the dialog (or the dialog container
  itself if none exists) immediately on open, and returns to the element that had focus
  immediately before the dialog opened once it closes

#### Scenario: The underlying view is scroll-locked while a dialog is open
- **WHEN** any Dialog-composed component is open
- **THEN** the view behind it does not scroll in response to wheel/touch input, and scrolling is
  restored once the dialog closes

### Requirement: Sidebar and Settings surfaces use the shared navigation/row patterns
`Sidebar.tsx` SHALL use the `.sb`/`.sb__item` pattern (active item = accent left-edge + `--text`
color; inactive = `--dim`), and `SettingsView`'s tabs/rows (`TabBar.tsx`, `SettingsRow.tsx`,
`Panel.tsx`, `SliderRow.tsx`, `Toggle.tsx`) SHALL use the shared `.set-tab`/`.srow`/`.set-grp`
patterns so every settings tab (General, Display, Sync, Shortcuts, Storage) looks structurally
identical. `TabBar.tsx` SHALL use native tab semantics: `role="tablist"` on the container,
`role="tab"` + `aria-selected` on each tab, and arrow-key navigation between tabs.

#### Scenario: Active sidebar item is marked by exactly one visual signal set
- **WHEN** a sidebar nav item is the active view
- **THEN** it shows the `--selected` background, `--text` color, and an accent-colored left edge
  bar, and no other sidebar item shows any of those three signals

#### Scenario: Every settings tab shares row anatomy
- **WHEN** any settings tab (General/Display/Sync/Shortcuts/Storage) is viewed
- **THEN** each setting is a `.srow` with a left label(+optional description) and a right-aligned
  control (toggle, segmented control, slider, or button), separated from adjacent rows by a single
  hairline divider — never a bordered box

#### Scenario: Settings tabs support arrow-key navigation
- **WHEN** a settings tab has keyboard focus
- **THEN** the Left/Right (or Up/Down, matching the tab list's orientation) arrow keys move focus
  and selection between tabs, and `aria-selected` reflects the active tab

### Requirement: Popup surface reuses the same primitives as the main window
The system SHALL style the quick-paste popup (`Popup.tsx`, `PopupRow.tsx`, `GlideHighlight.tsx`,
`HighlightedText.tsx`) from the same token set and the same row/tile/empty-state/kbd primitives as
the main window — not a separate visual language — per `STYLEGUIDE.md` §9.13.

#### Scenario: Popup keycap hints use the shared `.kbd` primitive
- **WHEN** the popup footer shows navigation/paste/close hints or a numbered row shows a
  Cmd+1–9 keycap
- **THEN** each hint renders via the same `.kbd` class used nowhere else but consistently across
  every keycap in the popup

#### Scenario: Selection highlight glides smoothly and respects reduced motion
- **WHEN** the user moves keyboard/mouse selection between popup rows
- **THEN** `GlideHighlight`'s overlay animates its position over `--dur` using `--ease`, and
  performs no animation when `prefers-reduced-motion: reduce` is set

### Requirement: Icons are restored with explicit fixed sizing
Every icon reintroduced into a stripped component SHALL be a `lucide-react` icon (or, where no
suitable Lucide icon exists, an inline SVG matching the reference file's stroke-width/viewBox
convention) rendered with an explicit width/height (via CSS or component props) — never an
unsized inline SVG that can inherit an unbounded intrinsic size.

#### Scenario: No icon renders at intrinsic size
- **WHEN** any `<svg>` is rendered anywhere in `copypaste-ui`
- **THEN** it has an explicit `width`/`height` (or an ancestor CSS rule fixing its box) rather than
  relying on the SVG's own `viewBox`-derived intrinsic dimensions

### Requirement: Accessible behavior is preserved via observable contracts, not literal attribute placement
The system SHALL preserve accessible behavior through observable contracts — correct accessible
role and name, resolved labelled-by/described-by relationships, accurately exposed state
(`aria-expanded`, `aria-selected`, etc.), keyboard behavior unchanged or improved, and stable
`data-testid`s only where they are an intentional test contract — rather than requiring every
`role`/`id`/`aria-*` attribute to remain on the literal same DOM element. This SHALL NOT block a
necessary semantic correction, such as fixing the accessible-name leak on masked sensitive content
(see the clipboard history row requirement's sensitive-content scenarios).

#### Scenario: A semantic correction is allowed even if it moves an attribute to a different element
- **WHEN** restyling requires moving a `role` or `aria-*` attribute to a different (but
  semantically equivalent or more correct) element to fix an existing accessibility defect
- **THEN** this is permitted, provided the resulting accessible role/name/state/labelling remains
  correct and keyboard behavior is unchanged or improved

#### Scenario: Stable test IDs are preserved only where they are an intentional contract
- **WHEN** the existing test suite queries by `data-testid`
- **THEN** every `data-testid` that is documented as an intentional selector contract continues to
  resolve to an equivalent element; a `data-testid` present only incidentally is not treated as a
  frozen contract

### Requirement: Focus-visible indicators meet contrast, clipping, and forced-colors requirements
Every interactive element (button, toggle, chip, tab, row, field) SHALL show a focus-visible ring
on keyboard focus with at least 3:1 contrast against every adjacent surface it can appear against,
never clipped by an ancestor's `overflow: hidden` in rows, modals, tabs, or the popup, with a
`forced-colors` fallback where the target WebView supports that media feature, and with focus
restored to a well-defined element after any modal closes.

#### Scenario: Focus-visible ring appears on every interactive element with sufficient contrast
- **WHEN** any interactive element (button, toggle, chip, tab, row, field) receives keyboard focus
- **THEN** a focus ring using the `--focus-ring-width`/`--focus-ring-offset` tokens is visible with
  at least 3:1 contrast against the surface behind it, and it is never suppressed without an
  equivalent replacement

#### Scenario: Focus ring is never clipped
- **WHEN** a focusable element near the edge of a scrollable row, modal, tab strip, or the popup
  receives focus
- **THEN** its focus ring is fully visible, not clipped by an ancestor's `overflow: hidden`

#### Scenario: Forced-colors mode still shows a visible focus indicator
- **WHEN** the target WebView reports a `forced-colors` (or equivalent high-contrast) mode
- **THEN** the focus indicator remains visible using system/forced colors rather than disappearing

#### Scenario: Automated tests referencing existing selectors keep passing
- **WHEN** the existing test suite (unit tests referencing `role`/`aria-label`/`data-testid`
  selectors) runs against the restyled components
- **THEN** all such selector-based queries continue to resolve to an equivalent element as before
  this change, per the observable-contract requirement above

### Requirement: Zoom, text scaling, and reduced motion are verified per component
The system SHALL support 200% browser zoom and OS-level text scaling without requiring
two-dimensional scrolling for primary content, SHALL expose `aria-expanded`/`aria-controls` on
every device-row disclosure header, SHALL provide `aria-live` regions for toast and status-banner
content, and SHALL verify `prefers-reduced-motion: reduce` disables every animation in the app —
not only the three named duration tokens — including native smooth scrolling and any
keyframe/transform-based animation.

#### Scenario: 200% zoom reflows every primary surface
- **WHEN** the browser zoom level is 200%
- **THEN** History, Devices, Settings, and the popup reflow their content without requiring
  horizontal scrolling to read primary content

#### Scenario: Toast and banner content is announced via a live region
- **WHEN** a toast or status banner appears
- **THEN** it is exposed via an `aria-live` region so assistive technology announces it without
  requiring focus to move to it

#### Scenario: Every animation, not only duration-token-gated ones, respects reduced motion
- **WHEN** `prefers-reduced-motion: reduce` is set
- **THEN** row insert/remove, toast, spinner, selection-glide, presence-pulse, native smooth
  scrolling, and any keyframe animation not gated by a duration token all fail to visibly animate
