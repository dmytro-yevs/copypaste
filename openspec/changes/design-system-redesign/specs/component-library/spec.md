## ADDED Requirements

### Requirement: Single, ITCSS-layered stylesheet
The system SHALL provide exactly one authored stylesheet, `crates/copypaste-ui/src/index.css`,
imported by both the main window entry point and the popup entry point, organized in ITCSS order
(tokens → base → primitives → patterns → app-shell → utilities). No Tailwind utility classes, CSS-
in-JS, CSS Modules, or additional hand-authored stylesheet files SHALL be introduced.

#### Scenario: One stylesheet serves both windows
- **WHEN** the main window (`index.html`) and the quick-paste popup window (`popup.html`) each
  load
- **THEN** both import `src/index.css` (directly or via their respective `main.tsx` entry
  modules) and no other project-authored CSS file

#### Scenario: No Tailwind or CSS-in-JS remains
- **WHEN** the `copypaste-ui` source tree is searched for Tailwind utility classes, a
  `tailwind.config.*` file, or CSS-in-JS usage (`styled-components`, emotion, inline `style={{}}`
  used for anything other than dynamically computed per-instance values already required by
  existing logic, e.g. `--ct` custom-property wiring)
- **THEN** none are found

### Requirement: DRY primitive components — one class per repeated pattern
Any visual element occurring in two or more places in the app SHALL be implemented as exactly one
CSS class (or one shared React component wrapping that class), never duplicated per call site.
At minimum this covers: buttons (`.btn` + `--primary`/`--secondary`/`--ghost`/`--danger`
variants, `.sm` size, `disabled` state), icon buttons (`.iconbtn` incl. `.danger`), the toggle
switch (`.toggle` / `.off`), the segmented control (`.seg`), the search/text field (`.field`),
chips (`.chip` / `.chip.on` / `.chip--ct`), transport/identity pills (`.tpill--p2p` /
`.tpill--cloud` / `.tpill--this`), the verified badge (`.badge--verified`) and count badge
(`.badge--count`), the content-type tile (`.tile`, `.tile--swatch`, `.tile--thumb`), the keycap
(`.kbd`), and the presence dot (`.dot-stat` / `.off`).

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

### Requirement: Clipboard history row covers every content kind with one component
`HistoryRow.tsx` and `PopupRow.tsx` SHALL render every clipboard content kind (`text`, `url`,
`email`, `phone`, `code`, `json`, `number`, `color`, `path`/`file`, `image`, `secret`) through a
single shared row anatomy — tile, single-line ellipsized preview, meta line
(`kind · sourceApp · relTime · originDevice`), pin affordance, and hover-revealed actions — with
only the content-type token and tile content (glyph vs. swatch vs. thumbnail) varying by kind.

#### Scenario: Sensitive content is masked, not hidden
- **WHEN** a `secret`-kind row is rendered and masking is enabled
- **THEN** the preview text is visually masked (blurred/redacted) but occupies its real width, and
  clicking/tapping the masked span reveals the underlying text (optionally behind a warning per
  existing `showSensitiveWarnings` preference), matching `STYLEGUIDE.md` §7 "masked by blur, never
  deletion"

#### Scenario: Pin and delete actions are reachable via hover and keyboard
- **WHEN** a row is hovered (desktop) or receives keyboard focus
- **THEN** its pin and delete affordances become visible/focusable, each a real `<button>` with an
  `aria-label` naming the action and the item, not a click-only `<div>`

#### Scenario: Multi-select replaces the tile with a checkbox
- **WHEN** History enters selection mode
- **THEN** every row's tile position is replaced by a checkbox control, and per-row hover actions
  (pin/delete) are hidden while selection-mode actions (bulk pin/delete) are shown in the bulk
  action bar instead

### Requirement: Device list uses one expandable row component for both own device and peers
`DeviceCard.tsx`'s `ThisDeviceCard` and `PeerRow` SHALL render through the same expandable
row/field-grid pattern (`.devrow`/`.cfields` per `copypaste-design-reference.html`'s live app
demo), sharing one `StatusDot`, one aligned metadata grid, and one danger-button footer pattern —
own device renders no action footer; a paired peer renders Unpair + Revoke as equal-width danger
buttons.

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
- **THEN** a confirm modal (`.modal` over `.scrim`) names the specific device and requires an
  explicit confirm click before the action proceeds

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
search results), Devices (no paired devices), and the popup (offline, starting up, no matches,
nothing copied yet), each rendering a centered icon + one-line title + one-line body + optional
action, per `STYLEGUIDE.md` §9.10.

#### Scenario: Empty state adapts message and action per context
- **WHEN** History has zero items vs. zero search results vs. Devices has zero paired devices vs.
  the popup is offline
- **THEN** each context supplies its own title/body/action text through `EmptyState`'s existing
  props, and all four render with identical layout/spacing/icon treatment

### Requirement: Modal/confirm dialogs share one pattern
`ConfirmModal.tsx`, `SasPairingModal.tsx`, and `RevokeConfirmDialog.tsx` SHALL each render through
the same `.scrim`/`.modal` pattern — centered panel, title (600 weight), body text (`--dim`),
right-aligned actions (ghost cancel + primary/danger confirm) — with destructive confirms naming
the specific device/item and using the `danger` button variant.

#### Scenario: Escape and backdrop click dismiss non-destructive modals
- **WHEN** a dismissible modal is open and the user presses Escape or clicks the scrim outside the
  modal panel
- **THEN** the modal closes without performing its action, consistent across all three modal
  components

#### Scenario: Focus is trapped inside an open modal
- **WHEN** a modal is open
- **THEN** Tab/Shift+Tab cycle focus only among the modal's own focusable elements until it closes

### Requirement: Sidebar and Settings surfaces use the shared navigation/row patterns
`Sidebar.tsx` SHALL use the `.sb`/`.sb__item` pattern (active item = accent left-edge + `--text`
color; inactive = `--dim`), and `SettingsView`'s tabs/rows (`TabBar.tsx`, `SettingsRow.tsx`,
`Panel.tsx`, `SliderRow.tsx`, `Toggle.tsx`) SHALL use the shared `.set-tab`/`.srow`/`.set-grp`
patterns so every settings tab (General, Display, Sync, Shortcuts, Storage) looks structurally
identical.

#### Scenario: Active sidebar item is marked by exactly one visual signal set
- **WHEN** a sidebar nav item is the active view
- **THEN** it shows the `--selected` background, `--text` color, and an accent-colored left edge
  bar, and no other sidebar item shows any of those three signals

#### Scenario: Every settings tab shares row anatomy
- **WHEN** any settings tab (General/Display/Sync/Shortcuts/Storage) is viewed
- **THEN** each setting is a `.srow` with a left label(+optional description) and a right-aligned
  control (toggle, segmented control, slider, or button), separated from adjacent rows by a single
  hairline divider — never a bordered box

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

### Requirement: All existing accessibility attributes are preserved
The system SHALL preserve every `role`, `id`, `aria-*`, and `data-testid` attribute present on a
component before this change, on the same element, after restyling — restyling SHALL only add
`className`/CSS, never remove or relocate accessibility wiring.

#### Scenario: Focus-visible ring appears on every interactive element
- **WHEN** any interactive element (button, toggle, chip, tab, row, field) receives keyboard focus
- **THEN** a `2px solid var(--accent)` focus ring with `2px` offset is visible, and it is never
  suppressed without an equivalent replacement

#### Scenario: Automated tests referencing existing selectors keep passing
- **WHEN** the existing test suite (unit tests referencing `role`/`aria-label`/`data-testid`
  selectors) runs against the restyled components
- **THEN** all such selector-based queries continue to resolve to the same elements as before this
  change
