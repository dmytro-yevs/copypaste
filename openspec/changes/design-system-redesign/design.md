## Context

`crates/copypaste-ui` (React 19 + TS + Vite 7, Tauri 2) currently has **no visual design**:
components were deliberately BARE-STRIPPED (CopyPaste-3sys/CopyPaste-h1n3) — every `className`,
inline `style`, and icon was removed, leaving bare semantic HTML (`<div>`, `<button>`, `<span>`,
`role`/`aria-*` intact). In the same demolition pass, the *entire* theming system was deleted:

- `src/store.ts`'s `UIPrefs` (v4 schema) has **no `theme` or `accent` field at all** — the v3→v4
  migration comment says old appearance fields were "removed", not renamed.
- `index.html` still carries stale attributes from the prior "Liquid Glass" era —
  `data-theme="light" data-palette="graphite-mist" data-density="compact" data-motion="cinematic"
  data-contrast="balanced"` — none of which anything reads or writes anymore.
- `popup.html` carries a stray `data-theme="light"` with the same dead comment trail.
- `SettingsView/tabs/DisplayTab.tsx` has a comment: "Appearance section (Theme, Accent,
  Translucency) removed."

Meanwhile an approved, from-scratch design system already exists and is NOT superseded:
`copypaste-design-reference.html` (979 lines, executable reference — token layers, live app shell,
component gallery, mobile mirror, states) plus `docs/design/STYLEGUIDE.md` (549 lines, the written
spec, explicitly "source of truth for the design migration"). `docs/design/DESIGN-SYSTEM-v2.md` is
marked `[SUPERSEDED — HISTORICAL REFERENCE ONLY]` in its own banner and must **not** be used as a
source (it documents the deleted Liquid-Glass system this change replaces).

Existing infra we build on, unchanged:
- `crates/copypaste-ui/src/lib/ipc/transport.ts` exports `MOCK: boolean` — true only in dev builds
  when `?mock=1`/`VITE_MOCK=1` is set; tree-shaken out of production. This is the correct gate for
  anything gallery/dev-only.
- `vite.config.ts` already builds a **multi-page app** (`index.html` + `popup.html` as separate
  Rollup inputs) on a fixed dev port `1420`.
- `package.json` already depends on `lucide-react@^1.18.0` — no new icon dependency needed.
- Playwright visual tests already exist (`test:visual` / `test:visual:update`) — natural home for
  gallery-driven screenshot regression, though wiring that is a stretch goal, not required scope.

## Goals / Non-Goals

**Goals:**
- Ship one CSS file (`src/index.css`), ITCSS-layered, that is the single styling mechanism for
  both windows. No Tailwind, no CSS-in-JS, no per-component stylesheets.
- Port the token architecture from `copypaste-design-reference.html` verbatim (same variable names,
  same values) so the reference file and the shipped CSS can never silently drift.
- Restore `theme`/`accent` (and optionally `translucency`) to `UIPrefs`, write them to
  `data-theme`/`data-accent` on `<html>` in both windows, and rebuild the Appearance section of
  Settings (`DisplayTab.tsx`) to expose exactly Theme + Accent (+ Translucency), per
  STYLEGUIDE.md §2/§12.
- Re-skin every stripped component/view/popup file with token-driven CSS classes, restoring icons
  and interaction states without touching component logic, props, or existing tests.
- Enforce DRY: one canonical class/component per repeated pattern (`.row`, `.devcard`/`.devrow`,
  `.btn`, `.banner`, `.empty`, `.chip`, `.toggle`, `.seg`, `.field`, `.modal`, `.tile`, `.kbd`, …),
  matching the primitives cataloged in `copypaste-design-reference.html`'s Layer 3/4.
- Ship a preview gallery reachable at `localhost:1420/?mock=1`, gated by the existing `MOCK` flag,
  rendering every component in every state × both themes × all 6 accents.
- Preserve 100% of existing `role`/`id`/`aria-*`/`data-testid` attributes; no regression to
  keyboard focus order or screen-reader labels; WCAG AA contrast per STYLEGUIDE.md §3.3/§7.

**Non-Goals:**
- Android/Compose parity (`copypaste-android`) — STYLEGUIDE.md §11 documents the target but it is
  explicitly a separate, later migration; out of scope here.
- Any change to IPC contracts, daemon behavior, or `copypaste-core`/`copypaste-ipc` types.
- New component *behavior* (state machines, data flow) — this change is purely presentational;
  components keep their existing props/hooks/logic.
- Automated visual-regression CI wiring (Playwright snapshot baselines) — nice-to-have, tracked as
  a follow-up task, not required for this change to be considered done.
- A generic Storybook-style tool — the gallery is a bespoke in-app view, not a new dependency.

## Decisions

### 1. Single `src/index.css`, ITCSS layers, imported by both entry points
**Decision:** One stylesheet at `crates/copypaste-ui/src/index.css`, imported from both
`src/main.tsx` (main window) and `src/popup/main.tsx` (popup), organized top-to-bottom as:
`1-tokens` → `2-base` → `3-primitives` → `4-patterns` → `5-app-shell` → `6-utilities`. Sections are
delimited by banner comments (mirroring the reference file's `LAYER N` comments) rather than split
into separate files, to keep cascade order trivially correct with a single `<link>`/import and
avoid import-order bugs across two HTML entry points.
**Alternative considered:** per-component CSS Modules. Rejected — the design system is
token/pattern-driven, not component-scoped (the same `.row` class serves 11 clip kinds); CSS
Modules would fight the DRY requirement by encouraging duplication per component.
**Alternative considered:** multiple physical files (`tokens.css`, `base.css`, …) per
DESIGN-SYSTEM-v2.md's now-superseded layout. Rejected for *this* change — STYLEGUIDE.md §10 shows
a single token block being dropped into one file, and the user directive is explicit: "Styling =
plain CSS in `crates/copypaste-ui/src/index.css`". One file, internally layered, satisfies both.

### 2. Token values copied verbatim from `copypaste-design-reference.html`
**Decision:** Every custom property (surfaces, lines, text, overlays, status, content-type,
accents, spacing, radius, shadow, motion) is copied byte-for-byte from the reference file's Layer
1 block (lines 10–54) into `index.css`'s tokens layer — including both `:root[data-theme="dark"]`
and `:root[data-theme="light"]` blocks and all six `:root[data-accent="…"]` blocks plus the
light-theme accent overrides. No renaming, no re-deriving values.
**Rationale:** the reference file *is* the approved design; any transcription drift becomes a bug
that's invisible until a specific theme×accent combination is viewed. Byte-identical copy makes
the reference file itself the executable acceptance test.
**Alternative considered:** re-derive tokens from `STYLEGUIDE.md`'s prose tables. Rejected as a
primary source — the tables are consistent with the reference file today, but the HTML file is
higher-fidelity (it's rendered, not transcribed) and is what the user named as "THE design system
source of truth".

### 3. Theme/accent state: restore to `UIPrefs`, write to `<html>` via a `useEffect`
**Decision:** Add `theme: "dark" | "light"` and `accent: "indigo" | "blue" | "teal" | "green" |
"amber" | "rose"` (default `"dark"` / `"indigo"`, matching the reference file's default) back to
`UIPrefs` in `store.ts`, bump to a new persisted key (`copypaste-ui-prefs-v5`) with a migration
that carries forward all v4 fields and defaults the two new ones. `App.tsx` and `Popup.tsx` each
get a `useEffect` that sets `document.documentElement.dataset.theme` / `.dataset.accent` from
`prefs.theme`/`prefs.accent` on mount and on change — mirroring STYLEGUIDE.md §10's `App.tsx`
snippet. `index.html`/`popup.html` keep only a static `data-theme="dark"` attribute (for
first-paint, before JS runs) and drop `data-palette`/`data-density`/`data-motion`/`data-contrast`
entirely (STYLEGUIDE.md §12 "Definition of done").
**Rationale:** `UIPrefs` is already the one persisted-preferences object read by both windows;
reusing it avoids a second storage mechanism. A v5 bump (rather than mutating v4) keeps the
existing whitelist-based migration pattern (`knownKeys` prune) working unchanged.
**Alternative considered:** a separate `ThemeContext`/localStorage key just for
theme/accent. Rejected — `UIPrefs` migration plumbing already exists and is exercised by tests;
splitting state stores doubles the persistence surface for no benefit.

### 4. DRY component inventory — CSS classes map 1:1 to reference-file primitives
**Decision:** Re-skinning does not invent new visual patterns; it maps each stripped
component/view to the *existing* named class(es) in `copypaste-design-reference.html`:
`Sidebar.tsx`→`.sb`/`.sb__item`, `HistoryRow.tsx`→`.row`/`.row__body`/`.row__title`/`.row__meta`/
`.tile`, `DeviceCard.tsx` (`ThisDeviceCard`/`PeerRow`)→`.devcard`/`.dmeta` (desktop grid card
variant) or `.devrow`/`.cfields` (the reference file's expandable-row variant used in its live app
demo) — **the row/expandable variant is selected** because it is what the interactive `#app` demo
actually ships (see Decision 4a), `SettingsRow.tsx`/`Panel.tsx`→`.srow`/`.set-grp`, `Toggle.tsx`→
`.toggle`, `ConfirmModal.tsx`→`.modal`/`.scrim`, `EmptyState.tsx`→`.empty`, banners in `App.tsx`
(`AccessibilityBanner`, mismatch/stale banners)→`.banner banner--{warn|err|info|ok}`,
`PopupRow.tsx`→condensed `.row` variant used in `#mobile`/popup mock-ups. Each CSS class is defined
once in the patterns layer and reused by every component that needs it — never duplicated per
component file.

**4a. Device list uses `.devrow` (expandable row), not `.devcard` (grid card).**
The reference file demonstrates *both* shapes: `.devcard` in the component gallery section (a
static card spec) and `.devrow`/`.cfields` in the *live, interactive* `#app` demo and the mobile
mirror — expandable rows, "tap any field to copy". Since `DevicesView.tsx` today renders a linear
list (not a grid) and `DeviceCard.tsx` already exports `ThisDeviceCard`/`PeerRow` as list items
(not a `dev-grid`), the `.devrow` pattern is the better fit and is what actually ships in the
reference's working prototype. `.devcard`/`.dmeta` tokens remain documented in the
`component-library` spec as an available primitive (for any future grid layout) but are not wired
into `DevicesView` by this change.

### 5. Preview gallery: a 6th `ViewId`, gated by the existing `MOCK` export, not a new route/page
**Decision:** Add `"gallery"` to `ViewId` in `store.ts` and a `GalleryView` component under
`src/views/GalleryView/`. `Sidebar.tsx` renders the Gallery nav item only when
`import.meta.env.DEV && MOCK` is true (the same flag `lib/ipc/transport.ts` already exports and
tree-shakes from production). This makes the gallery reachable at exactly
`localhost:1420/?mock=1` (click "Gallery" in the sidebar) with zero new Vite entry points, zero new
routing library, and a guarantee it never ships to end users (dead code eliminated exactly like
`mockIpc.ts` already is).
**Alternative considered:** a new HTML entry point (`gallery.html`) alongside `index.html`/
`popup.html`. Rejected — the task explicitly says the gallery must be viewable "via the existing
preview infra (localhost:1420/?mock=1)", i.e. the *same* main-window URL, not a third page; a new
entry point would also need its own theme/accent wiring duplicated a third time.
**Alternative considered:** a floating theme/accent switcher control overlaid on the gallery only.
Rejected — the gallery must show *every* theme × accent combination simultaneously (per the task's
explicit requirement), so it renders repeated sections per combination rather than relying on a
single live toggle; a small live toggle is still included at the top for spot-checking interaction
states (hover/active/focus) without needing 12 fully-interactive copies.

### 6. Icons: `lucide-react`, sized explicitly, matching the reference file's glyph set
**Decision:** Re-introduce icons via `lucide-react` (already a dependency), choosing components
whose default outline matches the inline SVGs hand-drawn in the reference file (search, chevron,
pin, trash, settings-gear, shield/lock for secrets, link, mail, code braces, file, image, etc.).
Every `<Icon>` usage gets an explicit `size`/CSS box per STYLEGUIDE.md §8 ("every inline icon has a
fixed size") to avoid the documented "SVG balloons to intrinsic 300×150" bug class.
**Alternative considered:** hand-copy the reference file's raw inline `<svg>` markup instead of
`lucide-react` components. Rejected — `lucide-react` is already installed and is the icon set
STYLEGUIDE.md §8 names explicitly ("the matching set... `lucide-react` on web"); using the library
gives consistent stroke-width/viewBox for free and avoids maintaining ~40 hand-copied SVG strings.

## Risks / Trade-offs

- **[Risk] Token transcription drift between the reference HTML and `index.css`.**
  → Mitigation: copy-paste the token blocks verbatim (Decision 2); the `component-library`/
  `design-tokens` specs require a task that diffs variable *names* between the two files as part of
  verification.
- **[Risk] Restoring `theme`/`accent` to `UIPrefs` touches the shared prefs migration chain (v1→v4
  already has 3 legacy-key migration branches) — a bug here could corrupt existing users'
  settings.** → Mitigation: additive-only change (v4→v5 keeps the same whitelist-prune pattern,
  only adds two keys with defaults); existing prefs tests must stay green; no field is removed or
  renamed.
- **[Risk] `.devrow`/`.cfields` selection (Decision 4a) means `.devcard`/`dev-grid` from the
  reference file's gallery section is spec'd but never rendered in the real app** — a future
  redesign of `DevicesView` into a grid would need to re-derive wiring. → Mitigation: document both
  shapes in `component-library` spec explicitly so the unused primitive isn't lost; gallery view
  renders both variants for visual reference even though `DevicesView` only uses one.
- **[Risk] Popup window is a separate, frameless, always-on-top surface — restoring shadows/blur
  (`--sh3`, translucency) could hurt paste-latency perf if not GPU-cheap.** → Mitigation: keep
  `backdrop-filter`/`box-shadow` usage identical to the reference file (already tuned for a
  compact popup) and avoid adding new blur surfaces beyond what STYLEGUIDE.md §9.13 specifies.
- **[Risk] Reduced-motion / a11y regressions when re-adding animation (row insert/remove, toast,
  spinner, pulse dot).** → Mitigation: the reference file's `@media (prefers-reduced-motion:
  reduce)` block (collapsing all `--dur*` tokens to `0ms`) is copied verbatim as part of the tokens
  layer; this is non-optional per STYLEGUIDE.md §6.
- **[Trade-off] No automated visual-regression baseline is required for this change**, even though
  Playwright visual tests exist in the repo. → Accepted: manual `?mock=1` browser verification
  (tasks.md) is the bar for this change; wiring gallery-driven Playwright snapshots is filed as
  follow-up, not blocking.

## Migration Plan

1. Land `index.css` (tokens → base → primitives) with **no component changes yet** — verify the
   app still builds/renders (unstyled, since no component references the new classes yet).
2. Restore `theme`/`accent` to `UIPrefs` + `<html>` wiring in both windows; verify persisted prefs
   migrate cleanly from v4.
3. Re-skin components bottom-up: shared primitives first (`ActionButton`, `Toggle`, `EmptyState`,
   `SectionHeader`, `Panel`, chips/badges), then per-surface (Sidebar → History → Devices →
   Settings tabs → About/Logs → popup), so later steps can compose already-styled primitives.
4. Build the gallery view last, once every primitive/pattern class exists, so it has something real
   to render (not a placeholder).
5. Manually verify every surface + the gallery at `localhost:1420/?mock=1` in both themes and a
   sample of accents (full 6×2 matrix for the gallery itself, spot-check for individual views).
6. No rollback complexity: this is additive CSS + restored state fields; reverting is a plain
   `git revert` of the change's commits with no data migration to undo (v5 prefs key is additive).

## Open Questions

1. **Translucency toggle** — STYLEGUIDE.md §2 lists it as an *optional* remaining boolean, and
   DESIGN-SYSTEM-v2.md (superseded) had a fuller glass/solid dual-surface system. Should this
   change ship a working Translucency toggle (frosted `backdrop-filter` on/off), or defer it and
   ship only Theme + Accent in `DisplayTab.tsx`? Needs a user decision before `tasks.md`
   implementation of `DisplayTab.tsx` locks the field list.
2. **Content-type detection coverage** — the reference file's `.row` demo covers 11 kinds
   (`text/url/email/phone/code/json/number/color/path/file/secret`). Does the current
   `HistoryEntry.kind` enum in `copypaste-core`/`copypaste-ipc` cover all 11, or should the gallery
   only demonstrate the kinds the backend actually emits today? Affects gallery scope and whether
   `PHONE`/`NUMBER` tile styling is reachable in the real app vs. gallery-only.
  3. **Source-app icon** (mentioned in the superseded DESIGN-SYSTEM-v2.md §4 as blocked on daemon
   `source_bundle_id`) — out of scope here since it needs daemon data, but should the row anatomy
   reserve visual space for it now (empty slot) so a future daemon change doesn't force a second
   layout pass? Recommend: yes, reserve the slot, ship type-glyph fallback only — flag for user
   confirmation.
4. **Gallery persistence** — should the theme/accent choices made while browsing the gallery leak
   into the user's real `UIPrefs` (since it's the same store), or should the gallery use fully
   local component state so browsing all 12 combinations doesn't change the user's actual saved
   preference? Recommend local state scoped to the gallery view; flagging for confirmation since it
   changes the gallery's implementation shape (task estimate).
