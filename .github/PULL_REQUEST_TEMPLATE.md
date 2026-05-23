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

## Breaking changes

<!-- List any breaking changes to IPC protocol, config schema, CLI flags, or public Rust API. -->

## Screenshots / logs

<!-- Optional. Helpful for UI and CLI changes. -->
