## Summary

<!-- One paragraph: what changed and why. Focus on intent, not diff. -->

## Linked issues

<!-- Closes #N, Refs #M. Required for non-trivial changes. -->

## Type of change

- [ ] feat — new user-visible functionality
- [ ] fix — bug fix
- [ ] perf — performance improvement
- [ ] refactor — internal restructure, no behaviour change
- [ ] docs — documentation only
- [ ] test — adding or fixing tests only
- [ ] chore — build, CI, deps, tooling

## Testing

<!-- Describe how you verified the change. Paste command output where useful. -->

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all -- --check` clean
- [ ] New unit / integration tests added for new behaviour
- [ ] Manually tested on affected platform(s): <!-- macOS / Android / Windows / Linux / relay -->

## Checklist

- [ ] All tests pass locally
- [ ] Clippy is clean (no new warnings)
- [ ] Public API / IPC / config schema changes are documented
- [ ] **ADR added under `docs/adr/`** if this changes architecture
- [ ] **CHANGELOG entry added** if this is user-visible
- [ ] No plaintext clipboard content, keys, or tokens added to logs
- [ ] No secrets, credentials, or `.env` files committed

## Design review (UI changes only)

<!-- Fill in only if this PR touches UI/layout/styling. See docs/design/design-review.md for how to apply this checklist and what the mechanical gates check. -->

- [ ] Visual hierarchy preserved (primary action / content reads first)
- [ ] Spacing rhythm consistent with `docs/design/STYLEGUIDE.md`
- [ ] Native-platform suitability (no accidental full-screen cards; matches platform shell conventions)
- [ ] Empty / loading / error states covered
- [ ] Legible at 200% OS font scale
- [ ] Verified in both dark and light themes

**Mechanical evidence (link, don't just claim):**

- [ ] Android: passing `SettingsScreenSnapshotTest` / relevant `*SnapshotTest.kt` under `android/app/src/test/java/com/copypaste/android/paparazzi/` — link the golden-diff artifact or CI run
- [ ] macOS: green `npx playwright test crates/copypaste-ui/e2e/visual/layout-invariants.spec.ts` (`npm run test:visual` in `crates/copypaste-ui`) — link output/log. Structural/geometry assertions only (no pixel screenshot baselines yet).

## Breaking changes

<!-- List any breaking changes to IPC protocol, config schema, CLI flags, or public Rust API. -->

## Screenshots / logs

<!-- Optional. Helpful for UI and CLI changes. -->
