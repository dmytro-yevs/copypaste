## ADDED Requirements

### Requirement: Shared Content-Type Color Source
The app SHALL derive every content-type color from one shared source (per STYLEGUIDE §3.7)
consumed identically by the History list and the full-screen Preview, eliminating any divergence
between the two surfaces for the same clip kind.

#### Scenario: List and Preview agree on color
- **WHEN** a clip of a given `kind` is rendered in both the History list and the full-screen
  Preview
- **THEN** both surfaces resolve the content-type color from the same shared token source
- **AND** the rendered color is identical between the two surfaces for that kind

#### Scenario: Twelve kinds map onto ten colors with defined aliases
- **WHEN** the shared color source is queried for any of the twelve content kinds resolved via
  `ContentVisualKind` (TEXT, URL, EMAIL, PHONE, CODE, JSON, NUMBER, COLOR, PATH, FILE, IMAGE, SECRET)
- **THEN** it returns one of the ten `c-*` content-type colors defined in STYLEGUIDE §3.7
- **AND** PHONE resolves to `cNum` (aliasing NUMBER) and PATH resolves to `cFile` (aliasing FILE);
  there is no distinct `cPath` token

### Requirement: Content-Type Tile Rendering
Each list row SHALL render a content-type tile whose treatment follows STYLEGUIDE §9.4/§3.7: a
tinted background at 14% of the content color with a full-strength glyph, except for `COLOR` and
`IMAGE`, which render their own content in place of a glyph.

#### Scenario: Standard kind tile
- **WHEN** a row's kind is any of `TEXT`, `URL`, `EMAIL`, `PHONE`, `CODE`, `JSON`, `NUMBER`,
  `PATH`, `FILE`
- **THEN** the tile background renders the kind's content color at 14% opacity
- **AND** the tile glyph renders the matching Lucide icon at full content-color strength

#### Scenario: COLOR kind renders the swatch
- **WHEN** a row's kind is `COLOR`
- **THEN** the tile renders the actual parsed color swatch instead of a glyph

#### Scenario: IMAGE kind renders the thumbnail
- **WHEN** a row's kind is `IMAGE`
- **THEN** the tile renders the actual image thumbnail instead of a glyph

#### Scenario: SECRET kind shows a lock glyph
- **WHEN** a row's `is_sensitive` flag is true (kind `SECRET`)
- **THEN** the tile renders a lock glyph in the `c-secret` color
- **AND** the tile background renders `c-secret` at 14% opacity

### Requirement: List Row Anatomy
Each row SHALL follow the STYLEGUIDE §9.5 anatomy: tile, single-line ellipsized preview text, a
meta line, an optional pin indicator, and actions revealed on interaction.

#### Scenario: Preview text typography by kind
- **WHEN** a row's kind is one of `code`, `url`, `path`, `json`, `number`, `color`, `secret`
- **THEN** the preview text is rendered in the monospace type role
- **AND** when the kind is `text` or `email`, the preview text is rendered in the sans (Inter)
  type role instead

#### Scenario: Meta line composition
- **WHEN** a row is rendered
- **THEN** the meta line shows kind, source app, relative time, and origin device in that order
- **AND** the kind word within the meta line is tinted with the row's content-type color
- **AND** origin device is shown only when the item did not originate on this device

#### Scenario: Pinned row indicator
- **WHEN** a row's item is pinned
- **THEN** a fixed 13px star glyph in `--c-color` is shown in the pin position
- **AND** pinned rows are grouped above the `Today` date group

#### Scenario: Row interaction states
- **WHEN** a row is hovered, pressed, or selected
- **THEN** hover applies the `--hover` overlay, pressed applies `--pressed`, and selected applies
  the `--selected` tint plus an accent left-edge indicator

#### Scenario: Actions reveal
- **WHEN** a non-sensitive row is hovered (or long-pressed/swiped on touch)
- **THEN** Pin and Delete actions become visible, with Delete tinted `--err`
- **AND** when the row is sensitive, a Reveal affordance is shown in place of the plain preview
  interaction

### Requirement: Date Group Headers
The list SHALL group rows under sticky date headers per STYLEGUIDE §9.6.

#### Scenario: Header rendering
- **WHEN** the list renders a date group boundary
- **THEN** the header text is uppercase mono, 10px, `--faint`, with `letter-spacing .1em`, and no
  background slab
- **AND** the header is one of `PINNED`, `TODAY`, `YESTERDAY`, or `EARLIER`

#### Scenario: Header stickiness
- **WHEN** the user scrolls the list
- **THEN** the current date group header remains pinned to the top of the list surface until the
  next group scrolls into place

### Requirement: Device Filter Chips
The list SHALL offer a device filter rendered as pill-shaped chips, letting the user restrict the
list to items originating from a specific device.

#### Scenario: Filter chip selection
- **WHEN** the user taps a device filter chip
- **THEN** the chip's selected visual state is applied (per STYLEGUIDE §9.4 pill styling)
- **AND** the list is filtered to items whose origin device matches the selected chip

#### Scenario: Clearing the filter
- **WHEN** the user deselects the active device filter chip (or selects an "All" chip)
- **THEN** the list returns to showing items from every device

### Requirement: Selection and Multi-Select
The list SHALL support entering a multi-select mode in which rows show a checkbox in place of the
content-type tile and support bulk actions.

#### Scenario: Entering multi-select
- **WHEN** the user long-presses a row (or activates multi-select through an equivalent
  affordance)
- **THEN** the list enters multi-select mode and the interacted row is marked selected
- **AND** every visible row's tile position is replaced by a checkbox

#### Scenario: Bulk copy excludes sensitive content
- **WHEN** the user performs a bulk-copy action while one or more selected rows are sensitive
  (masked)
- **THEN** the sensitive rows' plaintext content is excluded from the copied payload
- **AND** only non-sensitive selected rows contribute their content to the bulk copy

### Requirement: List Display States
The list SHALL present a distinct, correctly-triggered UI for each of its defined states:
loading, populated, empty, empty-in-private-mode, no-search-results, and error/degraded.

#### Scenario: Loading state
- **WHEN** the list's underlying data has not yet been fetched
- **THEN** a loading state is shown instead of an empty or populated list

#### Scenario: Empty state (normal)
- **WHEN** the list has finished loading and contains zero clipboard items
- **THEN** the STYLEGUIDE §9.10 empty state is shown: a centered line icon, one-line headline,
  one-line hint

#### Scenario: Empty state (private mode)
- **WHEN** the list has finished loading, contains zero visible items, and the app is in private
  mode
- **THEN** a private-mode-specific empty state is shown, distinct in copy from the normal empty
  state

#### Scenario: No search results
- **WHEN** an active search query matches zero items
- **THEN** a no-search-results empty state is shown instead of the normal empty state
- **AND** the state's copy references the active query

#### Scenario: Error / degraded state persists
- **WHEN** the list's data source is unavailable or returns an error
- **THEN** a persistent error/degraded state is shown in the list surface itself
- **AND** the error is not communicated solely via a transient toast

### Requirement: List Masking Contract
Sensitive rows in the list SHALL be masked according to the STYLEGUIDE masking contract, and
masked content SHALL never be exposed through accessibility semantics, logs, or bulk operations.

#### Scenario: Masked rendering preserves geometry at every API level
- **WHEN** a row is sensitive and the running device is API 31 or higher
- **THEN** its preview text is rendered with a blur mask that keeps the real text width and the reveal affordance
- **AND** when the running device is below API 31, a single defined native fallback is used: a
  geometry-preserving **opaque overlay** drawn OVER a sanitized/offscreen-safe representation (never
  plaintext underneath), keeping the real width and the reveal affordance — NOT bullet substitution
  and NOT pixelating live plaintext
- **AND** neither path renders or exposes plaintext in the display list, semantics tree (merged +
  unmerged), logs, recents, or fixtures — including partial spans (recorded Native adaptation in
  `cross-platform-parity.md`); a layout-stability + no-plaintext test covers before/after reveal

#### Scenario: Masked text hidden from semantics
- **WHEN** a sensitive row is masked
- **THEN** the row's masked text node is excluded from the accessibility tree via
  `clearAndSetSemantics`
- **AND** any content description exposed for the row is a placeholder that never contains the
  underlying plaintext

#### Scenario: Partial span masking is visually and semantically safe
- **WHEN** only part of a row's content is classified sensitive (span-level masking)
- **THEN** only the sensitive spans are masked while non-sensitive spans remain visible
- **AND** the row exposes a sanitized accessibility string where the sensitive spans are replaced by a
  localized placeholder, so the plaintext of a masked span never appears in the merged OR unmerged
  semantics tree (covered by a dedicated span-masking semantics test with a synthetic mixed string)
