## ADDED Requirements

### Requirement: Preview Peeking and Pinned phases
The Preview SHALL preserve its two existing phases: **Peeking** — a centered card over a `scrim` with
the History list still present behind it — and **Pinned** — the expanded interaction mode reached by
dragging up (`previewPeekGesture`, ~64dp commit). The redesign SHALL preserve drag-up-to-pin, swipe/
back dismissal, and their gesture arbitration; it SHALL NOT silently change these behaviours.

#### Scenario: Peeking keeps the list behind the scrim
- **WHEN** the user peeks a clip from the History list
- **THEN** a centered card renders over a `scrim` and the list remains visually present behind it

#### Scenario: Drag up pins; swipe/back dismisses
- **WHEN** the user drags the peek card past the commit threshold, or swipes down / presses back
- **THEN** the Preview transitions to Pinned, or dismisses back to the list at its prior scroll position

#### Scenario: Actions toolbar availability
- **WHEN** the Preview displays a non-sensitive item
- **THEN** the toolbar exposes Copy, Pin/Unpin, Delete, and — for file/path items — conditional Open and Save
- **AND** when the item is sensitive, non-plaintext actions (Reveal, Pin, Delete) are available but
  plaintext is not exposed until an explicit reveal

### Requirement: Preview Reveal (NEW)
The Preview SHALL introduce a Reveal action — new functionality, not a preserved behaviour: today
`PreviewActionRow` unconditionally renders Copy/Pin/Open/Save/Delete with no Reveal control, and
`PreviewContent` hardcodes `masked = computeMasked(..., revealed = false)`, so a sensitive item in
Preview is permanently masked with no in-Preview way to reveal it. This change adds a Reveal action to
`PreviewActionRow` and wires a `revealed` state through `PreviewOverlay`/`PreviewContent`/
`PreviewImageContent`, mirroring `HistoryRow`'s existing `revealed by remember(item.id)` pattern.

#### Scenario: Reveal action unmasks a sensitive item in Preview
- **WHEN** the user taps Reveal on a sensitive item's Preview toolbar
- **THEN** a `revealed` state (keyed by `remember(item.id)`, mirroring `HistoryRow`) flips true and the
  Preview re-renders the item's plaintext content, replacing the masked representation

#### Scenario: Reveal state resets per item
- **WHEN** the user navigates from one sensitive item's Preview to a different sensitive item
- **THEN** the new item's `revealed` state starts false (masked), independent of the previous item's
  reveal state

### Requirement: Content Rendering by Kind
The Preview SHALL render each clip's content using the same shared content-type color source as
the History list, and SHALL apply the same mono-vs-sans typography rule per kind.

#### Scenario: Color parity with the list
- **WHEN** the Preview renders the content-type accent (e.g. header tint, type label) for a given
  kind
- **THEN** the color is resolved from the same shared source used by the History list for that
  kind
- **AND** it matches the color shown for the same item in the list

#### Scenario: Monospace kinds
- **WHEN** the previewed item's kind is `code`, `url`, `path`, `json`, `number`, `color`, or
  `secret`
- **THEN** the content body is rendered in the monospace type role

#### Scenario: Sans-serif kinds
- **WHEN** the previewed item's kind is `text` or `email`
- **THEN** the content body is rendered in the sans (Inter) type role

### Requirement: Image Preview Loading States
The Preview SHALL present distinct loading, success, and failure states when rendering image
content.

#### Scenario: Image loading
- **WHEN** an `IMAGE` item's full-resolution content is being fetched or decoded
- **THEN** a loading indicator is shown in place of the image

#### Scenario: Image load success
- **WHEN** an `IMAGE` item's content finishes decoding successfully
- **THEN** the decoded image replaces the loading indicator and is displayed at a size that
  respects the screen bounds

#### Scenario: Image load failure
- **WHEN** an `IMAGE` item's content fails to load or decode
- **THEN** a failure state is shown with an explanatory message
- **AND** the failure state does not silently fall back to a blank or broken-image render

### Requirement: File Open and Save Failure Handling
The Preview SHALL surface explicit failure feedback when opening or saving file-backed content
fails.

#### Scenario: Open failure
- **WHEN** the user requests to open a `FILE`/`PATH` item's underlying content and the operation
  fails
- **THEN** the Preview shows an explicit error message describing the failure
- **AND** the user remains on the Preview surface rather than being silently dropped

#### Scenario: Save failure
- **WHEN** the user requests to save previewed content to disk and the operation fails
- **THEN** the Preview shows an explicit error message describing the failure
- **AND** no partial or corrupt file is presented to the user as if it succeeded

### Requirement: Large Content Handling
The Preview SHALL remain usable and responsive when rendering content that is unusually large.

#### Scenario: Large text/code content
- **WHEN** the previewed item's text content exceeds a size where naive full rendering would
  degrade scroll performance
- **THEN** the Preview renders the content in a scrollable, performant manner (e.g. lazily)
  rather than blocking the UI thread

#### Scenario: Large image content
- **WHEN** the previewed item is an oversized image
- **THEN** the Preview downsamples or constrains the rendered bitmap to avoid an out-of-memory
  failure
- **AND** panning/zooming of the large image remains responsive

### Requirement: Preview Gestures
The Preview SHALL support gesture-based dismissal and navigation consistent with the app's
full-screen surfaces.

#### Scenario: Swipe to dismiss
- **WHEN** the user performs a swipe-down (or equivalent back) gesture on the Preview
- **THEN** the Preview is dismissed and the user returns to the History list at the same scroll
  position

### Requirement: Preview Masking Parity
The Preview SHALL apply the identical masking contract used by the History list, closing the
current gap where `PreviewTextContent` and `PreviewImageContent` omit `clearAndSetSemantics` for
masked content.

#### Scenario: Masked text preview hides plaintext from semantics
- **WHEN** the Preview renders a sensitive text item in its masked state
- **THEN** the masked text node is excluded from the accessibility tree via
  `clearAndSetSemantics`, matching the List's masking behavior
- **AND** no merged or unmerged semantics node exposes the underlying plaintext

#### Scenario: Masked image preview hides plaintext from semantics
- **WHEN** the Preview renders a sensitive image item in its masked state
- **THEN** the masked image node is excluded from the accessibility tree via
  `clearAndSetSemantics`
- **AND** any content description is a placeholder that never contains the underlying plaintext

#### Scenario: No leak in fixtures or logs
- **WHEN** masked Preview content is captured in test goldens or written to logs
- **THEN** synthetic placeholder content is used
- **AND** the underlying plaintext never appears in golden fixtures or log output
