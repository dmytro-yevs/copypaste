## ADDED Requirements

### Requirement: Gallery is reachable at the existing `?mock=1` preview URL
The system SHALL expose a component preview gallery as a view of the existing main window, reached
by running the existing dev server (`localhost:1420`) with the existing `?mock=1` query parameter
already used to activate mock IPC — with no new Vite entry point, no new dev server port, and no
new build target.

#### Scenario: Gallery nav item appears only in mock mode
- **WHEN** the app is running in a browser with `?mock=1` in the URL (dev build)
- **THEN** the sidebar shows a "Gallery" navigation item that, when clicked, renders the gallery
  view within the same window/session

#### Scenario: Gallery nav item is absent outside mock mode
- **WHEN** the app is running as the packaged Tauri app, or in a browser without `?mock=1`/
  `?bridge=1`, or in a production build
- **THEN** no "Gallery" navigation item is rendered and no gallery code path is reachable

#### Scenario: Gallery is excluded from production bundles
- **WHEN** the production build (`vite build`, `import.meta.env.DEV === false`) is inspected
- **THEN** the gallery view module is not present in the production JS bundle, exactly like the
  existing `mockIpc.ts` dead-code-elimination guarantee

### Requirement: Gallery renders every primitive and pattern component
The gallery SHALL render at least one live example of every shared primitive/pattern defined by
the `component-library` capability: buttons (all variants × sizes × disabled), icon buttons,
toggle, segmented control, field/search input, chips/badges/pills, content-type tiles (all 11
kinds), the clip history row (one example per content kind), the device row (own-device and
peer variants, expanded and collapsed), banners (all 4 severities), the modal/confirm pattern,
empty states (all documented variants), the sidebar, settings tabs/rows, and the popup row/keycap/
glide-highlight primitives.

#### Scenario: Every content-type kind has a gallery example
- **WHEN** the gallery's "History row" section is viewed
- **THEN** it shows one row per content kind (`text`, `url`, `email`, `phone`, `code`, `json`,
  `number`, `color`, `path`/`file`, `image`, `secret`), each with realistic sample data

#### Scenario: Every button/state combination is present
- **WHEN** the gallery's "Buttons" section is viewed
- **THEN** primary/secondary/ghost/danger variants are each shown in default, hover-capable,
  disabled, and (for `ActionButton`) pending states

### Requirement: Gallery covers long-text and empty-content states
The gallery SHALL include, for every component whose layout depends on content length (row titles,
meta lines, device names, banner messages), at least one example with unusually long text
(overflow/ellipsis/wrap check) alongside a normal-length example, and at least one example of each
documented empty state (no history, no search results, no paired devices, popup offline/starting
up/nothing copied yet).

#### Scenario: Long text does not break row layout
- **WHEN** the gallery renders a history row whose preview text is significantly longer than the
  row width
- **THEN** the preview text is truncated with an ellipsis and the row height does not grow beyond
  its single-line spec

### Requirement: Gallery renders every theme × accent combination
The gallery SHALL provide a way to view its full component set under all 12 theme×accent
combinations (2 themes × 6 accents) — either by rendering repeated sections per combination or by
an in-gallery control that switches the live `data-theme`/`data-accent` without leaving the view —
so every combination can be visually verified without manually editing preferences 12 times.

#### Scenario: Switching theme/accent inside the gallery updates every rendered example
- **WHEN** the user changes the gallery's theme or accent control
- **THEN** every component example currently rendered in the gallery updates to reflect the new
  theme/accent, without navigating away from the gallery view

### Requirement: Browsing the gallery does not alter the user's persisted preferences
Interacting with the gallery's theme/accent preview control SHALL NOT write to the same persisted
`UIPrefs.theme`/`UIPrefs.accent` fields the rest of the app reads, so that leaving the gallery
restores the user's actual saved theme/accent.

#### Scenario: Leaving the gallery restores the real preference
- **WHEN** the user's saved preference is `dark`/`indigo`, they open the gallery, switch its
  preview control through several theme/accent combinations, and then navigate to another view
- **THEN** the main window returns to `data-theme="dark"`/`data-accent="indigo"` and `UIPrefs` in
  the persisted store still reads `dark`/`indigo`
