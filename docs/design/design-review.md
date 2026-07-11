# Design review

## Purpose

A run of recent P1 regressions on `main` — a mis-anchored toast, `.row__body`
overflow reopening an empty gap in History rows, a popup outgrowing its
intended footprint, a full-screen card wrapper left on the Settings tab, and a
banner that didn't reflow at large font scale — landed because nothing forced
a design pass before merge. This checklist is that forcing function: a
reviewer (or the author, before requesting review) applies it explicitly
instead of relying on "it looked fine to me." It is referenced from
`.github/PULL_REQUEST_TEMPLATE.md`'s "Design review" section.

## When this applies

Any PR that touches:

- `crates/copypaste-ui/src/**/*.css`
- `crates/copypaste-ui/src/**/*.tsx`
- `android/app/src/main/java/com/copypaste/android/**/*.kt` UI/composable files

Non-UI PRs (protocol, backend, IPC, CLI) can skip this section entirely.

## The checklist

Wording matches the PR template exactly — do not let the two drift apart.

- **Visual hierarchy preserved** — the primary action or content in a view
  should still read first; a change shouldn't accidentally flatten emphasis
  or bury the main control.
- **Spacing rhythm consistent with `docs/design/STYLEGUIDE.md`** — use the
  fixed spacing scale (§5) rather than one-off pixel values.
- **Native-platform suitability (no accidental full-screen cards)** — macOS
  is moving toward a single continuous shell surface (sidebar + content, no
  card border/radius/frost framing around either pane) rather than a web-page
  composition; Android screens should read as native tab content, not a card
  floating inside a card. This is a standing design rule, not yet fully
  reflected in `crates/copypaste-ui/src/styles/shell.css` on every branch —
  check the current state of `shell.css` rather than assuming it.
- **Empty / loading / error states covered** — every list/detail view needs
  all three states designed, not just the happy path (STYLEGUIDE §3.7, §9.10).
- **Legible at 200% OS font scale** — text must not clip or overlap controls.
- **Verified in both dark and light themes** — check `--panel`/`--surface`
  contrast per STYLEGUIDE §3.1/§3.3.

## Mechanical evidence — what exists today

Two real, runnable gates back part of this checklist. Link their output in
the PR; a prose claim ("I checked it") is not sufficient evidence.

- **macOS: `crates/copypaste-ui/e2e/visual/layout-invariants.spec.ts`**
  (Playwright). Run via `npm run test:visual` in `crates/copypaste-ui`
  (`npx playwright test crates/copypaste-ui/e2e/visual/layout-invariants.spec.ts`
  directly also works). These are **structural/geometry assertions plus
  accessibility checks (`@axe-core/playwright`)** — toast safe-area vs.
  sidebar rects, mutually-exclusive-state checks, row density bounds, content
  max-width — **not pixel screenshot diffs**. `playwright.config.ts` has
  `snapshotDir`/`maxDiffPixelRatio` configured but currently unused; pixel
  baselines are deliberately deferred until the native-shell redesign lands.

- **Android: Paparazzi snapshot tests** under
  `android/app/src/test/java/com/copypaste/android/paparazzi/`. These are
  **pixel-diff goldens** against reference PNGs, run via
  `./gradlew :app:testDebugUnitTest` (see `scripts/android-verify.sh` step 4
  for the deterministic invocation). `SettingsScreenSnapshotTest.kt` is the
  regression guard for the Settings de-carding fix. `MainShellContentSnapshotTest.kt`
  covers the extracted `MainShellContent` seam (dark/light shell, one
  font-scale stress case, empty and populated History).

## What this does NOT cover

- No macOS pixel-diff baselines exist yet — only the structural assertions
  above. Adding pixel baselines is future work, gated on the native-shell
  redesign landing first.
- Paparazzi only covers the composables that have a `*SnapshotTest.kt`
  written for them; a UI change to an untested composable gets no mechanical
  coverage from this gate.
- None of this replaces manual testing on real devices/OS builds. A green
  Playwright or Paparazzi run means "the specific assertions/goldens it
  checks passed," nothing more.
