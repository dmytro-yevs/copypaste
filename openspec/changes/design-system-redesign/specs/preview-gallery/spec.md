## ADDED Requirements

### Requirement: Gallery is reachable at the existing `?mock=1` preview URL, gated by `MOCK`, not by bridge mode
The system SHALL expose a component preview gallery as a view of the existing main window, reached
by running the existing dev server (`localhost:1420`) with the existing `?mock=1` query parameter
(or `VITE_MOCK=1`) already used to activate the `MOCK` flag exported by `lib/ipc/transport.ts` —
with no new Vite entry point, no new dev server port, and no new build target. The gallery is gated
on `import.meta.env.DEV && MOCK` exactly — it is NOT reachable in `?bridge=1` (live-daemon) mode,
since `MOCK` is false whenever the app is talking to a real daemon bridge, and all project
documentation SHALL state this consistently.

#### Scenario: Gallery nav item appears only when DEV and MOCK are both true
- **WHEN** the app is running in a dev build in a browser with `?mock=1` (or `VITE_MOCK=1`) set
- **THEN** the sidebar shows a "Gallery" navigation item that, when clicked, renders the gallery
  view within the same window/session

#### Scenario: Gallery nav item is absent in bridge mode and outside mock mode
- **WHEN** the app is running as the packaged Tauri app, in a browser with `?bridge=1` (live
  daemon, `MOCK` is false), in a browser with neither `?mock=1` nor `?bridge=1`, or in a
  production build
- **THEN** no "Gallery" navigation item is rendered and no gallery code path is reachable

#### Scenario: Gallery is not a reachable entry in the production ViewId type
The production view registry SHALL be typed and implemented against `ProductionViewId` (excluding
`"gallery"`). The store's `view` field is **in-memory only (NOT persisted)** and keeps the production
union type; the gallery SHALL NOT be added to it and no `DevViewId` SHALL be introduced into the
store. The gallery is a **dev-only navigation branch** (a `DEV && MOCK`-gated local flag or a
`?view=gallery` URL check) handled OUTSIDE the production view registry via the DEV-gated
dynamic-import branch described below.

- **WHEN** `App.tsx`'s view registry (the `Record<ProductionViewId, …>` mapping view IDs to
  components) is inspected
- **THEN** it has no `"gallery"` entry; the gallery is never a statically-registered production view

#### Scenario: Gallery is loaded through a DEV-gated dynamic import, not a static import
- **WHEN** `import.meta.env.DEV && MOCK` is true and the current view is `"gallery"`
- **THEN** `App.tsx` loads the gallery module via a dynamic `import()` call — mirroring
  `lib/ipc/transport.ts`'s existing `await import("../mockIpc")` pattern — rather than a static
  top-level import, so the gallery module is absent from the production dependency graph

#### Scenario: An invalid in-memory/URL `view` value narrows to a valid view
- **WHEN** a `view` value of `"gallery"` (or any unrecognized value) arrives in a production context
  from code or a `?view=` URL param — note `view` is NOT persisted, so this is defensive input
  narrowing, not recovery of a stored downgrade state
- **THEN** the app treats it as unknown in production and falls back to `"history"`, rather than
  attempting to render a module that was never bundled

#### Scenario: Gallery exclusion from production is verified by both a string check and a chunk-graph check
- **WHEN** the production build (`vite build`, `import.meta.env.DEV === false`) is inspected
- **THEN** the gallery view module's unique string content is absent from the emitted `dist/`
  output (existing check), AND the emitted Rollup chunk graph/manifest shows no
  production-entry-reachable chunk containing the gallery module's file path (additional check —
  a unique-string grep alone does not prove reachability is severed)

### Requirement: Gallery is structured as canonical sections, a local switcher, and a compact critical-combination matrix
The gallery SHALL render at least one live example of every shared primitive/pattern defined by
the `component-library` capability — buttons (all variants × sizes × disabled), icon buttons,
toggle, segmented control, field/search input, chips/badges/pills, content-type tiles (all 11
named kinds plus an explicit `unknown`-kind example), the clip history row (one example per
content kind plus unknown), the device row (own-device and peer variants, expanded and collapsed,
one example per documented device state), banners (all 4 severities), the modal/confirm pattern,
empty states (all 4 documented variants), the sidebar, settings tabs/rows, and the popup
row/keycap/glide-highlight primitives — organized as (a) canonical component/state sections, each
with a deterministic `id` for automated deep-linking, (b) a local theme/accent/translucency
switcher affecting the whole gallery, and (c) a separate, compact "token/critical-component matrix"
section rendering the full 12 theme×accent combinations only for a small critical-component subset
(button, card, focus ring, status banner) — the gallery SHALL NOT render twelve complete
interactive application copies simultaneously to achieve combination coverage.

#### Scenario: Every content-type kind, including unknown, has a gallery example
- **WHEN** the gallery's "History row" section is viewed
- **THEN** it shows one row per content kind (`text`, `url`, `email`, `phone`, `code`, `json`,
  `number`, `color`, `path`/`file`, `image`, `secret`) plus one row demonstrating the `unknown`-kind
  fallback, each with realistic sample data, and each annotated with whether that kind is
  currently emitted by the real daemon or is gallery/design-canvas-only

#### Scenario: Every button/state combination is present, using forced-state helpers where needed
- **WHEN** the gallery's "Buttons" section is viewed
- **THEN** primary/secondary/ghost/danger variants are each shown in default, disabled, and (for
  `ActionButton`) pending states via static examples, and hover/active/focus-visible states are
  demonstrated either via a debug-only forced-state attribute (with a CSS parity test confirming
  it matches the real pseudo-class's computed style) or via a Playwright interaction screenshot —
  not asserted as merely "hover-capable" with no defined expected visual result

#### Scenario: Each gallery section has a deterministic ID for automated navigation
- **WHEN** any canonical gallery section (buttons, history row, device row, banners, modal, empty
  states, etc.) is rendered
- **THEN** it has a stable, deterministic DOM `id` (e.g. `gallery-buttons`, `gallery-history-row`)
  that automated tests can navigate to directly without depending on visual position

#### Scenario: The devcard grid variant appears only in the gallery's reference section
- **WHEN** the gallery's device-row reference section is viewed
- **THEN** it may show the `.devcard`/`.dmeta` grid variant for documentation purposes even though
  `DevicesView` does not use it, and this variant's CSS ships only in the gallery-only stylesheet,
  never in the production stylesheet

### Requirement: Gallery covers long-text and all four documented empty states
The gallery SHALL include, for every component whose layout depends on content length (row titles,
meta lines, device names, banner messages), at least one example with unusually long text
(overflow/ellipsis/wrap check) alongside a normal-length example, and at least one example of each
of the four documented empty states (no history, no search results, no paired devices, and each of
the popup's offline / starting-up / no-matches / nothing-copied-yet states) — all rendered through
`EmptyState`'s one public component API.

#### Scenario: Long text does not break row layout
- **WHEN** the gallery renders a history row whose preview text is significantly longer than the
  row width
- **THEN** the preview text is truncated with an ellipsis and the row height does not grow beyond
  its single-line spec

### Requirement: Gallery renders every theme × accent combination without twelve full app copies
The gallery SHALL provide a way to view its full component set under all 12 theme×accent
combinations (2 themes × 6 accents) via an in-gallery local-state control that switches the
rendered `data-theme`/`data-accent`/`data-translucency` on a scoped `.theme-scope` wrapper without
leaving the view or mutating `<html>`, plus the compact critical-component matrix section for
simultaneous full-matrix visual comparison — so every combination can be verified without manually
editing preferences 12 times and without rendering the entire component set 12 times over.

#### Scenario: Switching theme/accent/translucency inside the gallery updates every rendered example
- **WHEN** the user changes the gallery's theme, accent, or translucency control
- **THEN** every component example currently rendered in the gallery updates to reflect the new
  theme/accent/translucency, without navigating away from the gallery view

#### Scenario: The gallery wrapper is scoped, not applied to the document root
- **WHEN** the gallery's theme/accent/translucency switcher is used
- **THEN** `data-theme`/`data-accent`/`data-translucency` are set on a
  `.theme-scope[data-theme][data-accent][data-translucency]` wrapper element that the gallery
  renders inside, not on `document.documentElement`, and the `design-tokens` capability's token
  selectors resolve correctly against that scoped wrapper (not exclusively against `:root`)

### Requirement: Browsing the gallery does not alter the user's persisted preferences
Interacting with the gallery's theme/accent/translucency preview control SHALL NOT write to the
same persisted `UIPrefs.theme`/`UIPrefs.accent`/`UIPrefs.translucency` fields the rest of the app
reads, so that leaving the gallery restores the user's actual saved preferences.

#### Scenario: Leaving the gallery restores the real preference
- **WHEN** the user's saved preference is `dark`/`indigo`/translucency-on, they open the gallery,
  switch its preview control through several theme/accent/translucency combinations, and then
  navigate to another view
- **THEN** the main window returns to `data-theme="dark"`/`data-accent="indigo"`/
  `data-translucency="on"` and `UIPrefs` in the persisted store still reads unchanged

### Requirement: Gallery and mock-IPC fixtures are produced by shared, typed factories
The system SHALL provide typed fixture factory functions (e.g. `makeHistoryEntry(overrides)`,
`makeDevice(overrides)`) under a shared, DEV-only module, used by **both** `mockIpc.ts` and the
gallery, with per-call-site override support for gallery-specific states (long text, secret,
unknown kind). These factories, and any secret-like sample values they produce, SHALL be excluded
from the production bundle by the same dynamic-import/DEV gate that excludes the gallery itself.

#### Scenario: Gallery and mock IPC render structurally identical sample data
- **WHEN** the gallery renders a sample history entry and `mockIpc.ts` returns a sample history
  entry for the same scenario
- **THEN** both are produced by the same factory function, so their shape cannot drift from the
  real `HistoryEntry` type independently in the two call sites

#### Scenario: Fixture factories are excluded from the production bundle
- **WHEN** the production build is inspected (per the chunk-graph check in the gallery-exclusion
  requirement above)
- **THEN** the shared fixture factory module is not present in any production-entry-reachable
  chunk
