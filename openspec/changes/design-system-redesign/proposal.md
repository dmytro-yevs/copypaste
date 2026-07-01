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

- Add `crates/copypaste-ui/src/styles/{reset,tokens,base,primitives,patterns,shell,utilities}.css`
  as the authored source files, imported in that order into one emitted stylesheet consumed by both
  `index.html` and `popup.html`, using native CSS `@layer reset, tokens, base, primitives, patterns,
  shell, utilities` so cascade order is enforced by the browser, not by file concatenation order.
  **BREAKING**: replaces the (currently absent) ad-hoc styling approach; no Tailwind, no CSS-in-JS.
  See `design.md` Decision 2 (CSS architecture) for the specificity policy and gallery-only-CSS
  production exclusion.
- Port the full token architecture from `copypaste-design-reference.html` into the `tokens` layer:
  surface/line/text/overlay/status/content-type tokens for `data-theme="dark"|"light"`, 6
  `data-accent` variants (indigo/blue/teal/green/amber/rose), a translucency axis (`data-translucency`,
  default on), and spacing/radius/shadow/motion/typography scales. Zero hardcoded hex/px values are
  permitted outside this layer except the runtime-computed geometry called out in `design.md`
  Decision 12 (pixel policy). A synchronous pre-paint bootstrap script (not a React effect) applies
  the persisted theme/accent/translucency to `<html>` in both `index.html` and `popup.html` before
  first paint; a React effect keeps it live-synchronized afterward, including across windows
  (`design.md` Decisions 4 and 8). No `data-palette`/`data-skin`/density/contrast/motion axes
  (matches STYLEGUIDE.md §2, §12 "Definition of done").
- Re-skin every stripped component under `src/components/`, `src/views/` (History, Devices,
  Settings + all tabs, About, Logs), and `src/popup/` using semantic CSS classes only — restoring
  icons (`lucide-react` as the single normative icon source; inline SVG only as a documented
  fallback), states (hover/active/selected/disabled/pinned/copied/removing), and the exact anatomy
  specified in STYLEGUIDE.md §9 (buttons, toggle, segmented, field, chips/badges, list row, device
  card, banner, modal, empty state, sidebar, tab bar, popup). A shared `Dialog` primitive (composing
  the existing `useFocusTrap` hook and `ConfirmModal`'s dismissal pattern) backs every modal; see
  `design.md` Decision 5.
- Apply semantic reuse criteria (`design.md` Decision 3), not a mechanical "≥2 occurrences" rule:
  reuse a component when anatomy, semantics, interaction, accessibility, and variants align; reuse
  tokens/primitives when only presentation aligns. Behavior-heavy patterns (modal, toggle, segmented
  control, banner actions, expandable disclosure rows) get typed React primitives, not CSS-only
  reuse — e.g. one `.row` anatomy (via shared `normalizeContentKind()`/`ContentTile`/`ClipPreview`
  units) for all clip kinds, one `.devcard`/`.devrow` for own+peer devices, one `.btn` family for
  button-shaped controls (tabs/icon-buttons/chips/row-actions have their own allowed primitives per
  `design.md` Decision 3), one `.banner` for all 4 banner variants, one `.empty` for every empty
  state (4 documented states — offline, starting-up, no-matches, nothing-copied-yet).
- Ship a **Translucency** toggle (default on) alongside Theme/Accent in `DisplayTab.tsx`, persisted
  in `UIPrefs` (additive field, no migration): on = chrome surfaces (sidebar, popup container, modal scrim, toast, tab bar) use
  `backdrop-filter`; content surfaces (cards, rows, fields) stay solid; off = every surface solid.
  Falls back to solid when `backdrop-filter` is unsupported or `prefers-reduced-transparency` is set.
- Add a new **component preview gallery**, DEV-gated behind a dynamic import (mirroring the existing
  `mockIpc.ts` pattern) so it is provably absent from the production module graph — not merely
  hidden from navigation. It renders every component/state (both themes × all 6 accents ×
  hover/active/disabled/long-text/empty) reachable in the browser via the existing `?mock=1` preview
  infra (localhost:1420) with no daemon required, using a scoped `.theme-scope[data-theme][data-accent]`
  wrapper rather than root mutation.
- Preserve accessible behavior via observable contracts (correct role/name, resolved
  labelled/described relationships, exposed state, unchanged-or-improved keyboard behavior, stable
  test IDs only where they are an intentional contract) rather than requiring every `role`/`id`/
  `aria-*` to remain on the literal same element — this allows fixing the existing P0 accessibility
  gap where masked sensitive content leaks plaintext into the accessible name (see `design.md`
  Decision 7, sensitive-masking contract).
- Land the change in 6 build-independent slices within this one OpenSpec change (tokens/bootstrap/
  prefs; typed primitives/Dialog; History+Popup; Devices; Settings+shell; Gallery+automated
  verification) — see `tasks.md` and `design.md` Decision 1. Automated Playwright coverage (dark/
  light × main/popup, accent matrix, modal keyboard/focus, reduced-motion, contrast, production
  gallery exclusion) is a required CI gate, not a manual spot-check (`design.md` Decision 13).

## Capabilities

### New Capabilities
- `design-tokens`: the layered token/base layer — CSS custom properties for theme × accent ×
  translucency, spacing, radius, shadow, typography, and motion scales, wired via
  `data-theme`/`data-accent`/`data-translucency` on `<html>` through a pre-paint bootstrap plus a
  live React effect, backed by validated, versioned `UIPrefs` persistence.
- `component-library`: the re-skinned, semantically-DRY component set (primitives + patterns)
  covering every surface of the main window and the popup, including the shared `Dialog` primitive,
  shared clipboard-presentation units, and behavior/state contracts for devices, hover-revealed
  actions, and sensitive-content masking.
- `preview-gallery`: a DEV-only route (dynamic-import gated, absent from the production bundle)
  that renders every component/state combination for visual QA via `?mock=1`, without requiring a
  live daemon, structured for automated Playwright coverage rather than ad hoc manual review.

### Modified Capabilities
(none — no pre-existing `openspec/specs/` capabilities cover UI styling; this is greenfield for the
spec tracker even though the underlying app code already exists.)

## Impact

- **Affected code**: `crates/copypaste-ui/src/styles/*.css` (new), `index.html`/`popup.html`
  (pre-paint bootstrap `<script>` + stylesheet import), `src/App.tsx`, `src/main.tsx`,
  `src/popup/main.tsx`, `src/store.ts` (theme/accent/translucency persisted state, `UIPrefs` additive fields,
  runtime validation, `ProductionViewId`/`DevViewId` split), and every file under `src/components/`,
  `src/views/` (incl. `HistoryView/`, `DevicesView/`, `SettingsView/tabs`,
  `SettingsView/components`), `src/popup/`. `design.md`'s component inventory table maps each
  existing component to one of {unchanged/class-only, composed-from-new-primitive, behavior-changed,
  deprecated/removed, gallery-only} — that table, not a blanket "every file" declaration, is the
  actual review-risk boundary (resolves review finding D2).
- **New code**: `src/lib/dialog/` (shared `Dialog` primitive composing `useFocusTrap`), shared
  clipboard-presentation units (`normalizeContentKind()`, `ContentTile`, `ClipPreview`,
  `ClipMetadata`), a preview-gallery view + shared typed fixture factories (used by both mock IPC
  and the gallery), all DEV-gated so none of it reaches the production module graph.
- **Dependencies**: none added — plain CSS only, no Tailwind, no CSS-in-JS, no new npm packages.
- **Systems**: desktop main window (`index.html`) and quick-paste popup (`popup.html`); no changes
  to `copypaste-daemon`, IPC contracts, or the Android app (parity is a documented follow-up in
  STYLEGUIDE.md §11 but out of scope for this change). Minimum supported platform is macOS 13+
  (WKWebView/Safari 16.2), so `color-mix()` is used natively with no fallback path required
  (`design.md` Decision 9).
- **Verification**: an automated Playwright suite (main + popup, dark/light, accent/on-accent
  matrix, modal keyboard/focus, reduced-motion, long-text overflow, production gallery exclusion,
  token-contrast checks, a11y scan where tooling is available) is a required CI gate for this
  change, per `design.md` Decision 13; manual `?mock=1` browser verification remains useful for
  exploratory spot-checks but is not itself the completion bar (see `tasks.md` slice 6).
- **Performance budgets**: popup open/render latency and CSS/JS bundle size deltas are measured
  against a pre-change baseline and gated as acceptance criteria (`design.md` Decision 14).
