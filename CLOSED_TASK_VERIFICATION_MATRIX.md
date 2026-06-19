# CopyPaste — Closed-Task Verification Matrix

Date: 2026-06-19 · Tracking: CopyPaste-d6th. Independent verification of every closed audit/parity campaign task against the uncommitted diff + a rustc-1.96 gate run. Closed status NOT trusted.

**Status legend:** VERIFIED = independently confirmed (code + test/gate evidence) · NEEDS-QA = code correct but test weak/missing or device-only · REOPENED = not truly fixed / gate fails / incomplete · DESIGN = needs product/design sign-off.

**Gate run (rustc 1.96.0):** fmt PASS · clippy FAIL (25 errors, copypaste-daemon) · test 26 binaries / 1 fail (android live cache test) / rest green · UI 171+ PASS · parity 53/53 PASS.

## Audit P1 (13)

| ID | Problem | Fix claim | Auditor | Status | Evidence |
|---|---|---|---|---|---|
| jbao | P1-1 sensitive items uploaded to relay/cloud/P2P | `if is_sensitive {continue}` on 5 paths | Security, Sync | **REOPENED** | Guard code correct (relay.rs:597, cloud.rs:1010/1288, sync_orch.rs:343/997). BUT test is a tautology + relay-api.md:254 doc now contradicts the fix. |
| lszh | P1-2 lsappinfo fail-open | fail-closed | Security, Daemon | VERIFIED | daemon.rs:1664-1748; skip+warn when exclusion non-empty and lsappinfo None; JoinError→None→fail-closed. |
| 26pd | P1-3 blocking lsappinfo on async tick | spawn_blocking | Daemon | VERIFIED | daemon.rs:1668-1718. |
| oti6 | P1-4 `unreachable!()` crash | run_degraded + error log | Daemon | VERIFIED | daemon.rs:149-166; test open_plan_requires_ready_key (note: slightly over-claims branch coverage). |
| 68uk | P1-5 systemd Linux data loss | ReadWritePaths XDG | Daemon | VERIFIED | copypaste-daemon.service:18. |
| v6wh | P1-6 CLI Keychain + plaintext pw | CLI dep removed; daemon sole owner | Security, Arch | NEEDS-QA | security-framework removed; daemon stores+strips+verifies. Residual plaintext pw in IPC body (nq39). |
| wb2c | P1-7 Tauri null CSP | strict CSP | Daemon, macOS | VERIFIED | tauri.conf.json:32; UI build green. |
| xxsw | P1-8 Android raw DB key retained | SHA-256 cache key + evict | Security, Android | **REOPENED** | New unit tests pass, but `live_calls_reuse_cached_connection` (lib.rs:3120) not updated for re-key → FAILS under --all-features. |
| ivqa | P1-9 daemon missing rust-version | added | Test/CI | VERIFIED | daemon Cargo.toml. |
| 9evm | P1-10 version drift | package.json+gradle→0.7.4 | Test/CI | VERIFIED | all artifacts 0.7.4/704. |
| xmsz | P1-11 known-issues.md 404 | created | Test/CI | VERIFIED | docs/known-issues.md exists + linked. |
| z5hl | P1-12 README x86_64 claim | Rosetta/Sonoma noted | Test/CI | VERIFIED | README updated. |
| x2c6 | P1-13 protocol.md missing codes | 3 codes + 27 methods | Daemon, Test/CI | VERIFIED | docs/protocol.md. |

## Audit P2 (selected)

| ID | Problem | Auditor | Status | Evidence |
|---|---|---|---|---|
| iqkm | 9 spawn_blocking key copies unzeroized | Security, Test/CI | **REOPENED** | Zeroizing correct, but 24 `explicit_auto_deref` clippy errors → -D warnings FAIL. Mechanical. |
| ptb8 | version gate emits invalid_argument | Daemon | VERIFIED | ipc.rs:3229-3241 → ERR_CODE_VERSION_MISMATCH. |
| 8u2b | legacy arms drop error_code | Daemon | VERIFIED | ipc.rs:3322/3428/5139 tag INVALID_ARGUMENT. |
| tj9s/PG-5(mv1v) | export emits sensitive plaintext | Security, Storage, Sync | NEEDS-QA | include_sensitive flag (default false) + count-only audit log. No test for the filter. |
| 7185 | unauth GET /devices | Sync | NEEDS-QA | BearerToken extractor added (ct_eq). No 401-path test. |
| fb3e/r6cw/ozzt | detector FP + new patterns | Security, Test/CI | VERIFIED | 5 patterns →0.65 (below floor); 6 anchored cloud patterns; 20 meaningful tests. |
| o8ew | orphan ipc_win.rs | Daemon | VERIFIED | lib.rs:29 `#[cfg(windows)]`. |
| 17lj | relay-api.md rewrite | Sync | **REOPENED** | Introduced doc regression (sensitive "do sync") contradicting jbao. |
| g4rs/2915/4rui/m7mm | SECURITY/ARCH/CI/#[allow] | Daemon, Test/CI | VERIFIED | audit-retry fixed, ci-matrix broadened, MSRV 1.96 pinned, allow-comments present. |

## Parity P1 (PG-1..15, closed set)

| PG | ID | Auditor | Status | Evidence |
|---|---|---|---|---|
| PG-1 | 7d8x | Sync, Android, Parity | **REOPENED** | Unpair dispatch code correct (p2p_listener.rs) but NO test; close reason itself said "needs test". |
| PG-2 | kmcr | Sync, Android, Parity | **REOPENED** | Rust FFI+UDL only; Kotlin bindings not regenerated, no call-site, relay reg skips pop_b64. |
| PG-5 | mv1v | (see tj9s) | NEEDS-QA | export filter, no test. |
| PG-6 | zfqa | macOS, Daemon | NEEDS-QA | bridge forwards protocol_version ✓ but protocolMismatchHandler never assigned → no UI banner; stale comment ipc.ts:83. |
| PG-7 | 2hxr | macOS, Daemon, Parity | VERIFIED | isIpcNotReady in DevicesView/Popup/SettingsView. |
| PG-8 | l7n0 | Android, Parity | VERIFIED | qrRevealed=false removed from generateQr() (read from live file); blur persists. |
| PG-9 | wb6s | macOS, Parity | **DESIGN** | Fingerprint row added but full-hex (spec wanted truncate+tap) and reverses prior removal CopyPaste-n. |
| PG-11 | 71cf | macOS, Parity | NEEDS-QA | Same 5-min recency gate on both, but per-platform copies (no shared enum); PG-10 offline-signal divergence still open. |
| PG-12 | 8qcm | Sync, Android, Parity | **REOPENED** | Rust revoke+rotate only; Kotlin dialog still audit-only → revoked peer keeps key (security). |
| PG-13 | jhvl | macOS, Parity | VERIFIED | Supabase email+password inputs (masked), via set_config, not logged. |
| PG-14 | tpvi | Security, Daemon, Parity | VERIFIED | daemon.rs:1553 degraded boot loads private_mode; regression test added. |

## Parity P2 (closed by fix round)

| PG | ID | Status | Evidence |
|---|---|---|---|
| PG-21 | jfxl | VERIFIED | ipc.ts supabase fields documented writable. |
| PG-30 | j9xj | NEEDS-QA | macOS toggle inert until daemon tke7 (documented; not false-secure). |
| PG-32 | noui | VERIFIED | history limit persists via UIPrefs.historyDisplayLimit. |
| PG-34 | n9gp | VERIFIED | showSensitiveWarnings toggle + reveal gating. |
| PG-36 | fcpf | VERIFIED | :root #3D8BFF→#4D8DFF; parity 53/53. |
| PG-37 | efdc | NEEDS-QA(GRADLE) | offline dot grey→red in DevicesActivity (×2); runtime needs build. |
| PG-40 | i2sr | VERIFIED | hybrid relative/absolute last-sync. |
| 3e6g | 3e6g | VERIFIED | light-first default + migration. |
| 54h5 | 54h5 | VERIFIED | raw error strings out of DOM. |
| vo79 | vo79 | VERIFIED | Android IdeSelection/IdeMultiSel #4D8DFF. |

## New issues filed during verification
- **e5oe** (P2) — FTS orphan on cloud-sync overwrite (`replace_cloud_item_by_item_id`, `sweep_poison_rows` don't clean `clipboard_fts`).
- **h7v8** (P3) — Kotlin `CopypasteException.Panicked` not propagated by hand-wrapper catch blocks.

## Roll-up
- **VERIFIED:** ~28 (all daemon/IPC, crypto invariants, storage core, macOS UI, detector, version/CI/docs-content).
- **NEEDS-QA:** ~8 (v6wh, tj9s/PG-5, 7185, PG-6, PG-11, PG-30, PG-37, + device-only Rust behavior).
- **REOPENED:** 7 (jbao, xxsw, iqkm, kmcr/PG-2, 8qcm/PG-12, 7d8x/PG-1, 17lj) + 2 new (e5oe, h7v8).
- **DESIGN:** 1 (PG-9/wb6s).
