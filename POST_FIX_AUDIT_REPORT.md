# CopyPaste — Post-Fix Independent Verification Report

**Date:** 2026-06-19 · **Tracking:** CopyPaste-d6th · **Method:** 8 independent lead auditors (read-only, against the uncommitted working-tree diff) + a real toolchain gate run. Closed status was NOT trusted.
**Companion docs:** `CLOSED_TASK_VERIFICATION_MATRIX.md`, `REOPENED_ISSUES.md`, `MANUAL_QA_CHECKLIST.md`.

---

## Executive summary

The previous audit-fix campaign was **never compiled** (the toolchain was absent then). This round had **rustc/cargo 1.96** available and ran the gates the prior round could not. The outcome: the fix work is **substantially real and sound on the Rust/macOS/CLI/docs side** — code compiles, and **all but one** of 26 test binaries pass (`copypaste-core` 489, `copypaste-daemon` 114, `copypaste-relay`, `copypaste-sync`, etc. all green). But independent verification found **concrete defects that the "closed" status hid**, justifying the don't-trust-closed mandate:

1. **`cargo clippy -D warnings` is RED** — 25 lint-errors (24 `explicit_auto_deref` from the iqkm `&*` derefs + 1 `empty_line_after_doc_comments`), all in `copypaste-daemon`. The `-D warnings` guarantee is not met.
2. **One test FAILS** — `copypaste-android::live_calls_reuse_cached_connection` (the xxsw fix re-keyed the cache to SHA-256 but left the old test asserting a raw-key lookup).
3. **Two Android P1 closures are only 1/3 done** — PG-2 (relay PoP) and PG-12 (revoke+rotate) have the Rust FFI + UDL but **no regenerated Kotlin bindings and no call-site**. Relay registration still skips PoP; a revoked peer still retains the sync key (a real security gap). They were marked closed.
4. **A critical documentation regression** — the relay-api.md rewrite states *"sensitive items do sync through the relay,"* the exact opposite of the jbao P1-1 fix.
5. **Two flagship P1 fixes have non-functional tests** — jbao's `push_loop_skips_sensitive_items` is a tautology (passes even if the guard is deleted); PG-1's Unpair handler has no test (the bd close reason itself said "needs test").

The core crypto, storage, daemon/IPC, and macOS-UI fixes verify cleanly. The failures cluster in **Android cross-platform completion, test quality, and one doc**.

## Verdicts

| Dimension | Verdict |
|---|---|
| **Security** | PASS with conditions — crypto invariants intact (XChaCha20/AAD/HKDF/zeroize/constant-time confirmed unregressed); P1-1 sensitive-exclusion code correct on all 5 paths. BUT: PG-12 revoked-peer-keeps-key gap not closed on Android; residual plaintext Supabase password in IPC body (nq39); relay-api.md doc contradicts the privacy guarantee. |
| **Architecture** | PASS — CLI & UI still do not link `copypaste-core`; daemon sole socket/Keychain owner; Tauri commands thin proxies; relay ciphertext-only. No boundary regressions. |
| **Platform parity** | PARTIAL — shared `SyncBadgeState` enum was **not** adopted (PG-10/11 patched as two independent copies; PG-10 offline-signal divergence still open). PG-2/PG-12 Android halves incomplete. PG-16 classifier FFI added but `TextKind.kt` not swapped. |
| **Test/CI** | FAIL — clippy red; 1 test red; 2 flagship tests non-functional (jbao tautology, PG-1 missing). CI config improvements (no-default-features job, pnpm build in PR, MSRV 1.96 pin, audit-retry fix) are real and good. |
| **Production readiness** | **NOT production-ready.** Blockers below. |

## Production-readiness blockers (must clear before merge)

1. **iqkm** — fix the 25 clippy errors (`&*x` → `&x`; remove the blank line at `relay.rs:1963`). Mechanical.
2. **xxsw** — update `live_calls_reuse_cached_connection` to look up by `key_cache_hash` (or assert the new contract). Test is currently red.
3. **kmcr (PG-2)** — regenerate Kotlin bindings + wire `relay_registration_pop` into `RelayClient.registerDevice` (`pop_b64`). Until then Android relay registration is non-compliant.
4. **8qcm (PG-12)** — wire the Android revoke dialog to `revoke_device_and_rotate_key` (security: revoked peer must lose the key).
5. **jbao** — fix the relay-api.md:254-255 doc regression AND replace the tautological test with one that actually exercises the push-path guard.
6. **17lj** — relay-api.md correction (same doc regression).
7. **7d8x (PG-1)** — add the Unpair-dispatch test.

## Risk register (non-blocking, tracked)

- **nq39** — residual plaintext Supabase password in the `set_config` IPC JSON body (0600 socket; local-process only).
- **e5oe (new)** — FTS orphan: cloud-sync overwrite paths (`replace_cloud_item_by_item_id`, `sweep_poison_rows`) don't delete `clipboard_fts` rows → stale searchable plaintext. P2.
- **h7v8 (new)** — Kotlin `Panicked` exception not propagated through hand-wrapper catch blocks.
- **PG-6 (zfqa)** — bridge forwards `protocol_version` but `protocolMismatchHandler` is never assigned in `App.tsx` → no UI banner (console.warn only); stale comment `ipc.ts:83`.
- **PG-9 (wb6s)** — own-fingerprint re-added to macOS card reverses the prior intentional removal (CopyPaste-n) and shows full hex (spec wanted truncated+tap-to-copy) → **design sign-off needed**.
- **PG-30 (j9xj)** — macOS master sync toggle inert until daemon `tke7` (clearly labeled in-UI, no false security).
- **PG-24** — Android per-item TTL prune (WorkManager) not addressed (deferred).
- **tj9s/PG-5** — export sensitivity filter has no test (code correct).
- **k89j** — `tracing-subscriber` still in `copypaste-core` (deferred refactor).

## Gate evidence (commands actually run on rustc 1.96.0)

```
cargo fmt --all --check                                    → PASS (exit 0)
cargo clippy --workspace --all-targets --all-features -Dwarnings → FAIL (exit 101; 25 errors, all copypaste-daemon)
cargo test --workspace --all-features --no-fail-fast       → 712+ passed, exactly 1 FAILED
        (copypaste-android::live_calls_reuse_cached_connection);
        all other binaries ok (core 489, daemon 114, relay 8, sync/cli/etc).
        Run hit the 25-min timeout (exit 124) while COMPILING doc-tests —
        environmental, not a code failure; the unit/integration suite completed.
pnpm -C crates/copypaste-ui run test (vitest)              → PASS (171+)
pnpm -C crates/copypaste-ui run build (tsc + vite)         → PASS
node scripts/parity-check.mjs                              → PASS 53/53
```
Could NOT run (documented): Android `./gradlew assembleDebug/test/lint` — UniFFI `.so` needs cargo-ndk cross-compile (OOM-guarded, single-build rule); `cargo deny`/`cargo audit` — advisory-db is a partial clone in this env (no new external crates introduced by the diff, so the prior clean verdict stands).

## What was verified (high-confidence)

- Crypto invariants unregressed (XChaCha20-Poly1305/24-byte OsRng nonce; AAD `(item_id,schema_version,key_version)`; HKDF purpose-separation; `ZeroizeOnDrop`; constant-time compares; no new secret-logging).
- iqkm: 0 bare `**self.local_key` copies remain (all `Zeroizing`-wrapped) — functionally correct, only the clippy style trips.
- Daemon/IPC: all 9 tasks PASS (oti6, 26pd, 68uk, ptb8, 8u2b, o8ew, wb2c CSP, PG-6 bridge, PG-7 ipc_not_ready); socket 0600/0700 + graceful shutdown intact; no new `unwrap()`.
- Storage: ugv7 startup purge (no general-history data loss), PG-26 import re-classification (meaningful tests) PASS.
- macOS UI: CSP, ipc_not_ready everywhere, light-first default, Liquid-Blue `:root`, error sanitization, history-limit persistence, sensitive-warnings toggle — all PASS.
- Detector: 5 FP patterns correctly below the 0.70 auto-wipe floor; 6 new cloud-credential patterns anchored (not over-broad); 20 tests.
- Version consistency now uniform at **0.7.4** (package.json + Android gradle fixed); `docs/known-issues.md` exists; CI gate coverage improved.

## Can the previous fixes be trusted?

**Partially.** The Rust-core / daemon / macOS-UI / CLI / docs-content work is largely sound and now gate-tested. But the **closed status was not reliable**: two Android security P1s were closed with only the Rust half written, two flagship tests don't test anything, clippy was never run (and is red), and a doc was rewritten to contradict its own fix. Independent verification was warranted and necessary.

## Next steps

1. Clear the 7 blockers above (most are small; the Android two need cargo-ndk + Kotlin wiring).
2. Re-run `cargo fmt && clippy -Dwarnings && test --workspace --all-features` to green.
3. Run the Android build + `MANUAL_QA_CHECKLIST.md` (GRADLE-required + on-device scenarios).
4. Decide PG-9 design question and the shared-`SyncBadgeState` direction (PG-10 still open).
5. Only then consider committing (nothing is committed yet — all changes are uncommitted working-tree).
