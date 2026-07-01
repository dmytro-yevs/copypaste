## Why

`crates/copypaste-ui` was recently BARE-STRIPPED (CopyPaste-3sys): every component now renders
bare semantic HTML with zero `className`/inline `style` and no icons. This was intentional — it
gives us a clean canvas — but today the app has no visual design at all. Meanwhile
`copypaste-design-reference.html` and `docs/design/STYLEGUIDE.md` already define an approved,
token-driven design system (theme dark/light × 6 accents, ITCSS-style CSS layers, a full component
inventory) that has never been wired into the real app. We need to implement that design system as
plain CSS against the stripped components, cover every surface (main window + popup), and ship a
first-class preview gallery so every component/state can be verified in a browser without a
running daemon.

## What Changes

- Add `crates/copypaste-ui/src/index.css` as the single stylesheet (ITCSS layers: tokens → base →
  primitives → patterns → app shell → utilities), imported by both `index.html` and `popup.html`.
  **BREAKING**: replaces the (currently absent) ad-hoc styling approach; no Tailwind, no CSS-in-JS.
- Port the full token architecture from `copypaste-design-reference.html` verbatim into layer 1:
  surface/line/text/overlay/status/content-type tokens for `data-theme="dark"|"light"`, 6
  `data-accent` variants (indigo/blue/teal/green/amber/rose), spacing/radius/shadow/motion scales.
  Zero hardcoded hex/px values are permitted outside this layer.
  `App.tsx`/`main.tsx` set `data-theme`/`data-accent` on `<html>`; no `data-palette`/`data-skin`/
  density/contrast/motion axes (matches STYLEGUIDE.md §2, §12 "Definition of done").
- Re-skin every stripped component under `src/components/`, `src/views/` (History, Devices,
  Settings + all tabs, About, Logs), and `src/popup/` using semantic CSS classes only — restoring
  icons (inline SVG, lucide-style per STYLEGUIDE.md §8), states (hover/active/selected/disabled/
  pinned/copied/removing), and the exact anatomy specified in STYLEGUIDE.md §9 (buttons, toggle,
  segmented, field, chips/badges, list row, device card, banner, modal, empty state, sidebar, tab
  bar, popup).
- Extract any element/pattern occurring ≥2× into a single reusable component/CSS class (DRY) —
  e.g. one `.row` for all clip kinds, one `.devcard`/`.devrow` for own+peer devices, one `.btn`
  family for every button variant, one `.banner` for all 4 banner variants, one `.empty` for every
  empty state.
- Add a new **component preview gallery** route/screen that renders every component in every state
  (both themes × all 6 accents × hover/active/disabled/long-text/empty), reachable in the browser
  via the existing `?mock=1` preview infra (localhost:1420) with no daemon required.
- Preserve all existing `role`/`id`/`aria-*` attributes and WCAG AA contrast already encoded in the
  stripped components; no regressions to keyboard focus (`:focus-visible`) or screen-reader labels.

## Capabilities

### New Capabilities
- `design-tokens`: the ITCSS token/base layer — CSS custom properties for theme × accent, spacing,
  radius, shadow, typography, and motion scales, wired via `data-theme`/`data-accent` on `<html>`.
- `component-library`: the re-skinned, DRY component set (primitives + patterns) covering every
  surface of the main window and the popup, built from `index.css` classes with restored icons,
  states, and a11y attributes.
- `preview-gallery`: a dedicated in-app route that renders every component/state combination for
  visual QA via `?mock=1`, without requiring a live daemon.

### Modified Capabilities
(none — no pre-existing `openspec/specs/` capabilities cover UI styling; this is greenfield for the
spec tracker even though the underlying app code already exists.)

## Impact

- **Affected code**: `crates/copypaste-ui/src/index.css` (new), `src/App.tsx`, `src/main.tsx`,
  every file under `src/components/`, `src/views/` (incl. `HistoryView/`, `DevicesView/`,
  `SettingsView/tabs`, `SettingsView/components`), `src/popup/`, plus `index.html`/`popup.html`
  (stylesheet `<link>`/import) and `src/store.ts` (theme/accent persisted state, if not already
  present).
- **New code**: a preview-gallery view/route + its own data fixtures (reusing `?mock=1` mock data
  where possible) gated so it never ships reachable from production navigation without the flag.
- **Dependencies**: none added — plain CSS only, no Tailwind, no CSS-in-JS, no new npm packages.
- **Systems**: desktop main window (`index.html`) and quick-paste popup (`popup.html`); no changes
  to `copypaste-daemon`, IPC contracts, or the Android app (parity is a documented follow-up in
  STYLEGUIDE.md §11 but out of scope for this change).
- **Verification**: every surface (History/Devices/Settings incl. all 5 tabs/About/Logs/popup) and
  the gallery must be visually verified in a real browser at `localhost:1420/?mock=1` before this
  change is considered done (see tasks.md).
