Six build-independent delivery slices (design.md Decision 1). Each slice is expected to compile,
pass its own tests, and leave the app in a usable state before the next slice starts. Slice
boundaries match the `design-tokens` / `component-library` / `preview-gallery` capability specs.

## Slice 1 — Tokens, cascade layers, pre-paint bootstrap, `UIPrefs` additive fields

- [ ] 1.1 Create `crates/copypaste-ui/src/styles/reset.css`, `tokens.css`, `base.css`,
      `primitives.css`, `patterns.css`, `shell.css`, `utilities.css` (empty placeholders except
      `reset`/`tokens`/`base`, filled in later slices) and `src/styles/index.css` that `@import`s
      them in order and declares `@layer reset, tokens, base, primitives, patterns, shell,
      utilities;` up front (design.md Decision 2).
- [ ] 1.2 Populate `tokens.css`'s `@layer tokens` with the `:root`/`:root[data-theme="dark"]`/
      `:root[data-theme="light"]` blocks copied from `copypaste-design-reference.html` (surfaces,
      lines, text, overlays, status tokens), plus every selector duplicated under
      `.theme-scope[data-theme="…"]` so the gallery's scoped wrapper (slice 6) resolves the same
      tokens without mutating `<html>` (design.md Decision 7 gallery isolation).
- [ ] 1.3 Add the 6 `:root[data-accent="…"]` / `.theme-scope[data-accent="…"]` blocks plus the
      light-theme accent-value overrides, verbatim from the reference file.
- [ ] 1.4 Add a `data-translucency="on"|"off"` axis: on `.theme-scope`/`:root`, a `--scrim`/frost
      recipe applied only to chrome surfaces (sidebar, popup container, modal scrim, toast, tab
      bar) via `backdrop-filter`; content surfaces (cards, rows, fields) always solid; `off` (or
      `backdrop-filter` unsupported, or `prefers-reduced-transparency: reduce` where the WebView
      supports that media feature) falls back to fully solid tokens for the chrome surfaces too.
- [ ] 1.5 Add content-type tokens (`--c-text`, `--c-url`, `--c-mail`, `--c-num`, `--c-code`,
      `--c-json`, `--c-color`, `--c-file`, `--c-image`, `--c-secret`) for both themes.
- [ ] 1.6 Add spacing (`--s-1`…`--s-9`), radius (`--r-chip/pill/ctl/input/card/window`), shadow
      (`--sh1/2/3`, themed), typography (font stack(s), weight scale, line-height scale,
      letter-spacing scale — design.md Decision 12/S5), and motion (`--dur-fast`/`--dur`/
      `--dur-theme`/`--ease`) tokens, plus explicit tokens for focus-ring width/offset
      (`--focus-ring-width`, `--focus-ring-offset`), hairline width (`--hairline`), icon sizes
      (`--icon-sm/md/lg`), and control heights (`--ctl-h-sm/md/lg`) — design.md Decision 12 (S1).
      Include the `@media (prefers-reduced-motion: reduce)` override collapsing `--dur*` to `0ms`.
- [ ] 1.7 Add layout-constraint tokens/documentation: minimum main-window dimensions, popup
      width, and a note on long/localized-text handling (design.md Decision 12/S5).
- [ ] 1.8 Add a script/test asserting **exact name-and-value parity** between
      `copypaste-design-reference.html`'s token block and `tokens.css` (design.md Decision 11) —
      not a name-only diff.
- [ ] 1.9 Add the `2-base` layer (`@layer base`) — box-sizing reset, body/svg/button/input/link/
      focus-visible/selection/scrollbar base rules — from the reference file's Layer 2.
- [ ] 1.10 In `crates/copypaste-ui/src/store.ts`: add `theme: "dark" | "light"`,
      `accent: "indigo" | "blue" | "teal" | "green" | "amber" | "rose"`, and
      `translucency: boolean` to `UIPrefs` as **additive fields** on the current key
      `copypaste-ui-prefs-v4` — NO version bump, NO migration chain, NO dual-write, NO downgrade
      handling (design.md Decision 10; back-compat out of scope). The existing
      `{ ...DEFAULT_PREFS, ...parsed }` merge supplies the new fields' defaults for older blobs.
      Also split `ViewId` into `ProductionViewId`/`DevViewId` (design.md Decision 6, used from slice 6).
- [ ] 1.11 Add per-field runtime validation to the loader (design.md Decision 10): `theme`
      must be `"dark"`/`"light"` else default; `accent` must be one of the 6 known values else
      default; `translucency` must be `boolean` else default `true`; an invalid field never
      discards other valid fields. Log a console warning whenever a validation-fallback path is
      taken.
- [ ] 1.12 Add validation tests (NO migration tests — there is no migration): malformed JSON →
      full `DEFAULT_PREFS`; unknown keys dropped; each new field individually invalid → that field
      defaults while other valid fields are kept; a blob predating the fields → fields default in;
      normal reload round-trips (design.md Decision 10).
- [ ] 1.14 Add the synchronous pre-paint bootstrap `<script>` (not `type="module"`) to both
      `index.html` and `popup.html`: reads `localStorage["copypaste-ui-prefs-v4"]` defensively
      (try/catch, missing/malformed → defaults), validates each of `theme`/`accent`/
      `translucency` independently, and sets `document.documentElement.dataset.theme`/`.accent`/
      `.translucency` before any deferred/module script runs (design.md Decision 4/B1). Delete
      the stale `data-palette`/`data-density`/`data-motion`/`data-contrast` attributes and their
      comment trails from both HTML files.
- [ ] 1.15 Verify the bootstrap script's compatibility with the app's Tauri CSP configuration
      (same-origin inline script, no `eval`/`Function`, no external fetch) and record the
      verification (design.md Decision 4).
- [ ] 1.16 In `src/App.tsx` and `src/popup/Popup.tsx` (or their `main.tsx`), add a `useEffect`
      that re-applies `prefs.theme`/`.accent`/`.translucency` to `<html>` on mount and on change
      (live updates after the bootstrap has already handled first paint).
- [ ] 1.17 First VERIFY whether the main window and popup share one `localStorage` partition in
      Tauri (design.md Decision 4/A5). Guaranteed behavior: the popup applies persisted prefs on
      every open (reads `copypaste-ui-prefs-v4` at mount). On top of that, implement BEST-EFFORT
      live sync ("updates as it can", not a hard gate): emit/listen for a Tauri `ui-prefs-changed`
      event (the reliable channel if partitions are separate) and, where the partition is shared,
      also a `storage` event listener. Acceptance: with the popup open, a Settings theme change
      updates it live where a channel exists; in all cases the popup shows the correct theme after
      reopening.
- [ ] 1.18 Record the pre-change performance baseline (design.md Decision 15): popup
      open→first-render latency and CSS+JS bundle size for the main and popup entry chunks. Pick
      and record the acceptance threshold (regression cap / bundle-delta cap) alongside the
      baseline numbers for slice 6 to gate against.
- [ ] 1.19 State the supported OS/WebView matrix (macOS 13+ / WKWebView Safari 16.2+) as a
      non-functional requirement; confirm no `color-mix()` fallback is needed (design.md
      Decision 14/S2).
- [ ] 1.20 Verify: `rg` the repo for `data-palette|data-skin|data-density|data-motion|
      data-contrast` outside `docs/`/changelog and confirm zero remaining occurrences
      (STYLEGUIDE.md §12 done-check).

## Slice 2 — Typed primitives + shared Dialog/disclosure a11y foundations

- [ ] 2.1 Add the `@layer primitives` CSS section in `primitives.css`: `.btn` (+
      `--primary/--secondary/--ghost/--danger`, `.sm`, `.block`, `:disabled`), `.iconbtn` (+
      `.danger`), `.toggle` (+ `.off`), `.seg`, `.field`, `.chip` (+ `.on`, `.chip--ct`), `.tpill`
      (+ `--p2p/--cloud/--this`), `.badge` (+ `--verified/--count`), `.tile` (+
      `--swatch/--thumb`), `.dot-stat` (+ `.off` + pulse keyframes), `.card`, `.divider`,
      `.spinner`, `.kbd` — using the allowed-button-primitives list from design.md Decision 3
      (C3): `.btn` family is for standalone actions only, not tabs/icon-buttons/disclosure
      headers/chips/row-actions, each of which is its own documented primitive.
- [ ] 2.2 Wire `ActionButton.tsx` to emit `.btn .btn--<variant>` (+ `.sm` for `size="sm"`); keep
      all existing props/behavior unchanged.
- [ ] 2.3 Wire `Toggle.tsx` to the `.toggle`/`.off` classes and its knob `<span>`.
- [ ] 2.4 Wire `SectionHeader.tsx`, `Panel.tsx`, `SettingsRow.tsx`, `SliderRow.tsx` to their
      corresponding patterns (`.set-grp__h`, `.card`/panel surface, `.srow`, slider track/thumb).
- [ ] 2.5 Wire `SyncStatusChip.tsx`, `DeviceBadge.tsx`, `FileChip.tsx` to `.chip`/`.badge`/
      `.tpill` primitives as appropriate to each one's semantics.
- [ ] 2.6 Restore icons via `lucide-react` (the single normative icon source; inline SVG only as
      a documented fallback when no suitable Lucide icon exists — design.md Decision "icons"/F2)
      with explicit sizes (`--icon-sm/md/lg`) in every component touched in this section; verify
      no `<svg>` renders without an explicit width/height.
- [ ] 2.7 Build the shared `Dialog` primitive (`src/lib/dialog/Dialog.tsx`) composing the
      existing `useFocusTrap` hook: portal to `document.body`, `role="dialog"`/
      `aria-modal="true"`, caller-supplied `aria-labelledby`/`aria-describedby`, initial focus +
      focus trap + Escape + backdrop-dismiss (configurable) + focus restoration (all via
      `useFocusTrap`, unchanged), plus new scroll-lock on the underlying view while open
      (design.md Decision 5 — the one genuinely new behavior in this primitive).
- [ ] 2.8 Migrate `ConfirmModal.tsx` to compose `Dialog` (behavior-preserving refactor — its
      existing focus-trap/portal/backdrop/Escape behavior is unchanged, only the shared wrapper
      changes).
- [ ] 2.9 Migrate `SasPairingModal.tsx`, `RevokeConfirmDialog.tsx`, and `DetailsModal.tsx` to
      compose `Dialog` (design.md component inventory: `B`/`P` — behavior consolidated onto the
      shared contract).
- [ ] 2.10 Add a typed disclosure-header primitive (`aria-expanded`/`aria-controls`, no `.btn`
      styling) for expandable rows, used by Devices in slice 4 and documented in
      `component-library` spec (design.md Decision 3).
- [ ] 2.11 Add `.set-tab`/tab-list a11y foundations: `role="tablist"`/`role="tab"`, arrow-key
      navigation, wired later into `TabBar.tsx` in slice 5.
- [ ] 2.12 Add Dialog a11y tests: initial focus lands on the first focusable element (or the
      container fallback), Tab/Shift+Tab cycle correctly, Escape and backdrop-click dismiss,
      focus restores to the trigger element on close, and scroll-lock engages/releases correctly.

## Slice 3 — History + Popup via shared clipboard-presentation units

- [ ] 3.1 Add `@layer patterns` CSS for `.row`/`.row__body`/`.row__title`/`.row__meta`/
      `.row__right`, `.del`/`.star-btn`, `.chk`, `.grouphead`, `.bulkbar`, plus `filtered`/
      `removing`/`copied`/`pinned`/`sel` state classes (expressed as `data-state`/native states
      per design.md Decision 3/C1) and their keyframes.
- [ ] 3.2 Implement `src/lib/clip/normalizeContentKind.ts`: case/alias normalization, `kind`-wins-
      over-`content_type` precedence (falling back to `content_type` when `kind` is absent),
      `PATH`/`FILE`→`file` and `PHONE`/`NUMBER`→`num` mappings, `"unknown"` fallback for any
      unrecognized or `undefined` value, and the image-MIME-with-absent-kind→`"image"` rule
      (design.md Decision 8/A3/A4). Add unit tests: unknown string, `undefined`, a future
      hypothetical kind, both alias pairs, and the image-MIME case.
- [ ] 3.3 Implement the typed `KIND_PRESENTATION` map (token/icon/label per normalized kind,
      including an explicit `unknown` entry) and the shared `ContentTile`, `ClipPreview`, and
      `ClipMetadata` components (design.md Decision 8), including the source-app fallback
      contract: always render the generic type-glyph fallback (no daemon `source_bundle_id` yet),
      reserve the slot's layout space unconditionally on every row, and set the accessible label
      from the existing source-app name field (design.md Decision 8/C5).
- [ ] 3.4 Wire `HistoryRow.tsx` to `.row` + `ContentTile`/`ClipPreview`/`ClipMetadata` for all 11
      content kinds + unknown; wire `PopupRow.tsx` to the same shared units in its condensed
      layout — the two components remain separate layout wrappers (design.md Decision 8).
- [ ] 3.5 Wire `HistoryView.tsx`/`VirtualList.tsx` list container, search field, and filter chips
      to `.list`/`.field`/`.filters`/`.chip`; runtime-computed item offsets stay inline
      style/CSS-var per design.md Decision 12 (S1) — not replaced with tokens.
- [ ] 3.6 Wire `BulkActionBar.tsx` to `.bulkbar`.
- [ ] 3.7 Wire `EmptyState.tsx` to `.empty`/`.empty__ic`/`.empty__t`/`.empty__s` and verify all
      documented History empty-state call sites (no items, no search results) render correctly.
- [ ] 3.8 Wire `DetailsModal.tsx` and `HistoryView`'s bulk-delete `ConfirmModal` usage to the
      `Dialog`-backed `.scrim`/`.modal` pattern from slice 2.
- [ ] 3.9 Implement the sensitive-masking contract exactly per design.md Decision 9 (X6):
      `.mask` styling occupies the real rendered width (no length masking, documented
      trade-off); copy/paste reads from item data, never the masked DOM text; the accessible
      name is masked (placeholder text) until revealed — this fixes the existing P0 gap where
      the accessible name leaks plaintext while blurred; text selection stays unrestricted;
      auto-re-mask on window blur is unchanged (`useSensitiveReveal`); add the optional
      reveal-timeout as a new, off-by-default preference.
- [ ] 3.10 Add tests for the sensitive-masking contract: accessible name is the placeholder while
      masked and updates to the real value on reveal; copy while masked returns the real item
      value; window-blur re-masks (existing behavior, now regression-tested against this
      contract); reveal-timeout preference off by default and functions when enabled.
- [ ] 3.11 Add hover-revealed row actions (pin/delete) per design.md Decision 13 (X4): visible on
      fine-pointer `:hover` and `:focus-within`; always-visible under `(hover: none)`; replaced by
      the checkbox in selection mode; never focusable while visually hidden.
- [ ] 3.12 Verify popup's 4 empty states (offline / starting up / no matches / nothing copied yet
      — design.md/F4 corrected count) each render via `EmptyState`, and document whether
      startup/offline share the same component API as the other two (they do — same `EmptyState`
      props contract).
- [ ] 3.13 Wire `GlideHighlight.tsx`'s overlay to `--dur`/`--ease` tokens (runtime-computed
      position stays inline style/CSS var, design.md Decision 12) and confirm it no-ops under
      `prefers-reduced-motion: reduce`.
- [ ] 3.14 Wire `HighlightedText.tsx`'s fuzzy-match spans to the accent-tinted highlight token.
- [ ] 3.15 Add `aria-expanded`/keyboard-order regression tests for History/Popup rows per
      design.md Decision 13 (X5): 200% zoom/text-scaling reflow, no required 2D scroll, minimum
      target size (or documented desktop exception) for row actions.

## Slice 4 — Devices

- [ ] 4.1 Add `.devrow`/`.devrow__head`/`.devrow__name`/`.devrow__sum`/`.devrow__chev`/
      `.devrow__body`/`.cfields`/`.cfield` (+ `.this`/`.open`/`.removing` states) using the
      disclosure-header primitive from slice 2 (`aria-expanded`/`aria-controls`); document
      `.devcard`/`.dmeta` in `component-library` spec as gallery-only (design.md Decision
      7/15/F6) — not wired into `DevicesView` and not shipped in production CSS.
- [ ] 4.2 Wire `DeviceCard.tsx`'s `StatusDot`, `MetaRow`, `DeviceMetaGrid`, `FingerprintRow`,
      `ThisDeviceCard`, `PeerRow` to the `.devrow`/`.cfields` pattern.
- [ ] 4.3 Implement the device action behavior/state table from design.md Decision 16 (C4)
      exactly: own device (no destructive footer), paired peer online (Unpair + Revoke, equal
      width), paired peer offline (both shown, Unpair best-effort/Revoke unconditional —
      surfaced via tooltip/label, not by hiding), discovered device (neither action), pending
      action (both disabled with spinner on that row), failed action (re-enabled + inline error,
      no silent retry). Add tests asserting no state renders an invalid destructive action.
- [ ] 4.4 Wire `DevicesView/index.tsx` list container, header, and "Pair device" button to
      `.dev-head`/`.dev-hint`/`.dev-list`/`.btn--primary`.
- [ ] 4.5 Wire `DiscoveredRow.tsx` to the same row pattern with its disabled/hint state for
      non-pairable devices.
- [ ] 4.6 Wire `SasPairingModal.tsx` (already `Dialog`-composed, slice 2) to `.qr`/`.sas` (SAS
      digit pills) patterns.
- [ ] 4.7 Wire `RevokeConfirmDialog.tsx` (already `Dialog`-composed, slice 2) with danger confirm
      styling, naming the specific device.
- [ ] 4.8 Wire the Unpair/Revoke footer to equal-width `.btn.btn--danger` per device row, per the
      Decision 16 table.
- [ ] 4.9 Wire Devices' empty state ("No devices paired") to `.empty` with the accent-tinted icon
      variant.
- [ ] 4.10 Transport shown by `.tpill` chip only, never by row background/border color; add a
      regression test.

## Slice 5 — Settings + sidebar + About + Logs + banners + toast

- [ ] 5.1 Add `.set-tabs`/`.set-tab`/`.set-body`/`.set-pane`/`.set-grp`/`.set-grp__h` and wire
      `TabBar.tsx` to the tab-list a11y foundation from slice 2 (`role="tablist"`/`role="tab"`,
      arrow-key navigation, sliding-underline indicator using measured/runtime position per
      design.md Decision 12).
- [ ] 5.2 Wire `SettingsRow.tsx`/`Panel.tsx` usage inside `GeneralTab.tsx`, `SyncTab.tsx`,
      `StorageTab.tsx`, `DisplayTab.tsx`, `ShortcutsTab.tsx` to `.srow`/`.set-grp`.
- [ ] 5.3 Rebuild the Appearance section in `DisplayTab.tsx`: Theme segmented control (`.seg`)
      bound to `prefs.theme`, Accent swatches (`.swatches`/`.swatch`) bound to `prefs.accent`,
      and a Translucency toggle (`.toggle`) bound to `prefs.translucency`, default on (design.md
      Decision 4 — no remaining open question here; all three fields are normative).
- [ ] 5.4 Wire `SliderRow.tsx` (storage limits, preview lines, image height) to the token-driven
      slider track/thumb/tick-mark styling.
- [ ] 5.5 Wire `ShortcutCapture.tsx` keycap rendering to `.kbd`.
- [ ] 5.6 Wire `StatusBanners.tsx`, `CloudAccountMismatchBanner.tsx`, `LimitsMsg.tsx`,
      `InfoPopover.tsx`, `StatusRow.tsx` to `.banner`/`.srow__s` patterns; non-dismissible
      banners (daemon-spawn-error) render with no dismiss control, dismissible banners
      (protocol-mismatch, stale-daemon) each render a Dismiss button.
- [ ] 5.7 Wire the delete-all/import `ConfirmModal.tsx` usage in `SettingsView.tsx` (already
      `Dialog`-composed, slice 2) to `.modal`.
- [ ] 5.8 Add `@layer shell` CSS: `.sb`/`.sb__item`/`.sb__foot`, `.main`, `.vhead`/`.vtitle`/
      `.vsub`, `.about*`, `.logs`/`.logline`/`.lvl`.
- [ ] 5.9 Wire `Sidebar.tsx` nav items + active-item accent left-edge + footer sync chip
      (Gallery nav item wiring happens in slice 6).
- [ ] 5.10 Wire `AboutView.tsx` to `.about`/`.about__logo`/`.about__grid`/`.about__links`.
- [ ] 5.11 Wire `LogView.tsx` to `.logs`/`.logline`/`.lvl` (ok/info/warn/err) with the search
      field.
- [ ] 5.12 Wire `App.tsx`'s daemon-error/protocol-mismatch/stale-daemon banners and
      `AccessibilityBanner.tsx` to the shared `.banner` pattern (correct severity per banner).
- [ ] 5.13 Wire `ErrorBoundary.tsx`'s fallback UI to `.empty`-style centered error block.
- [ ] 5.14 Wire `ViewShell.tsx`'s draggable header region and title/actions slot.
- [ ] 5.15 Wire `Toast.tsx` (`GlassToastItem`/`ToastContainer`) to `.toast` pattern with severity
      dot and an `aria-live` region (design.md Decision 13/X5).
- [ ] 5.16 Verify the cross-window live theme-sync acceptance scenario end-to-end with the real
      Settings Appearance controls (design.md Decision 4/A5): change Theme/Accent/Translucency
      in Settings with the popup open, confirm the popup updates without reopening.

## Slice 6 — Gallery + automated visual/accessibility coverage

- [ ] 6.1 Add `"gallery"` to `DevViewId` only (never to `ProductionViewId`); keep `App.tsx`'s view
      registry a `Record<ProductionViewId, …>` with no gallery entry (design.md Decision 6/B2).
- [ ] 6.2 Implement the DEV-gated dynamic import in `App.tsx`: when
      `import.meta.env.DEV && MOCK && view === "gallery"`, `await import("./views/GalleryView")`
      — mirroring `lib/ipc/transport.ts`'s existing `await import("../mockIpc")` pattern exactly.
- [ ] 6.3 Define stale-state recovery: if a production build ever has `"gallery"` persisted in
      `view`, treat it as unknown and fall back to `"history"` rather than attempting to render
      an unbundled module (design.md Decision 6).
- [ ] 6.4 In `Sidebar.tsx`, render the Gallery nav item only when `import.meta.env.DEV && MOCK`.
- [ ] 6.5 Add `src/lib/fixtures/` typed fixture factories (e.g. `makeHistoryEntry`,
      `makeDevice`) shared by both `mockIpc.ts` and the gallery, with per-story override support
      (design.md Decision 7/G3); ensure these are DEV-only and excluded from production.
- [ ] 6.6 Build gallery sections for every primitive: buttons (all variants/sizes/disabled/
      pending), icon buttons, toggle, segmented control, field, chips/badges/pills, tiles (all 11
      kinds + unknown) — each section with a deterministic `id` for deep-linking (design.md
      Decision 7/G2).
- [ ] 6.7 Build gallery sections for patterns: history row (one per content kind + unknown + one
      long-text example), device row (own + peer, expanded + collapsed, one example per Decision
      16 state), banners (all 4 severities), modal/confirm (via `Dialog`), empty states (all 4
      documented variants), sidebar, settings tab/row, popup row/keycap/glide-highlight.
- [ ] 6.8 Add debug-only forced-state attributes (`data-force-state="hover"|"active"|"focus"`)
      with a CSS parity test confirming forced-state selectors match the real pseudo-class's
      computed styles (design.md Decision 7/G1).
- [ ] 6.9 Add a gallery-local theme/accent/translucency switcher using component state (never
      `setPrefs`), rendering inside a `.theme-scope[data-theme][data-accent][data-translucency]`
      wrapper (design.md Decision 7/A6) — not `<html>` mutation; verify leaving the gallery
      restores the user's real persisted theme/accent/translucency.
- [ ] 6.10 Add the compact "token/critical-component matrix" section rendering the full 12
      theme×accent combinations only for a small critical-component subset (button, card, focus
      ring, status banner) — not twelve full interactive app copies (design.md Decision 7/G2).
- [ ] 6.11 Add long-text and empty-state gallery coverage per component whose layout depends on
      content length (row titles, meta lines, device names, banner messages).
- [ ] 6.12 Confirm the gallery module is absent from the production bundle two ways: (a) `rg` the
      built `dist/` output for the gallery view's unique string content, and (b) inspect the
      emitted Rollup chunk graph/manifest to confirm no production-entry-reachable chunk contains
      the gallery module's file path (design.md Decision 6/B2) — the string check alone is not
      sufficient.
- [ ] 6.13 Write the automated Playwright suite (design.md Decision 13/G4/G5), replacing the
      "manual verification only" posture: main window and popup in dark and light theme; the
      accent/on-accent contrast matrix for the critical-component subset; modal keyboard/focus
      behavior (trap, Escape, backdrop, focus-restoration); `prefers-reduced-motion: reduce` (no
      visible animation, covering the full enumerated scope from Decision 13/S3, not only the
      three duration tokens); long-text overflow (ellipsis, no row-height growth); the production
      gallery-exclusion check from 6.12; automated token-contrast checks (all 12 theme×accent
      combinations × normal/large text/non-text UI/focus indicators/on-accent/status
      surfaces/content-type metadata — design.md Decision 13/X1); and an accessibility scan using
      whatever tool is already approved in the repo's toolchain (flag explicitly if none is
      approved, rather than silently skipping).
- [ ] 6.14 Wire this suite into CI as a required gate for this change — not a stretch goal; update
      any existing "manual spot-check" task language elsewhere in the repo's CI config/docs that
      contradicts this.
- [ ] 6.15 Re-measure popup open/render latency and CSS+JS bundle size against the slice-1
      baseline (design.md Decision 15); confirm both are within the recorded acceptance
      thresholds, or document and justify any exception.
- [ ] 6.16 Confirm zoom/text-scaling (200%), forced-colors fallback for the focus ring, and
      logical focus order pass across the gallery's critical-component subset (design.md
      Decision 13/X3/X5).

## Cross-cutting cleanup (applies across slices; verify at the end of slice 6)

- [ ] C.1 Audit `src/styles/*.css` for any component-specific selector that duplicates a
      primitive instead of reusing its class (semantic DRY per design.md Decision 3) —
      consolidate duplicates found.
- [ ] C.2 Audit all restyled files for hardcoded hex colors, raw px shadow/radius/focus-ring
      values, or raw ms durations outside `tokens.css`, distinguishing legitimate
      runtime-computed geometry (allowed inline, design.md Decision 12) from design constants
      (must be tokens) — replace any design-constant literal with a `var(--…)` reference.
- [ ] C.3 Run the UI lint/format tooling (`npm run lint` in `crates/copypaste-ui`) and fix
      findings.
- [ ] C.4 Run the existing UI unit test suite (`npm test` in `crates/copypaste-ui`) and fix any
      selector-based test breakage caused by restyling — expected to be none, verified via the
      observable-contract acceptance criteria (design.md Decision 13/X2), not literal-attribute
      preservation.
- [ ] C.5 Confirm every icon in the codebase is `lucide-react` (or a documented inline-SVG
      fallback) with explicit sizing — no unsized `<svg>` anywhere (design.md Decision "icons"/
      F2 consistency check between proposal.md and design.md).
