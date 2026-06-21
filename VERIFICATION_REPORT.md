# CopyPaste — Verification Report (Audit Stage 1)

Commands executed during the audit, with exact results. "ENV-blocked" = could not run due to environment/tooling, not a code defect.

## Rust gates

| Command | Exit | Result |
|---|---|---|
| `cargo check --workspace --all-features` | 0 | ✅ clean build, all features |
| `cargo fmt --all --check` | 0 | ✅ formatting clean |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | ✅ zero warnings |
| `cargo test --workspace --all-features` | 0 | ✅ all workspace tests pass (all-features) |
| `cargo deny check` | 0 | ✅ `advisories ok, bans ok, licenses ok, sources ok` |
| `cargo audit` | — | ⚠️ **ENV-blocked**: cached `~/.cargo/advisory-db/crates` missing/corrupt AND network egress blocked (`refusing to initialize non-empty directory` on fetch; `No such file or directory` on `-n`). Advisory coverage still satisfied by `cargo deny check` (advisories ok). Re-run in CI where the advisory-db is fetchable. |

## Parity scripts

| Command | Result |
|---|---|
| `node scripts/parity-check.mjs` | ✅ PASS — "all 53/53 token comparisons within tolerance ±5" |
| `node scripts/check-skin-parity.mjs` | ✅ PASS — "web SKINS and android skinTokens expose the same 21 canonical tokens" |

## Not run this pass (deferred to fix/verify stages)

- **Frontend** `pnpm lint / typecheck / test / build` — no ESLint config exists yet (filed: H-F01). `tsc` + `vitest` exist and run in CI; not executed locally this pass.
- **Android** `./gradlew test / lint / assembleDebug` — deferred: cross-compile is RAM-heavy and project policy (CopyPaste-5a9y) forbids concurrent NDK builds; the Android stream audited source statically. ABI version match (Rust 18 ↔ Kotlin 18) was confirmed by reading both sides.
- **Real-device runtime** (macOS↔Android sync, camera QR scan, Keychain ACL on a live keychain, launchd/foreground-service lifecycle) — requires manual QA; not executable headless.

## Auditor process notes (transparency)

- 11 audit streams ran as parallel background subagents. **4 sub-runs were killed mid-flight by transient server-side rate-limiting** (not usage limit): architecture, parity, sync/relay, and test-coverage. **All 4 were relaunched and completed.** One stream (daemon/IPC) had an internal sub-agent rate-limited and completed that section by direct source reading (noted in `.audit/D-daemon-ipc.md`).
- The completeness stream sampled (did not exhaustively trace) every Android Compose control; flagged as a coverage caveat.
- Raw per-stream evidence: `.audit/A-architecture.md`, `B-crypto.md`, `C-storage.md`, `D-daemon-ipc.md`, `E-sync-relay.md`, `F-macos-tauri.md`, `G-android.md`, `H-tests-ci.md`, `I-parity.md`, `P1-completeness.md`, `P2-reliability-ux.md`, `P3-testcoverage.md` (plus 6 pre-existing `.audit/audit-*.md` from earlier sessions).

## Production-readiness verdict

**Not production-ready yet, but close on fundamentals.** All automated gates that *could* run are green; no P0 and no broken crypto/architecture. The blockers are the 23 P1 reliability/UX/parity issues (destructive-action safety, Android ABI/ProGuard hardening, non-atomic writes, export warning, state-truth). After Phase 1 + the three P1 regression tests, re-run this full gate set plus the Android gradle chain and a real-device QA pass before release.
