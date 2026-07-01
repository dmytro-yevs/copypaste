## Context

`crates/copypaste-ui` (React 19 + TS + Vite 7, Tauri 2) currently has **no visual design**:
components were deliberately BARE-STRIPPED (CopyPaste-3sys/CopyPaste-h1n3) — every `className`,
inline `style`, and icon was removed, leaving bare semantic HTML (`<div>`, `<button>`, `<span>`,
`role`/`aria-*` intact). In the same demolition pass, the *entire* theming system was deleted:

- `src/store.ts`'s `UIPrefs` (v4 schema, `PREFS_KEY = "copypaste-ui-prefs-v4"`) has **no `theme`,
  `accent`, or `translucency` field at all** — the v3→v4 migration comment says old appearance
  fields were "removed", not renamed. `loadPrefs()` already does a whitelist-prune of unknown keys
  (`knownKeys`) but merges parsed JSON into `UIPrefs` via a bare cast
  (`{ ...DEFAULT_PREFS, ...parsed } as UIPrefs`) with **no per-field runtime validation** — a
  malformed `theme: "system"` or `accent: "purple"` value would reach the DOM unchecked.
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
  when `?mock=1`/`VITE_MOCK=1` is set. The `MOCK`/mock-invoke branch is behind an `import.meta.env.DEV`
  guard and pulls `mockIpc.ts` in via a **dynamic** `await import("../mockIpc")`, not a static
  import — this is the exact pattern the gallery must mirror (see Decision 6).
- `vite.config.ts` already builds a **multi-page app** (`index.html` + `popup.html` as separate
  Rollup inputs) on a fixed dev port `1420`.
- `package.json` already depends on `lucide-react@^1.18.0` — no new icon dependency needed.
- `src/lib/useFocusTrap.ts` already implements a full modal focus-trap hook: captures the
  previously-focused element, focuses the first focusable descendant (or the container) on mount,
  traps Tab/Shift+Tab, delegates Escape to an optional `onEscape` callback, and restores focus to
  the pre-open element on unmount. `src/components/ConfirmModal.tsx` already composes it
  (`useFocusTrap(dialogRef)`), renders via `ReactDOM.createPortal` to `document.body`, sets
  `role="dialog"`/`aria-modal="true"`/`aria-labelledby`, dismisses on backdrop click and on Escape.
  **None of this is new behavior to build — it already exists and already works.** See Decision 5.
- `src/hooks/useSensitiveReveal.ts` already implements masked-content reveal + auto-re-mask on
  window `blur` (SCRH-7). **Also not new** — see Decision 9.
- `src/lib/ipc/types.ts`'s `HistoryEntry.kind` is `kind?: string` — an **open, optional** string,
  not a closed union of the 11 design-reference kinds. Any normalization/fallback logic must handle
  `undefined`, unknown, and future values. See Decision 8.
- Playwright visual tests already exist (`test:visual` / `test:visual:update`) — the natural home
  for the automated gallery-driven verification this change now requires (Decision 13), not a
  stretch goal.

## Goals / Non-Goals

**Goals:**
- Ship one **emitted** stylesheet built from multiple authored source files
  (`src/styles/{reset,tokens,base,primitives,patterns,shell,utilities}.css`), using native CSS
  cascade layers, imported by both windows. No Tailwind, no CSS-in-JS, no per-component
  stylesheets. See Decision 2.
- Port the token architecture from `copypaste-design-reference.html` into the `tokens` layer, kept
  in exact name-**and**-value parity with the reference via an automated test (not a name-only
  diff). See Decision 2 and Decision 11 (token parity).
- Restore `theme`/`accent`/`translucency` to `UIPrefs` as **additive fields (no migration, no
  back-compat)**, applied via a synchronous pre-paint bootstrap script in both `index.html` and
  `popup.html` before first paint, kept live-synchronized by a React effect within each window, and
  best-effort synchronized across windows (Settings → open popup — "updates as it can"). Rebuild the
  Appearance section of `DisplayTab.tsx` to expose Theme + Accent + Translucency. See Decisions 4 and
  10.
- Re-skin every stripped component/view/popup file with token-driven CSS classes, restoring icons
  and interaction states, using semantic reuse criteria (not a mechanical occurrence count) to
  decide what becomes a shared primitive. See Decision 3.
- Ship a preview gallery reachable at `localhost:1420/?mock=1`, DEV-dynamic-import-gated so it is
  provably absent from the production module graph (not merely unreachable from navigation), using
  a scoped theme wrapper rather than root mutation, structured for automated Playwright coverage.
  See Decisions 6, 7 (gallery structure), and 13 (verification).
- Preserve accessible behavior via **observable contracts** — correct role/name, resolved
  labelled/described relationships, exposed state, keyboard behavior unchanged or improved, stable
  test IDs only where they are an intentional contract — rather than requiring every `role`/`id`/
  `aria-*` to remain on the literal same element. This also lets us **fix** the existing P0 gap
  where masked sensitive content currently leaks plaintext into the accessible name. See Decision 9.
- Define and ship a Translucency toggle (default on) with clear on/off surface rules and platform
  fallback. See the persistence/translucency decision below.
- Measure and gate popup latency and CSS/JS bundle-size deltas as acceptance criteria, not just
  "should be fine." See Decision 15.
- Make visual/contrast/keyboard/reduced-motion/production-exclusion checks an automated Playwright
  CI gate. Manual `?mock=1` browser walkthroughs remain useful but are exploratory, not the bar for
  "done." See Decision 13.

**Non-Goals:**
- Android/Compose parity (`copypaste-android`) — STYLEGUIDE.md §11 documents the target but it is
  explicitly a separate, later migration; out of scope here.
- Any change to IPC contracts, daemon behavior, or `copypaste-core`/`copypaste-ipc` types (including
  the eventual `source_bundle_id` field that would make source-app icons fully data-driven).
- **Inventing large new interaction models.** This change adds no new state machines. Most behavior
  is **existing** and merely (re)wired into shared primitives (see the component inventory + Decisions
  5 and 9). It does, however, **add or standardize several genuinely new behaviors** (corrected
  inventory, resolves M4/M5): (1) modal **scroll-lock** — new, and it MUST be stack-safe/
  reference-counted (Decision 5); (2) **best-effort cross-window theme sync** (Decision 4); (3) **tab
  arrow-key navigation** for segmented/tab controls, if not already present (Decision 13/X5); (4) a
  typed **disclosure-header keyboard contract** (`aria-expanded`/`aria-controls`) for expandable rows
  (Decision 3); (5) a **uniform inline device-action error presentation** (Decision 16 — presentation
  only, semantics unchanged); (6) the **masked-sensitive accessible-name fix** (Decision 9 — new
  correct behavior replacing a P0 leak). These are intentional and are called out so the risk
  analysis is honest — not minimized as "one small change."
- A generic Storybook-style tool — the gallery is a bespoke in-app view, not a new dependency.
- Speculative component APIs with no current consumer (e.g. `devcard` grid variant, `devcard`
  component itself) do not ship in production CSS ahead of a real caller — see Decision 15.

## Decisions

### 1. Delivery: six build-independent slices within this one OpenSpec change
**Decision:** `tasks.md` is restructured into six slices, each of which compiles, has its own tests,
and leaves the app in a shippable/usable state on its own:
1. Tokens + cascade layers + pre-paint theme bootstrap + `UIPrefs` additive fields + validation.
2. Typed React primitives + shared `Dialog`/disclosure accessibility foundations.
3. History + Popup via shared clipboard-presentation units.
4. Devices.
5. Settings + sidebar + About + Logs + banners + toast.
6. Gallery + automated visual/accessibility coverage.
**Rationale:** the original single-pass task list made regressions hard to isolate and review
surface unmanageable (staff review finding D1). Slicing by build-independence (rather than by
file-type) means a reviewer can approve slice *N* without slice *N+1* existing yet, and a partial
landing never leaves the app broken.
**Alternative considered:** splitting into separate OpenSpec changes. Rejected — the six slices
share one set of specs/decisions and reviewing them as one change with visible slice boundaries is
lower overhead than cross-referencing six change proposals.

### 2. CSS architecture: multiple source files, native cascade layers, one emitted stylesheet
**Decision:** Author CSS as separate files under `crates/copypaste-ui/src/styles/`: `reset.css`,
`tokens.css`, `base.css`, `primitives.css`, `patterns.css`, `shell.css`, `utilities.css`. A single
entry (`src/styles/index.css`, imported by both `main.tsx` and `popup/main.tsx`) `@import`s them in
order and declares `@layer reset, tokens, base, primitives, patterns, shell, utilities;` up front so
cascade order is enforced by the browser's native layer ordering, not by import/file order alone.
Each file's rules are wrapped in (or the file is entirely) its matching `@layer <name> { … }` block.
Specificity policy (normative): selectors are low-specificity classes; state is expressed via
`[data-*]`/ARIA attributes or native pseudo-classes, never via ID selectors or `!important`; no
view-specific selector is allowed to escalate specificity above its owning layer (e.g. a
`SettingsView`-only override must still live in the `patterns` layer, not stack a higher-specificity
rule in `shell`). Gallery-only CSS (any selector that only the gallery renders, e.g. matrix-grid
layout, forced-state helper classes) lives in its own `src/styles/gallery.css`, imported only by the
DEV-gated gallery module (Decision 6) — it is never part of the production `styles/index.css` chain.
**Rationale (resolves A1):** one authored file for tokens+base+primitives+patterns+shell+utilities
becomes a merge-conflict hotspot with no ownership boundary and no dead-code-analysis boundary.
Multiple files satisfy the actual constraint — "one network asset in production" — without forcing
one physical file. Native `@layer` also removes the "banner comments as cascade order" fragility:
misordered imports can no longer silently invert cascade priority.
**Alternative considered:** one physical file with banner-comment sections (the original proposal).
Rejected per the review: banner comments do not enforce order, and multiple contributors editing
"one big file" is a known hotspot.

### 3. DRY: semantic reuse criteria replace the mechanical "≥2 occurrences" rule
**Decision:** Reuse a **component** (not just a class) when anatomy, semantics, interaction
contract, accessibility contract, and supported variants all align across call sites. Reuse only
**tokens/CSS primitives** when call sites align on presentation but differ in semantics, lifecycle,
or interaction (e.g. two visually-similar rows that will diverge in future features). Behavior-heavy
patterns — modal/dialog, toggle, segmented control, banner-with-actions, expandable disclosure row —
always get a typed React primitive, never a CSS-class-only "shared look," because a class cannot
guarantee keyboard/focus/ARIA behavior.
**Rationale (resolves A2):** visual similarity at two call sites is not evidence of a shared
contract (they may differ in semantics or future change direction); conversely a single call site
can still need a typed primitive if it's behavior-heavy and will gain more call sites (e.g. the
`Dialog` primitive is introduced even though today's dialogs already superficially share `.scrim`/
`.modal` classes — the point is the *behavior* contract, not the class).
**Allowed button-shaped primitives (resolves C3):** `.btn` family (primary/secondary/ghost/danger ×
sm/block/disabled) for standalone actions; `.iconbtn` for icon-only actions; `.set-tab` for settings
tabs (native `role="tab"`); a disclosure header (`aria-expanded`/`aria-controls`, no `.btn` styling)
for expandable rows; `.chip` for filter/selection chips; row-action icon buttons for hover-revealed
per-row actions. A raw `<button>` outside these primitives is not allowed, but forcing every one of
these into the `.btn` family is exactly the over-generalization the review flagged — each has
distinct anatomy and is documented as its own allowed primitive in `component-library` spec.
**Naming/state contract (resolves C1):** component modifier classes (`.btn--small`, `.btn--danger`)
for variants; native states where the platform provides them (`:disabled`, `[aria-selected="true"]`,
`[aria-expanded="true"]`); explicit `data-state` attributes for states with no native equivalent
(`[data-state="removing"]`, `[data-kind="secret"]`). Authority order, most-specific wins: React
prop/state is the source of truth; ARIA attributes are derived from it; CSS reads ARIA/data
attributes and never independently tracks state (no CSS-only toggle that can desync from the DOM's
ARIA state).

### 4. Theme/accent/translucency: pre-paint bootstrap + additive `UIPrefs` fields + best-effort cross-window sync
**Decision:** `UIPrefs` gains three **additive** fields (no version bump — Decision 10):
`theme: "dark" | "light"` (default `"dark"`),
`accent: "indigo" | "blue" | "teal" | "green" | "amber" | "rose"` (default `"indigo"`), and
`translucency: boolean` (default `true`), persisted in the existing `UIPrefs` object at the current
key `copypaste-ui-prefs-v4`. Both `index.html` and
`popup.html` load a small **synchronous, same-origin EXTERNAL classic script**
(`<script src="./theme-bootstrap.js"></script>` — a build-emitted `'self'` asset, **NOT an inline
`<script>`**; see the CSP note below for why), placed **before** any `type="module"` app script so it
runs before the first paint of app content. It:
1. Reads `localStorage["copypaste-ui-prefs-v4"]` (falling back to defaults on missing/malformed
   JSON — a defensive read only; the React store is the authoritative loader on mount).
2. Validates `theme`/`accent`/`translucency` against their known enum/boolean values individually;
   an invalid value for one field falls back to that field's default without discarding the others.
3. Sets `document.documentElement.dataset.theme`, `.dataset.accent`, and
   `document.documentElement.dataset.translucency` (`"on"`/`"off"`) synchronously.
4. Is wrapped in `try/catch` so a `localStorage` access failure (private browsing, disabled storage)
   falls back to defaults rather than throwing before app code runs.
`App.tsx` and `Popup.tsx` each keep a `useEffect` that re-applies the same three `dataset.*` writes
whenever `prefs.theme`/`prefs.accent`/`prefs.translucency` change, so the bootstrap only owns
first-paint correctness and the effect owns live updates within its own window.
**Cross-window sync (resolves A5) — best-effort, not a hard gate (user directive: the real app must
be correct; the theme "updates as it can"):** the **guaranteed** behavior is that the popup applies
the persisted theme/accent/translucency **every time it opens** (it reads the same prefs at mount) —
so the popup is never wrong for more than the lifetime of one already-open session. Live propagation
to an *already-open* popup is best-effort on top of that: the writer emits a Tauri
`emit("ui-prefs-changed", …)` and, where the two windows share a `localStorage` partition, the
`storage` event also fires; each window's listener re-applies the bootstrap logic. **Persistence is
NOT a new architecture (resolves B2's persistence concern):** the app already persists ALL of
`UIPrefs` to `localStorage[PREFS_KEY]` via `loadPrefs()`/`savePrefs()`, and the popup **already reads
those same prefs today** (e.g. `previewLinesPopup` via `useUI(s => s.prefs)` in `Popup.tsx`). Theme /
accent / translucency are just three more fields in that same object, so their cross-window sharing is
**identical to how every existing pref is already shared in the shipped app** — whatever partitioning
Tauri uses is the status quo, not something this change introduces or must re-solve. **Contract
(single, non-contradictory): next-open correctness is REQUIRED** (the popup applies persisted prefs
on every open, exactly as it already does for existing prefs); **live update to an already-open popup
is BEST-EFFORT** (Tauri `ui-prefs-changed` event, plus `storage` event where partitions are shared) —
if a live channel doesn't reach an open popup it corrects on next open. The `design-tokens`
capability spec is worded to match this exactly (required next-open, best-effort live) — it does not
`SHALL`-mandate live update, removing the round-1/round-2 contradiction. Acceptance: (required) the
popup always shows the correct theme when opened; (best-effort) a Settings change updates an open
popup live where the channel exists.
**CSP compatibility (resolves B1) — corrected factual statement:** the checked-in Tauri CSP in
`crates/copypaste-ui/src-tauri/tauri.conf.json` is
`script-src 'self'` (verified: no `'unsafe-inline'`, no nonce, no hash on `script-src`; only
`style-src` carries `'unsafe-inline'`). An **inline** `<script>` is therefore **blocked** — `'self'`
does not authorize inline scripts. The prior draft's claim that inline is CSP-compatible was wrong.
Resolution: the bootstrap is an **external same-origin classic script** at the **relative** path
`./theme-bootstrap.js` (authorized by `script-src 'self'`), authored as a tiny source file at
`crates/copypaste-ui/public/theme-bootstrap.js` (Vite emits `public/` verbatim to a stable un-hashed
path) and referenced by both `index.html` and `popup.html` before the module entry. **The path is
normatively RELATIVE (`./`), not root-absolute (`/`) — one contract, no ambiguity (resolves round-6
B3):** relative resolution is safe under Tauri's packaged asset protocol (where the origin differs
from Vite dev), and the packaged-Tauri smoke (below / task 1.15) is the proof that the URL resolves
in BOTH windows. We do **not** weaken CSP to `'unsafe-inline'`
just to avoid a theme flash. Verification is a **packaged-Tauri** test (not only a Vite dev-mode
test), because dev and packaged builds enforce materially different CSP — the packaged app must load
`./theme-bootstrap.js` and apply `data-theme`/`data-accent`/`data-translucency` before first paint
without a CSP violation in the packaged runtime.
**Rationale (resolves B1):** a React `useEffect` runs after the browser paints, so a user with a
persisted `light` theme would see one frame (or more, depending on hydration timing) of the static
dark theme baked into `index.html`/`popup.html` before the effect corrects it — a visible flash and
a spec contradiction (`design-tokens` spec already required pre-paint application). The external
same-origin bootstrap script removes that gap by applying the real preference before any content is
visible, while staying within `script-src 'self'`.
**Alternative considered:** a `<meta>`-driven or CSS-only "no-flash" trick (e.g. `visibility:hidden`
until a class is added). Rejected — it still requires the same synchronous read/validate/write logic
and adds an extra visibility toggle to get right cross-browser; the external bootstrap script is simpler and
directly sets the attributes the CSS already keys off.

### 5. Shared `Dialog` primitive composes existing focus-trap/portal behavior — not new scope
**Decision:** `ConfirmModal`, `SasPairingModal`, `RevokeConfirmDialog`, and `DetailsModal` all
compose one `Dialog` primitive (`src/lib/dialog/Dialog.tsx`) with this contract:
- Portal to `document.body` (as `ConfirmModal` already does via `ReactDOM.createPortal`).
- `role="dialog"` + `aria-modal="true"`, with caller-supplied `aria-labelledby`/`aria-describedby`
  ids wired to the title/body elements (as `ConfirmModal` already does).
- Initial focus: the first focusable descendant, falling back to the container itself with
  `tabindex="-1"` (exactly `useFocusTrap`'s existing behavior — unchanged).
- Focus trap: Tab/Shift+Tab cycle within the dialog (exactly `useFocusTrap`'s existing behavior).
- Escape-to-dismiss and backdrop-click-to-dismiss, configurable per dialog (destructive dialogs may
  disable backdrop-dismiss if the design calls for an explicit choice; `ConfirmModal` today enables
  both).
- Focus restoration: refocus the trigger element on close (exactly `useFocusTrap`'s existing
  cleanup behavior — unchanged).
- Scroll lock on the underlying view while open (new: not currently implemented; added to the
  primitive so all four dialogs get it uniformly instead of each hand-rolling it or omitting it).
**What is existing vs. newly wired (resolves B3/C2):** focus trap, initial focus, Tab cycling,
Escape dismissal, and focus restoration **already exist** in `useFocusTrap.ts` and are **already
composed** by `ConfirmModal`. This change does not invent that behavior; it (a) extracts the
`role`/`aria-modal`/portal/backdrop-dismiss wiring that today lives inline in `ConfirmModal` into a
reusable `Dialog` primitive so `SasPairingModal`/`RevokeConfirmDialog`/`DetailsModal` compose the
same contract instead of re-implementing subsets of it, and (b) adds scroll-lock, which is genuinely
new. **Scroll-lock is genuinely new and MUST be stack-safe / reference-counted** — a shared counter
so nested/stacked dialogs don't let one dialog's cleanup restore `body` overflow while another
overlay is still open. Everything else in the Dialog primitive is (re)wiring of existing hooks; the
full list of genuinely-new behaviors across the change is in Goals/Non-Goals (resolves M4).

**Per-dialog compatibility matrix (resolves M5)** — capture current-vs-target BEFORE extracting the
primitive, so consolidation is behavior-preserving:

| Dialog | Portal | Escape | Backdrop dismiss | Initial focus | Pending/async close | Scroll-lock |
|---|---|---|---|---|---|---|
| `ConfirmModal` | `createPortal(body)` (existing) | yes (existing) | yes (existing) | first focusable (existing) | n/a | target: shared ref-counted |
| `SasPairingModal` | verify current | verify | verify | verify | pairing in flight → verify current | target: shared ref-counted |
| `RevokeConfirmDialog` | verify current | yes (5917.9) | verify | verify | `revokeBusy` pending (existing) | target: shared ref-counted |
| `DetailsModal` | verify current | verify | verify | verify | n/a | target: shared ref-counted |

"verify current" = read the file and record actual behavior before migration; the target column is
the unified `Dialog` contract. No dialog's observable behavior may regress (Escape/backdrop/focus
restoration/pending) — only the implementation is shared.

### 6. Preview gallery: DEV-only dynamic import, not a reachable production `ViewId`
**Decision:** `ProductionViewId = "history" | "devices" | "settings" | "about" | "logs"` **remains
the store's and app's view type, unchanged**. The gallery is **NOT** added to the store's `view`,
and **no `DevViewId` is introduced into `store.ts`** (no exported `DevViewId`, no `"gallery"` member
in the store union). Gallery selection is a **local, non-store dev-only navigation state**
(a `DEV && MOCK`-gated local flag / `GallerySelection` inside `App.tsx`, or a `?view=gallery` URL
check) — it never becomes first-class app state. `App.tsx`'s view registry
stays a `Record<ProductionViewId, …>` — the gallery is **never** a static entry in it. Instead, when
`import.meta.env.DEV && MOCK` is true and the current view is `"gallery"`, `App.tsx` renders a
component obtained via `const { GalleryView } = await import("./views/GalleryView")` (a dynamic
import, mirroring `transport.ts`'s existing `await import("../mockIpc")` pattern exactly), gated
behind the same `DEV`/`MOCK` check that already tree-shakes `mockIpc.ts` out of production.
**Factual correction (resolves B3): `view` is NOT persisted.** The Zustand `view` field is in-memory
only — only `UIPrefs` is written to `localStorage` (verified: `store.ts` has no `persist`/`partialize`
of `view`). So there is no "downgrade leaves `gallery` persisted" case to recover from; that premise
was false. The narrowing we keep is purely **defensive runtime validation of in-memory / URL-derived
`view` input**: `setView(v)` accepts `"gallery"` only when `import.meta.env.DEV && MOCK`, otherwise it
falls back to `"history"` — this guards against an invalid value arriving from code or a `?view=`
query param, not from persistence. **`DevViewId` does not pollute the production store type**: rather
than typing the global store's `view` as `DevViewId`, the gallery is a **dev-only navigation branch**
— production `view` stays `ProductionViewId`; the DEV/MOCK gallery path is handled outside the
production `Record<ProductionViewId, …>` so no production state type gains a `"gallery"` member.
**Verification beyond a string match (resolves B2):** in addition to the existing bundle-content
string assertion (task 8.7's `rg` check), the build verification also inspects the emitted Rollup
chunk graph (`vite build --mode production` then reading the generated manifest/chunk list) to
confirm no chunk is reachable from the production entry that contains the gallery module's file
path — a unique-string grep alone can pass by accident (e.g. if the string also appears in an
unrelated comment) and can't prove *reachability* is actually severed.
**Alternative considered:** keep `"gallery"` in the single production `ViewId` and hide the nav
item. Rejected per the review — hiding a nav item controls reachability from the UI, not
whether `GalleryView` and its fixtures are still statically imported into the production module
graph by `App.tsx`'s `Record<ViewId, …>`.

### 7. Gallery structure, fixtures, and forced-state testability
**Decision:** The gallery is organized as: (a) canonical component/state sections (one section per
primitive/pattern, each showing its documented states inline), (b) a local theme/accent/translucency
switcher (component state only, not `setPrefs` — see the "gallery never writes real prefs"
requirement, unchanged from the prior draft) that live-updates the whole gallery without navigating
away, and (c) a compact, separate "token/critical-component matrix" section that renders the full
12 theme×accent combinations only for a small set of critical components (button, card, focus ring,
status banner) — not by rendering twelve complete interactive app copies (resolves G2). Every
section has a deterministic `id` (e.g. `#gallery-buttons`, `#gallery-history-row`) for stable
deep-linking from automated tests.
**Forced-state testability (resolves G1):** native pseudo-states (`:hover`, `:active`,
`:focus-visible`) cannot be persistently rendered as static examples. The gallery uses **both**
mechanisms: (1) debug-only forced-state classes/attributes (e.g. `data-force-state="hover"`) with a
CSS parity test asserting the forced-state selector produces the same computed styles as the real
pseudo-class, and (2) Playwright interaction screenshots (`hover()`, mouse-down, keyboard focus) for
the automated visual suite (Decision 13) — "hover-capable" alone is not an acceptance criterion.
**Shared fixtures (resolves G3):** gallery sample data and mock-IPC sample data are both produced by
the same typed fixture factories (e.g. `makeHistoryEntry(overrides)`, `makeDevice(overrides)`) under
`src/lib/fixtures/`, with per-story overrides for gallery-specific states (long text, secret,
unknown kind). These factories (and any secret-looking sample values they produce) are DEV-only and
excluded from the production bundle by the same dynamic-import gate as the gallery itself — the
production build's chunk-graph check (Decision 6) also covers the fixtures module.
**Gallery isolation (resolves A6):** the gallery renders inside a scoped
`.theme-scope[data-theme][data-accent][data-translucency]` wrapper, not by mutating `<html>`. This
requires the `design-tokens` token layer's selectors to resolve on **both** `:root[data-theme=…]`
and `.theme-scope[data-theme=…]` (and likewise for `data-accent`/`data-translucency`) — the tokens
layer is written so every themed custom-property block is scoped to `:is(:root, .theme-scope)`
rather than `:root` alone, so a nested wrapper can render a different theme/accent than the real
app without touching `<html>` or needing cleanup-on-unmount logic.
**Devcard (resolves F6):** `.devcard`/`.dmeta` (the grid-card device variant not used by
`DevicesView`, see Decision 12 device-shape note) is documented only in the `component-library` spec
and rendered only in the gallery's reference section behind `gallery.css` (Decision 2) — it does not
ship in the production stylesheet, and no production component references it, avoiding a
speculative API under the DRY mandate.

### 8. Clipboard content-kind normalization is shared between History and Popup
**Decision:** Introduce shared units under `src/lib/clip/` used by **both** `HistoryRow` and
`PopupRow`, which remain separate layout wrappers (different row heights/anatomy) composed from:
- `normalizeContentKind(entry: HistoryEntry): NormalizedKind` — handles case-insensitive matching,
  known aliases, precedence (`kind` wins over `content_type` when both are present and disagree,
  since `kind` is the daemon's refined text-kind classification per its doc comment; `content_type`
  is used only when `kind` is absent), the `PATH`/`FILE` → shared `file` token mapping and
  `PHONE`/`NUMBER` → shared `num` token mapping, and a `"unknown"` fallback for any value (including
  `undefined`, since `HistoryEntry.kind` is `kind?: string`, not a closed union) that isn't
  recognized. An entry whose `content_type` indicates an image MIME type but whose `kind` is absent
  or contradictory normalizes to `"image"` (MIME type wins for the image case specifically, since
  it's the more reliable signal when `kind` disagrees).
- A typed `KIND_PRESENTATION: Record<NormalizedKind, { token: string; Icon: LucideIcon; label:
  string }>` map, including an explicit `unknown` entry (generic file-glyph icon, `--dim` token,
  label "Unknown").
- `ContentTile` — renders the glyph/swatch/thumbnail for a normalized kind.
- `ClipPreview` — renders the single-line preview, including the sensitive-masked state (Decision 9
  below defines exactly what masking does and does not hide).
- `ClipMetadata` — renders the `kind · sourceApp · relTime · originDevice` meta line, including the
  source-app fallback (see the source-app data contract note below).
**Tests:** unit tests for `normalizeContentKind()` cover an unknown string, `undefined`, a future
hypothetical kind value, `PATH`/`FILE` and `PHONE`/`NUMBER` aliasing, and the image-MIME-with-absent-kind
case.
**Source-app icon data contract (resolves C5):** today's `HistoryEntry` has no source-app icon
field at all (daemon `source_bundle_id` does not exist yet — explicitly out of scope, see
Non-Goals). `ClipMetadata` therefore always renders the **generic fallback** (a type-glyph, not a
per-app icon) for the source-app slot in this change; the slot's layout space is reserved on every
row unconditionally (not conditionally per row) so that a future daemon change wiring a real icon
requires no second layout pass. The fallback glyph carries an accessible label of the source app
name text already available today (`entry.sourceApp` or equivalent existing field), not a generic
"unknown app" string, so screen readers still get the real app name even without an icon.
**Rationale (resolves A3/A4):** `HistoryRow`/`PopupRow` sharing only the `.row` CSS class today
would leave kind→icon/token/label mapping, secret masking, and source-app fallback duplicated in
two files, guaranteed to drift. Centralizing the mapping (not the layout) keeps the two rows free to
have genuinely different anatomy while removing the actual duplication risk.
**Gallery coverage:** the gallery demonstrates all 11 design-reference kinds (text/url/email/
phone/code/json/number/color/path/file/secret) plus one `"unknown"` example; each gallery example is
annotated with whether that kind is currently backend-reachable (i.e., whether the daemon's `kind`
enum in `copypaste-core`/`copypaste-ipc` actually emits it today) so gallery-only kinds are visibly
labeled as design-canvas-only, not implied to be live.

### 9. Sensitive content: visual-blur-only masking, explicit security contract
**Decision:** Masking sensitive clipboard content is **visual blur only** — a CSS-level
presentation state, not a data-redaction mechanism. Explicitly, while masked:
- **Copy/paste works identically to a normal item.** The copy action reads from the item's
  in-memory/IPC data (the same path a normal item's copy uses), never from the visually-blurred DOM
  text, so masking never degrades or blocks the core clipboard-manager function.
- **The accessible name is masked** — this is a genuine fix, not merely a preserved behavior: today
  a masked row's `aria-label`/accessible text can still expose the plaintext even though the pixels
  are blurred (P0 gap, `A11Y-1`). After this change, the accessible name for a masked, unrevealed
  secret is a placeholder (e.g. "Sensitive item, hidden — activate to reveal"), never the plaintext,
  until the user reveals it (at which point the accessible name updates to the real content,
  matching the now-visible pixels). **Mechanism** (so this coexists with "selection unrestricted"
  below): the real text node stays in the DOM (so it remains selectable) but is marked
  `aria-hidden="true"`, and the row container carries an explicit `aria-label` holding the
  placeholder while masked / the real value once revealed. The accessible *name* therefore comes
  from the container's `aria-label`, not the hidden text node — masking the name without removing the
  selectable text.
- **No length masking.** The blurred span occupies the item's real rendered width — this is an
  intentional, accepted trade-off: it leaks approximate content length/shape, in exchange for stable
  row layout (a fixed-width mask would either truncate long secrets misleadingly or force
  variable-height rows). This trade-off is explicit product/security guidance from this decision, not
  an oversight.
- **Text selection is unrestricted** while masked — the user can select/copy the underlying text via
  normal browser selection even before clicking "reveal," consistent with "masking is presentational,
  not a security boundary against a user who already has clipboard access to their own history."
- **Auto-re-mask on window blur** — already implemented in `useSensitiveReveal` (window `blur` event
  resets `revealed` to `false`); this change does not alter that behavior.
- **Optional reveal timeout — full contract (kept per user directive, fully specified, resolves M3):**
  a new opt-in `UIPrefs` field `sensitiveRevealTimeoutSec: number` (additive field, same no-migration
  rules as Decision 10). **Allowed values:** integer `0`–`300`; **default `0` = disabled** (the
  blur-based re-mask remains the primary safeguard; the timeout is defense-in-depth). **Validation:**
  non-number / out-of-range → clamp to `[0,300]` (or default `0` if not a finite number), per the
  Decision-10 per-field validation. **Settings UI:** a labeled control in `DisplayTab.tsx`'s Privacy
  group (a slider or number field, `0` shown as "Off"). **Timer lifecycle:** when `> 0` and an item
  is revealed, start a per-item timer; **reset it on any interaction with that item** (hover, focus,
  scroll into the item); on expiry, re-mask that item; **clear on unmount, on manual re-mask, and on
  the existing window-blur re-mask** (whichever comes first). **Multi-item:** each revealed item owns
  an **independent** timer — revealing item B does not reset item A's timer, and re-masking is
  per-item, not global.
**Rationale (resolves X6):** the prior draft left "does masking leak information" as an implicit
assumption. This decision makes the security posture explicit and testable: DOM inspection,
screenshots, and browser selection can all reveal the underlying value while masked — that is
accepted, because copy/paste (the actual sensitive operation) is unaffected, and the alternative
(hiding the value from the DOM entirely) would break "click to reveal" without a data round-trip.
The one hard requirement carried over unchanged from the prior draft is that the value is never
copied from the visually-masked DOM text (which could contain a truncated/placeholder string) — it
is always copied from the real item data.

### 10. Persistence: additive fields on the existing prefs object — no migration, no back-compat
**Decision (per user directive — backward compatibility is explicitly NOT required; previous
versions are not supported):** `theme`, `accent`, and `translucency` are added as **additive
fields** to the existing `UIPrefs` object at its **current** key `copypaste-ui-prefs-v4`. There is
**no version bump, no legacy-key migration chain, no dual-write, and no downgrade handling.** The
existing `loadPrefs()` whitelist-merge-with-defaults (`{ ...DEFAULT_PREFS, ...parsed }`) already
supplies the three new fields' defaults for any stored blob that predates them, so an older stored
prefs object simply gains the new fields at their defaults on next load — there is nothing to
migrate. Stale appearance keys from the deleted Liquid-Glass era are already dropped by the existing
`knownKeys` whitelist and stay dropped. (Keeping the current key rather than bumping to a new one is
deliberate: with no back-compat requirement, a version bump would only add migration code for no
benefit.)
**Existing legacy migration is REMOVED (explicit user decision, resolves round-5 B2):** `store.ts`
today reads `copypaste-ui-prefs-v3`/`v2`/`v1` and forwards them to v4. Per the "no back-compat /
don't care about previous versions" directive, **these v1/v2/v3 legacy-key migration branches are
deleted** in this change. **Accepted impact, stated up front:** a user whose prefs are still under an
old v1–v3 key (never re-saved under v4) loses ALL their UI prefs (not just appearance) — they reset
to defaults. This is intentional cleanup, not an oversight; it is called out as its own task so an
implementer removes the branches deliberately rather than by inference. The `v4` key itself remains
the single current key.
**Runtime validation (robustness only, NOT migration):** `loadPrefs()` validates the three new
fields per-field so a corrupt stored value can never reach the DOM, each defaulting independently
without discarding the others: `theme` must be `"dark"|"light"` (else `"dark"`), `accent` one of the
six accent values (else `"indigo"`), `translucency` a boolean (else `true`). Tests: malformed JSON →
full `DEFAULT_PREFS`; unknown keys → dropped; each new field individually invalid → that field
defaults while the others are kept; normal reload round-trips. **No migration-path tests — there is
no migration.**
**Downgrade / rollback:** out of scope by directive. The only rollback concern is a `git revert` of
this change's code, which needs no data handling. If a stored blob is ever read by unrelated code
that doesn't know the new fields, the whitelist-merge simply ignores them — no corruption.

### 11. Token source of truth: exact name-and-value parity test
**Decision:** In addition to copying token values from `copypaste-design-reference.html` verbatim
into `src/styles/tokens.css` (unchanged from the prior draft's approach), add an automated test that
parses both files' token blocks and asserts **every custom property name resolves to the identical
value** in both files — not merely that the same set of names exists in both. The test fails loudly
on any future edit to either file that isn't mirrored in the other.
**Rationale (resolves S4):** a name-only diff (e.g. "both files define `--accent`") cannot catch a
value drifting in one file and not the other, which is exactly the silent-drift risk copying verbatim
was meant to prevent. The prior draft's claim that the two files "can never silently drift" was true
only if some executable check enforces it — this decision adds that check rather than relying on
manual verbatim-copy discipline alone.

### 12. Non-token CSS values: split pixel policy; typography and layout tokens
**Decision (resolves S1):** "No hardcoded pixel/color/duration values outside tokens" applies to
**design constants** (anything a designer would specify once — spacing, radii, shadow, focus-ring
width/offset, hairline width, icon sizes, control heights) — these MUST be tokens, and new explicit
tokens are added for focus-ring width/offset (`--focus-ring-width`, `--focus-ring-offset`), hairline
width (`--hairline`), icon sizes (`--icon-sm/md/lg`), and control heights (`--ctl-h-sm/md/lg`).
It does **not** apply to **runtime-computed geometry** — virtualized-list item offsets, the popup
row's dynamically measured height, the glide-highlight overlay's computed `top`/`left`/`width`, and
the settings tab-bar's measured underline position — these are legitimately expressed as inline
`style`/CSS custom properties set from JS, because their value is not known until layout/measurement
time. Structural values (`0`, percentages, fractions, `transform: translate(...)` expressed in
`%`/`px` derived from measurement) are allowed either way.
**Typography/layout (resolves S5):** the `tokens` layer adds a typography scale (font stack(s),
weight scale, line-height scale, letter-spacing scale) and text-scaling support (relative units so OS
text-size settings and 200% zoom reflow correctly, tested explicitly — see the `preview-gallery`/
`component-library` a11y requirements), plus layout constraints: minimum main-window dimensions,
minimum/fixed popup width, and documented behavior for long/localized text (a test case using a
string ~40% longer than the English source string, representative of German/Finnish-style
expansion, confirms rows/labels/buttons reflow or truncate without breaking layout).

### 13. Verification: automated Playwright suite is a required CI gate
**Decision:** The following automated Playwright coverage is a **required** gate for this change,
not a stretch goal or manual-only check: main window and popup in both dark and light theme, the
accent/on-accent contrast matrix (critical-component subset, Decision 7), modal keyboard/focus
behavior (trap, Escape, backdrop, restore-on-close), `prefers-reduced-motion: reduce` (no visible
animation), long-text overflow (ellipsis, no row-height growth), production gallery-exclusion
(chunk-graph check, Decision 6), automated token-contrast checks (Decision "X1" below), and an
accessibility scan using **`@axe-core/playwright`** — the concrete, non-optional tool this change
adds as the **single new DEV dependency** (test-only, never shipped; recorded under Impact /
Dependencies). It is NOT "whatever tool is already approved" — the tool is named here so the gate is
real. (Critical keyboard/focus behavior additionally needs packaged-WebView coverage per the
packaged-Tauri gate below, since browser and Tauri behavior can differ.) Manual `?mock=1` walkthroughs
(`ui-verifier`-style) remain valuable as **exploratory** verification but are not, by themselves,
the completion bar — `tasks.md` slice 6 aligns with this (the former "spot-check only" language for
tasks 10.x is removed; see the rewritten task list).
**Automated contrast checks (resolves X1):** a script/test computes contrast ratios for all 12
theme×accent combinations across: normal text, large text, non-text UI boundaries/controls, focus
indicators, on-accent text/icons, status surfaces (ok/info/warn/err), and content-type metadata
tokens — against WCAG AA thresholds — and fails the build if any combination is under threshold.
Manual visual inspection is no longer the primary evidence for the AA-contrast claim.
**Focus-visible requirements (resolves X3):** in addition to the existing "2px ring" requirement,
add: minimum 3:1 contrast between the focus ring and every adjacent surface it can appear against
(tested across the contrast-check combinations above); no clipping by `overflow: hidden` in rows,
modals, tabs, or the popup (verified by the Playwright suite's keyboard-navigation pass;
scrollIntoView / overflow rules adjusted if clipping is found); a `forced-colors` media-query
fallback (`forced-color-adjust`/system colors) so the ring remains visible under Windows High
Contrast-style forced-colors modes where supported by the target WebView; logical (DOM-order) focus
order verified by the same pass; and focus restoration after modal close (already covered by
`useFocusTrap`, Decision 5).
**Hover-revealed actions (resolves X4):** row actions (pin/delete) are visible on fine-pointer
`:hover` and on `:focus-within` (so keyboard users tabbing into a row see the same controls appear);
on coarse/touch pointers (no hover capability, detected via `(hover: none)` media feature) the
actions render always-visible rather than relying on a hover that can't occur; in selection mode the
hover actions are replaced by the row checkbox (existing behavior, Decision 3's naming section);
screen-reader users reach the actions via normal DOM/tab order regardless of visual hover state.
Controls are never focusable while visually hidden (a control hidden via `visibility`/`opacity` with
no hover/focus-within trigger is also `tabindex="-1"` or removed from the tab order, avoiding
"invisible but tabbable" traps).
**Additional a11y acceptance (resolves X5):** 200% zoom/OS text-scaling reflow (Decision 12); no
required two-dimensional scrolling at that zoom level for primary content; minimum target sizes per
platform guidance, with a documented desktop-pointer exception where a control is intentionally
denser than the mobile-target minimum (e.g. compact popup rows) — the exception is named per
control, not blanket-applied; `aria-expanded`/`aria-controls` on every device-row disclosure header;
tab semantics (`role="tab"`/`role="tablist"`, arrow-key navigation) for `.set-tab`/segmented
controls; `aria-live` regions for toast and status-banner content; reduced motion verified per
animation (not just the three duration tokens — Decision below); and sensitive-content
announcements that describe state ("hidden, activate to reveal" / "revealed") without ever including
the secret value itself while masked (Decision 9).
**Existing-attribute invariant restated (resolves X2):** rather than "every `role`/`id`/`aria-*`
stays on the same element," the acceptance contract is: accessible role and name remain correct (or
are corrected, as with the sensitive-masking fix); labelled/described relationships resolve to real
elements; state (`aria-expanded`, `aria-selected`, etc.) is exposed accurately; keyboard behavior is
unchanged or improved; and `data-testid`s used by the existing test suite remain stable only where
they are an intentional, documented contract (not incidentally, on every element).
**Reduced-motion audit scope (resolves S3):** the audit disables `animation-duration`,
`animation-iteration-count`, and `transition-duration` (via the existing collapsed-duration-token
approach) **and** additionally covers: native smooth scrolling (`scroll-behavior: auto` under
reduced motion, at the element/`html` level, not left at `smooth`), any hardcoded keyframe
`animation-duration` that doesn't reference a duration token (audited and fixed to reference one),
and CSS `transform`-based transitions that might not be gated by the three named duration tokens
(e.g. the glide-highlight overlay, tab-bar underline) — each such case is enumerated and confirmed
to no-op under `prefers-reduced-motion: reduce` rather than assuming the three tokens alone cover
every animated property in the stylesheet.

### 14. Minimum platform: macOS 13+ (WKWebView/Safari 16.2) — `color-mix()` needs no fallback
**Decision:** The supported OS/WebView matrix for this change is macOS 13 (Ventura) and later,
which ships WKWebView backed by Safari 16.2+ engine — `color-mix(in srgb, …)` has been supported
natively since Safari 16.2. This is stated as a non-functional requirement (supported platform
matrix) rather than left implicit; no `color-mix()` fallback (e.g. precomputed static color
fallback layer) is required for this change. Linux daemon-only builds have no UI surface affected by
this change. Windows is frozen per `ADR-012` and out of scope.
**Packaged targets vs. build config (resolves B4):** the **product** support matrix for the packaged
desktop UI is **macOS 13+** — established from repository/release policy: `CLAUDE.md` names macOS as
the primary target, `ADR-012` freezes Windows (homebrew-only), and Linux is **daemon-only** (no Tauri
UI is shipped). The browser `?mock=1` surface is a **private dev/QA harness only** (not a shipped
product) driven by a single Chromium engine via Playwright — its engine is stated separately from the
packaged-app OS matrix and carries no public browser-compatibility contract. **Known mismatch to
reconcile (flagged, app-code not spec):** `crates/copypaste-ui/src-tauri/tauri.conf.json` currently
sets `bundle.targets: "all"` and ships Windows icons — this contradicts the macOS-only product matrix
and is NOT a support signal; a follow-up narrows `targets` to macOS (or documents why `"all"` is
retained). This change does not use the macOS-only floor to waive compatibility work while the build
config claims otherwise — it names the discrepancy explicitly.
**Rationale (resolves S2):** the prior draft assumed `color-mix()` "just works" without stating what
"works" is scoped to; stating the floor explicitly turns an assumption into a verifiable non-goal
for fallback work.

### 15. Performance budgets: measured baseline, not an unstated assumption
**Decision:** Before slice 3 (History/Popup) lands, measure and record the current (pre-change)
baseline for: popup open→first-render latency (via the existing Playwright/manual timing harness or
a new lightweight `performance.now()` instrumentation point around popup mount) and total CSS+JS
bundle size for both the main window and popup entry chunks. Acceptance criteria for the completed
change (thresholds FIXED NOW, matching `tasks.md` 1.18 — not deferred to baseline): popup-open
**p95 regression ≤ max(15%, +40ms)** over baseline (measured p50/p95 across **10 warm runs** via a
`performance.now()` mark around popup mount); **CSS gzip delta ≤ 20 KB** and **JS gzip delta ≤ 30 KB**
per entry. Only the baseline *numbers* are recorded in slice 1; the acceptance *thresholds* above are
fixed here. Any exception requires documented reviewer sign-off in the slice-6 PR. These budgets are
gated as acceptance criteria for slice 6 (final verification), not aspirational.
**Rationale (resolves A7):** the review correctly notes the popup pays for every main-window
selector in one shared stylesheet; that's an accepted trade-off (Decision 2's "one emitted
stylesheet" goal) but its cost must be measured, not assumed acceptable.

### 16. Device action semantics — grounded in the real IPC methods; presentation-only change (resolves C4/M7)
**Decision:** the device list surfaces exactly the destructive actions the daemon/IPC **already**
provides, and this redesign changes **presentation only** — it does **not** redefine what any action
does. The real actions (verified in `copypaste-ipc`): **Unpair** = `unpair_peer` (remove the
pairing); **Revoke** = `revoke_peer` (remove trust); **Revoke & rotate key** = `revoke_and_rotate`
(revoke + rotate the sync key, requires a rotate passphrase ≥ 8 chars, returns `{revoked_at,
rotated}`). The existing UI already exposes Revoke + Revoke-&-rotate via `RevokeConfirmDialog` with a
`revokeBusy` pending flag. **What each action does at the daemon (incl. offline behavior and whether
a peer is notified) is authoritative and out of scope to change here** — this table reflects current
implementation; any question is answered by the IPC method, not by this design doc.

| Device state | Unpair | Revoke | Revoke & rotate | Notes |
|---|---|---|---|---|
| Own device | — | — | — | No destructive footer (unchanged). |
| Paired peer, online | Available | Available | Available (in Revoke dialog; passphrase ≥ 8) | Danger styling; buttons call `unpair_peer` / `revoke_peer` / `revoke_and_rotate` exactly as today. |
| Paired peer, offline | Available | Available | Available | Availability is **unchanged from today** — the design does NOT newly gate actions on online state; whether an action reaches an offline peer is daemon behavior, surfaced in labels, not by hiding buttons. |
| Discovered (unpaired) | — | — | — | Only "Pair"; no trust relationship yet (unchanged). |
| Action pending | Disabled + spinner | Disabled + spinner | Disabled + spinner | Existing `revokeBusy`-style flag; while one destructive action is in flight, the row's other destructive actions also disable to prevent a racing second call. |
| Action failed | Re-enabled + inline error **(NEW presentation)** | same | same | Failed action returns to enabled with a uniform inline error message; no silent retry. Error text/behavior source = the IPC error, unchanged. |

**Existing vs. changed:** the action set, their semantics, the pending flag, and offline availability
are **existing** (preserved here, grounded in the IPC methods above — NOT redefined). **New in this
redesign (presentation only):** consistent danger styling, equal-width action layout, and a uniform
inline error-state presentation for a failed action. Any change to what unpair/revoke/rotate actually
**do** is explicitly out of scope and requires daemon/domain-owner sign-off, not a design change.
This table is the acceptance criterion for C4/M7 — a discovered device showing a danger button, or an
action gated on online-state that today is not, is a bug.

## Component inventory

Resolves D2 — replaces the blanket "every file under components/views/popup" impact declaration
with a per-component classification. `U` = unchanged behavior, class-only styling; `P` = composed
from a new shared primitive; `B` = behavior changed (see decision cited); `D` = deprecated/removed;
`G` = gallery-only.

| Component | Class | Notes |
|---|---|---|
| `ActionButton.tsx` | P | Emits `.btn`/`.btn--<variant>` (Decision 3). |
| `Toggle.tsx` | P | `.toggle`/`.off` primitive. |
| `SectionHeader.tsx`, `Panel.tsx`, `SettingsRow.tsx`, `SliderRow.tsx` | U | Class-only restyle. |
| `SyncStatusChip.tsx`, `DeviceBadge.tsx`, `FileChip.tsx` | U | Class-only restyle onto `.chip`/`.badge`/`.tpill`. |
| `HistoryRow.tsx` | P | Composes `ContentTile`/`ClipPreview`/`ClipMetadata` (Decision 8). |
| `PopupRow.tsx` | P | Same shared units, condensed layout wrapper (Decision 8). |
| `HistoryView/VirtualList.tsx` | U | Class-only; runtime-computed geometry stays inline style (Decision 12). |
| `BulkActionBar.tsx` | U | Class-only. |
| `EmptyState.tsx` | U | Class-only; ONE component API used across **7 app empty-state contexts**: **2 History** (no items, no search results), **1 Devices** (no devices paired), **4 Popup** (offline, starting-up, no matches, nothing copied yet). The "4 states" refers to the Popup only — all 7 contexts share `EmptyState`'s props. |
| `DetailsModal.tsx` | P | Composes `Dialog` primitive (Decision 5). |
| `ConfirmModal.tsx` | P | Composes `Dialog` primitive; existing focus-trap/portal behavior unchanged. |
| `SasPairingModal.tsx` | P | Composes `Dialog` primitive. |
| `RevokeConfirmDialog.tsx` | P | Composes `Dialog` primitive. |
| `DeviceCard.tsx` (`ThisDeviceCard`/`PeerRow`) | B | `.devrow`/`.cfields`; **IPC/action semantics and action availability are PRESERVED** (unpair/revoke/revoke_and_rotate, offline availability unchanged) — only presentation (danger styling, equal-width layout) and the uniform inline error-state wiring change (Decision 16). No business-logic change. |
| `DiscoveredRow.tsx` | U | Class-only; no destructive actions (Decision 16). |
| `TabBar.tsx` | B | Adds `role="tab"`/`role="tablist"`/arrow-key nav (Decision 13, X5). |
| `Sidebar.tsx` | B | Adds Gallery item gated on `DEV && MOCK` (Decision 6). |
| `AboutView.tsx`, `LogView.tsx` | U | Class-only. |
| `App.tsx`'s banners, `AccessibilityBanner.tsx`, `StatusBanners.tsx`, `CloudAccountMismatchBanner.tsx` | U | Class-only onto `.banner`. |
| `ErrorBoundary.tsx` | U | Class-only onto `.empty`-style block. |
| `ViewShell.tsx` | U | Class-only. |
| `Toast.tsx` | B | Adds `aria-live` region (Decision 13, X5). |
| `GlideHighlight.tsx` | U | Class-only; runtime-computed geometry stays inline style/CSS var (Decision 12). |
| `HighlightedText.tsx` | U | Class-only. |
| `DisplayTab.tsx` | B | Rebuilds Appearance section: Theme + Accent + Translucency, bound to `UIPrefs`. |
| `store.ts` | B | `UIPrefs` additive fields + per-field validation (no migration). `view` stays `ProductionViewId` (in-memory, NO `DevViewId`, no `"gallery"`) — gallery is a local dev-only nav branch (Decisions 6, 10). |
| `index.html`, `popup.html` | B | Pre-paint bootstrap script added (Decision 4). |
| `GalleryView/*` (new) | G | DEV-only, dynamic-import gated (Decision 6). |
| Shared fixture factories (new) | G | DEV-only, shared with `mockIpc.ts` (Decision 7). |
| `Dialog` primitive (new) | P | Composed by all four modal components (Decision 5). |
| `normalizeContentKind()`/`ContentTile`/`ClipPreview`/`ClipMetadata` (new) | P | Shared by `HistoryRow`/`PopupRow` (Decision 8). |
| `.devcard`/`.dmeta` (grid variant) | G | Documented, gallery-only; no production consumer (Decision 7/15 in Non-Goals). |

## Risks / Trade-offs

- **[Risk] Token transcription drift between the reference HTML and the app's `tokens.css`.**
  → Mitigation: verbatim copy plus the automated name-and-value parity test (Decision 11).
- **[Risk] Adding `theme`/`accent`/`translucency` to `UIPrefs` could let a corrupt stored value
  reach the DOM.** → Mitigation: per-field runtime validation defaulting each field independently
  (Decision 10) plus tests for malformed JSON and each field invalid. No NEW migration chain is added
  (the new fields are additive with defaults); the existing v1/v2/v3→v4 legacy migration branches are
  intentionally **removed** (Decision 10/B2), so a user still on an old v1–v3 key resets to defaults —
  a documented, accepted no-back-compat impact, not a corruption bug.
- **[Risk] `.devrow`/`.cfields` selection (Decision "device shape", carried from the prior draft)
  means `.devcard`/`dev-grid` is spec'd but never rendered in the real app** — a future redesign of
  `DevicesView` into a grid would need to re-derive wiring. → Mitigation: documented in the component
  inventory as gallery-only (`G`), not silently dropped.
- **[Risk] Popup window pays CSS cost for every main-window selector via one emitted stylesheet.**
  → Mitigation: measured performance budget, Decision 15, not an unstated assumption.
- **[Risk] Reduced-motion / a11y regressions when re-adding animation.** → Mitigation: the audited,
  enumerated scope in Decision 13 (not just the three duration tokens).
- **[Risk] Cross-window live theme sync (Decision 4) is genuinely new state-sync code — the one
  piece of this change that is not "restyling already-existing behavior."** → Mitigation: two
  mechanisms (storage event + Tauri emit) so either window topology (shared or partitioned
  `localStorage`) is covered, plus an explicit two-window acceptance test.
- **[Trade-off] Sensitive-content masking accepts length/shape leakage and DOM/selection
  inspectability while masked**, in exchange for stable layout and unblocked copy/paste. → Accepted
  and stated explicitly, Decision 9 — not an oversight.
- **[Trade-off] Downgrading past this change's release resets Theme/Accent/Translucency to
  defaults.** → Accepted and documented in the release notes, Decision 10 — not claimed to be
  zero-cost.

## Migration Plan

1. **Slice 1** — land `src/styles/{reset,tokens,base}.css` with cascade layers, the pre-paint
   bootstrap script, and the `UIPrefs` additive fields + per-field validation. Verify: app still
   builds/renders (mostly unstyled beyond base/reset, since primitives/patterns land in slice 2+);
   existing stored prefs gain the new fields at defaults (no migration); invalid stored values
   default per-field; bootstrap-before-paint test passes; the popup reads the same persisted prefs
   at open (cross-window channel settled — Decision 4); performance baseline recorded (Decision 15).
2. **Slice 2** — land `primitives.css` plus the typed React primitives (`ActionButton`, `Toggle`,
   `Dialog`, disclosure header) and their a11y foundations. Verify: primitives render correctly in
   the app — which now also gains the **minimal DEV+MOCK gallery shell** (primitive stories + the
   early production-exclusion chunk-graph check, task 2.13/S2), so components are visually
   inspectable from slice 2 on; `Dialog` a11y contract tests pass (focus trap/restore/Escape/
   backdrop, reusing `useFocusTrap`'s existing test coverage plus new scroll-lock tests).
3. **Slice 3** — land `patterns.css` for History + Popup, the shared clipboard-presentation units
   (Decision 8), and the sensitive-masking contract (Decision 9). Verify: all 11 kinds + unknown
   render correctly in both row layouts; masking a11y fix (accessible name) verified; History/Popup
   remain independently usable.
4. **Slice 4** — land Devices, wiring the behavior/state table (Decision 16). Verify: no state
   renders an invalid destructive action; pairing/revoke modals compose `Dialog`.
5. **Slice 5** — land Settings (incl. the rebuilt `DisplayTab.tsx` Appearance section: Theme/Accent/
   Translucency), Sidebar, About, Logs, banners, and Toast. Verify: cross-window live theme sync
   (Decision 4) works with Settings open in one window and the popup in another.
6. **Slice 6** — land the gallery (DEV-dynamic-import gated, Decision 6/7) and the automated
   Playwright suite (Decision 13). Verify: production build's chunk-graph check confirms gallery
   absence; full automated suite green; performance budgets (Decision 15) met against the slice-1
   baseline.
Each slice's PR/commit keeps the app in a buildable, runnable state (resolves D3) — screenshot
evidence is captured per slice, a minimum-window-size and popup smoke check is run per slice, and
any prefs validation-fallback is logged (console warning when a stored field is invalid and defaults)
so a real-world bad-value path is observable rather than silent.

## Open Questions

None remaining. All nine items the user resolved, plus the additional review findings this document
self-resolves, are captured as normative Decisions above (see Decisions 1–16). The performance
acceptance **thresholds are fixed** (Decision 15: p95 ≤ max(15%, +40ms), CSS gzip ≤ 20 KB, JS gzip ≤
30 KB); only the pre-change baseline *numbers* remain to be measured in slice 1 — that is a task, not
an open threshold question. Nothing here is left as a blocking open question for the user.
