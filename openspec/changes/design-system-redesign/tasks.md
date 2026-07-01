## 1. Tokens layer (`design-tokens` capability)

- [ ] 1.1 Create `crates/copypaste-ui/src/index.css` with the `1-tokens` section: copy the
      `:root`/`:root[data-theme="dark"]`/`:root[data-theme="light"]` blocks verbatim from
      `copypaste-design-reference.html` (surfaces, lines, text, overlays, status tokens).
- [ ] 1.2 Add the 6 `:root[data-accent="…"]` blocks plus the `:root[data-theme="light"][data-accent="…"]`
      override blocks, verbatim from the reference file.
- [ ] 1.3 Add content-type tokens (`--c-text`, `--c-url`, `--c-mail`, `--c-num`, `--c-code`,
      `--c-json`, `--c-color`, `--c-file`, `--c-image`, `--c-secret`) for both themes.
- [ ] 1.4 Add spacing (`--s-1`…`--s-9`), radius (`--r-chip/pill/ctl/input/card/window`), shadow
      (`--sh1/2/3`, themed), font, and motion (`--dur-fast/--dur/--dur-theme/--ease`) tokens, plus
      the `@media (prefers-reduced-motion: reduce)` override block.
- [ ] 1.5 Add the `2-base` layer (box-sizing reset, body/svg/button/input/link/focus-visible/
      selection/scrollbar base rules) from the reference file's Layer 2.
- [ ] 1.6 Restore `theme: "dark" | "light"` and `accent: "indigo" | "blue" | "teal" | "green" |
      "amber" | "rose"` fields to `UIPrefs` in `crates/copypaste-ui/src/store.ts`; bump the
      persisted key to `copypaste-ui-prefs-v5` with an additive migration from v4 (defaults
      `dark`/`indigo`).
- [ ] 1.7 In `src/App.tsx`, add a `useEffect` that writes `prefs.theme`/`prefs.accent` to
      `document.documentElement.dataset.theme`/`.dataset.accent` on mount and on change.
- [ ] 1.8 In `src/popup/Popup.tsx` (or `src/popup/main.tsx`), add the equivalent `useEffect` for the
      popup window's `<html>`.
- [ ] 1.9 Update `index.html`: keep a static `data-theme="dark"` for first paint; delete
      `data-palette`, `data-density`, `data-motion`, `data-contrast` and their stale comment block.
- [ ] 1.10 Update `popup.html`: keep a static `data-theme="dark"`; delete the stale comment
      referencing the "Liquid Glass" look.
- [ ] 1.11 Import `src/index.css` from both `src/main.tsx` and `src/popup/main.tsx`.
- [ ] 1.12 Verify: `rg` the repo for `data-palette|data-skin|data-density|data-motion|data-contrast`
      outside `docs/`/changelog and confirm zero remaining occurrences (STYLEGUIDE.md §12 done-check).

## 2. Primitives layer (`component-library` capability — shared atoms)

- [ ] 2.1 Add the `3-primitives` CSS section: `.btn` (+ `--primary/--secondary/--ghost/--danger`,
      `.sm`, `.block`, `:disabled`), `.iconbtn` (+ `.danger`), `.toggle` (+ `.off`), `.seg`,
      `.field`, `.chip` (+ `.on`, `.chip--ct`), `.tpill` (+ `--p2p/--cloud/--this`), `.badge`
      (+ `--verified/--count`), `.tile` (+ `--swatch/--thumb`), `.dot-stat` (+ `.off` + pulse
      keyframes), `.card`, `.divider`, `.spinner`, `.kbd`.
- [ ] 2.2 Wire `ActionButton.tsx` to emit `.btn .btn--<variant>` (+ `.sm` for `size="sm"`) instead
      of bare `<button>`; keep all existing props/behavior unchanged.
- [ ] 2.3 Wire `Toggle.tsx` to the `.toggle`/`.off` classes and its knob `<span>`.
- [ ] 2.4 Wire `SectionHeader.tsx`, `Panel.tsx`, `SettingsRow.tsx`, `SliderRow.tsx` to their
      corresponding patterns (`.set-grp__h`, `.card`/panel surface, `.srow`, slider track/thumb).
- [ ] 2.5 Wire `SyncStatusChip.tsx`, `DeviceBadge.tsx`, `FileChip.tsx` to `.chip`/`.badge`/`.tpill`
      primitives as appropriate to each one's semantics.
- [ ] 2.6 Restore icons (via `lucide-react`) with explicit sizes in every component touched in this
      section; verify no `<svg>` renders without an explicit width/height.

## 3. Patterns layer — History surface

- [ ] 3.1 Add the `4-patterns` CSS for `.row`/`.row__body`/`.row__title`/`.row__meta`/`.row__right`,
      `.del`/`.star-btn`, `.chk`, `.grouphead`, `.bulkbar`, plus `filtered`/`removing`/`copied`/
      `pinned`/`sel` state classes and their keyframes.
- [ ] 3.2 Wire `HistoryRow.tsx` to `.row` + kind-specific `--ct` custom property and tile content
      (glyph vs. swatch vs. thumbnail) for all 11 content kinds.
- [ ] 3.3 Wire `HistoryView.tsx`/`VirtualList.tsx` list container, search field, and filter chips
      to `.list`/`.field`/`.filters`/`.chip`.
- [ ] 3.4 Wire `BulkActionBar.tsx` to `.bulkbar`.
- [ ] 3.5 Wire `EmptyState.tsx` to `.empty`/`.empty__ic`/`.empty__t`/`.empty__s` and verify all 3
      History empty-state call sites (no items / no search results) render correctly.
- [ ] 3.6 Wire `DetailsModal.tsx` and `HistoryView/`'s `ConfirmModal` usage (bulk delete) to the
      shared `.scrim`/`.modal` pattern.
- [ ] 3.7 Add masked-secret styling (`.mask`) preserving click-to-reveal behavior and full-width
      occupancy while masked.

## 4. Patterns layer — Devices surface

- [ ] 4.1 Add `.devrow`/`.devrow__head`/`.devrow__name`/`.devrow__sum`/`.devrow__chev`/
      `.devrow__body`/`.cfields`/`.cfield` (+ `.this`/`.open`/`.removing` states) and
      `.devcard`/`.dmeta` (documented, gallery-only per design.md Decision 4a).
- [ ] 4.2 Wire `DeviceCard.tsx`'s `StatusDot`, `MetaRow`, `DeviceMetaGrid`, `FingerprintRow`,
      `ThisDeviceCard`, `PeerRow` to the `.devrow`/`.cfields` pattern.
- [ ] 4.3 Wire `DevicesView/index.tsx` list container, header, and "Pair device" button to
      `.dev-head`/`.dev-hint`/`.dev-list`/`.btn--primary`.
- [ ] 4.4 Wire `DiscoveredRow.tsx` to the same row pattern with its disabled/hint state for
      non-pairable devices.
- [ ] 4.5 Wire `SasPairingModal.tsx` to `.modal` + `.qr`/`.sas` (SAS digit pills) patterns.
- [ ] 4.6 Wire `RevokeConfirmDialog.tsx` to `.modal` with danger confirm styling.
- [ ] 4.7 Wire the Unpair/Revoke footer to equal-width `.btn.btn--danger` per device row.
- [ ] 4.8 Wire Devices' empty state ("No devices paired") to `.empty` with the accent-tinted icon
      variant.

## 5. Patterns layer — Settings surface

- [ ] 5.1 Add `.set-tabs`/`.set-tab`/`.set-body`/`.set-pane`/`.set-grp`/`.set-grp__h` and wire
      `TabBar.tsx` to the sliding-underline `.set-tab` pattern (measured indicator, not JS-computed
      colors).
- [ ] 5.2 Wire `SettingsRow.tsx`/`Panel.tsx` usage inside `GeneralTab.tsx`, `SyncTab.tsx`,
      `StorageTab.tsx`, `DisplayTab.tsx`, `ShortcutsTab.tsx` to `.srow`/`.set-grp`.
- [ ] 5.3 Rebuild the Appearance section in `DisplayTab.tsx`: Theme segmented control (`.seg`)
      bound to `prefs.theme`, Accent swatches (`.swatches`/`.swatch`) bound to `prefs.accent` — see
      design.md Open Question 1 for whether Translucency ships in this pass.
- [ ] 5.4 Wire `SliderRow.tsx` (storage limits, preview lines, image height) to the token-driven
      slider track/thumb/tick-mark styling.
- [ ] 5.5 Wire `ShortcutCapture.tsx` keycap rendering to `.kbd`.
- [ ] 5.6 Wire `StatusBanners.tsx`, `CloudAccountMismatchBanner.tsx`, `LimitsMsg.tsx`,
      `InfoPopover.tsx`, `StatusRow.tsx` to `.banner`/`.srow__s` patterns.
- [ ] 5.7 Wire the delete-all/import `ConfirmModal.tsx` usage in `SettingsView.tsx` to `.modal`.

## 6. Patterns layer — Sidebar, About, Logs, app-level banners

- [ ] 6.1 Add `5-app-shell` CSS: `.sb`/`.sb__item`/`.sb__foot`, `.main`, `.vhead`/`.vtitle`/`.vsub`,
      `.about*`, `.logs`/`.logline`/`.lvl`.
- [ ] 6.2 Wire `Sidebar.tsx` nav items + active-item accent left-edge + footer sync chip.
- [ ] 6.3 Wire `AboutView.tsx` to `.about`/`.about__logo`/`.about__grid`/`.about__links`.
- [ ] 6.4 Wire `LogView.tsx` to `.logs`/`.logline`/`.lvl` (ok/info/warn/err) with the search field.
- [ ] 6.5 Wire `App.tsx`'s daemon-error / protocol-mismatch / stale-daemon banners and
      `AccessibilityBanner.tsx` to the shared `.banner` pattern (correct severity per banner).
- [ ] 6.6 Wire `ErrorBoundary.tsx`'s fallback UI to `.empty`-style centered error block.
- [ ] 6.7 Wire `ViewShell.tsx`'s draggable header region and title/actions slot.
- [ ] 6.8 Wire `Toast.tsx` (`GlassToastItem`/`ToastContainer`) to `.toast` pattern with severity dot.

## 7. Patterns layer — Popup surface

- [ ] 7.1 Add popup-specific CSS (search bar, condensed row, footer keycap strip) reusing
      `.field`/`.row`/`.kbd`/`.empty` primitives — no popup-only visual language.
- [ ] 7.2 Wire `Popup.tsx` search bar + result-count + footer.
- [ ] 7.3 Wire `PopupRow.tsx` to the condensed `.row` variant (tile/preview/keycap/pin).
- [ ] 7.4 Wire `GlideHighlight.tsx`'s absolute-positioned overlay to `--dur`/`--ease` tokens and
      confirm it no-ops under `prefers-reduced-motion: reduce`.
- [ ] 7.5 Wire `HighlightedText.tsx`'s fuzzy-match spans to the accent-tinted highlight token.
- [ ] 7.6 Verify popup's 3 empty states (offline / starting up / no matches / nothing copied yet)
      each render via `EmptyState`.

## 8. Preview gallery (`preview-gallery` capability)

- [ ] 8.1 Add `"gallery"` to `ViewId` in `store.ts`; create `src/views/GalleryView/index.tsx`.
- [ ] 8.2 In `Sidebar.tsx`, render the Gallery nav item only when `import.meta.env.DEV && MOCK`
      (import `MOCK` from `lib/ipc/transport`).
- [ ] 8.3 Build gallery sections for every primitive: buttons (all variants/sizes/disabled/pending),
      icon buttons, toggle, segmented control, field, chips/badges/pills, tiles (all 11 kinds).
- [ ] 8.4 Build gallery sections for patterns: history row (one per content kind + one long-text
      example), device row (own + peer, expanded + collapsed), banners (all 4 severities), modal/
      confirm, empty states (all documented variants), sidebar, settings tab/row, popup row/keycap/
      glide-highlight.
- [ ] 8.5 Add a gallery-local theme/accent preview control using component state (NOT `setPrefs`),
      so switching it never writes to persisted `UIPrefs`; render token swatches (surfaces, text
      ramp, content-type, status) alongside it.
- [ ] 8.6 Verify leaving the gallery restores the user's real persisted theme/accent (design.md
      Decision 5 / Open Question 4).
- [ ] 8.7 Confirm the gallery module is absent from the production bundle: `npm run build` then
      `rg` the built `dist/` output for the gallery view's unique string content and confirm no
      match.

## 9. Cross-cutting cleanup and DRY audit

- [ ] 9.1 Audit `src/index.css` for any component-specific selector that duplicates a primitive
      instead of reusing its class (DRY requirement) — consolidate duplicates found.
- [ ] 9.2 Audit all restyled files for hardcoded hex colors, raw px shadow/radius values, or raw ms
      durations outside `index.css`'s tokens layer; replace with `var(--…)` references.
- [ ] 9.3 Run `cargo fmt --all` is N/A (UI-only change); run the UI lint/format tooling
      (`npm run lint` in `crates/copypaste-ui`) and fix findings.
- [ ] 9.4 Run the existing UI unit test suite (`npm test` in `crates/copypaste-ui`) and fix any
      selector-based test breakage caused by restyling (should be none per the a11y-preservation
      requirement).

## 10. Manual browser verification (required before this change is done)

- [ ] 10.1 Start the dev server and open `localhost:1420/?mock=1`; verify History (empty, populated,
      filtered, selection mode, pinned, secret masked/revealed) in dark theme, indigo accent.
- [ ] 10.2 Verify Devices (own device expanded, a paired peer expanded/collapsed, discovered device,
      pairing modal QR→SAS→done flow, revoke confirm) in dark theme.
- [ ] 10.3 Verify Settings — all 5 tabs (General/Display/Sync/Shortcuts/Storage), including the
      rebuilt Appearance Theme/Accent controls actually restyle the app live.
- [ ] 10.4 Verify About and Logs views.
- [ ] 10.5 Verify the quick-paste popup (`localhost:1420/popup.html?mock=1` or the app's popup
      window) — search, keyboard nav, keycaps, empty states.
- [ ] 10.6 Switch to light theme and spot-check History/Devices/Settings/popup for contrast and
      layout regressions.
- [ ] 10.7 Cycle through all 6 accents (at least in the Gallery, spot-check 2 in the live app) and
      confirm `--accent`/`--accent-2`/`--on-accent` usages read correctly in both themes.
- [ ] 10.8 Open the Gallery view and visually confirm every component/state section renders without
      layout breakage, in at least 2 theme×accent combinations, using its own preview control for
      the rest.
- [ ] 10.9 Confirm `prefers-reduced-motion: reduce` (OS setting or browser emulation) removes all
      animation (row insert/remove, toast, spinner, selection glide, presence pulse).
- [ ] 10.10 Record verification results (screenshots or a short note) in the PR/change description
       before marking this change complete.
