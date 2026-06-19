# CopyPaste — Audit Fix Summary

**Date:** 2026-06-19 · **Driver:** Orchestrator + 7 parallel subagents (A–G) · **Source:** `AUDIT_FINDINGS.md`
**Footprint:** 44 files changed, +1671/−312, plus new `docs/known-issues.md`. Nothing committed/pushed — all changes are on the working tree for review.

## Environment caveat (read first)
No Rust toolchain exists in this environment (`cargo`/`rustc`/`rustup` absent). **All Rust source fixes are implemented + diff-reviewed but NOT locally compiled** — they are tagged `CI-COMPILE-REQUIRED` and must pass `cargo fmt/clippy/test` on a ≥1.96 toolchain (CI) before merge. UI (pnpm), docs, config, and design-token parity were verified locally.

---

## What was fixed

### P1 — all 13 addressed
| Audit | bd | Fix | Verify |
|---|---|---|---|
| P1-1 sensitive→relay/cloud/P2P | jbao | `if item.is_sensitive { continue; }` on all 5 outbound paths (relay push, cloud push, cloud backlog sweep w/ mark-synced, P2P outbound_tx, P2P catch-up) before any crypto; inbound untouched | CI + new test `push_loop_skips_sensitive_items` |
| P1-2 lsappinfo fail-open | lszh | fail-**closed**: lsappinfo failure + non-empty exclusion list → warn + advance changecount + skip capture | CI |
| P1-3 blocking lsappinfo | 26pd | moved into `tokio::task::spawn_blocking` | CI |
| P1-4 `unreachable!()` crash | oti6 | → `tracing::error!` + `run_degraded(DEGRADED_REASON_KEYCHAIN_LOCKED)` graceful fallback | CI + test |
| P1-5 systemd path (Linux data loss) | 68uk | `ReadWritePaths` → `%h/.local/share/copypaste` (XDG) | **verified** (config) |
| P1-6 CLI Keychain + plaintext pw | v6wh | CLI Keychain write + `security-framework` dep removed; daemon is sole Keychain owner. **Partial** — residual (pw in `set_config` JSON; non-macOS) tracked in **nq39** | CI; orphan-ref check **verified** |
| P1-7 Tauri null CSP | wb2c | strict CSP set, scoped to real asset usage | **verified** (pnpm build + 171 tests) |
| P1-8 Android raw DB key retained | xxsw | cache re-keyed by SHA-256(key), raw key never stored; `close_database` evicts | CI + 3 tests |
| P1-9 daemon missing rust-version | ivqa | `rust-version.workspace = true` | **verified** |
| P1-10 version drift | 9evm | package.json + Android gradle → 0.7.4/704 | **verified** (valid JSON) |
| P1-11 known-issues 404 | xmsz | `docs/known-issues.md` created | **verified** (link resolves) |
| P1-12 README x86_64 claim | z5hl | corrected to Rosetta 2 / Sonoma+ | **verified** |
| P1-13 protocol.md missing codes | x2c6 | added 3 error codes + full 27-method list | **verified** |

### P2 — 19 audit-derived bugs fixed
- **Detector (fb3e/r6cw/ozzt):** 5 FP patterns dropped below the 0.70 auto-wipe floor (→0.65) so benign data is no longer silently deleted; `twilio_auth_token` (was matching the SID, not the token) renamed + anchored; misleading `openai_legacy` lookahead comment corrected; 6 new cloud-credential patterns added (Azure storage/SAS, GCP SA-JSON, Cloudflare [context-required], SendGrid, Terraform). 20 tests. *(CI)*
- **IPC (ptb8/8u2b/iqkm/tj9s):** version gate now emits `version_mismatch`; legacy `delete`/`copy`/`paste`/`set_private_mode` arms tagged `INVALID_ARGUMENT`; 9 `spawn_blocking` key copies wrapped in `Zeroizing`; export handler gains `include_sensitive` flag + audit log (count only). *(CI)*
- **Relay (7185):** unauthenticated `GET /devices` now requires bearer auth (closes inbox-UUID enumeration). *(CI)*
- **Android (ar2r/2ffx/vo79):** cache eviction on close (fixes use-after-close); `eprintln!`→`tracing::debug!` (logcat hygiene); Liquid-Blue `IdeSelection`/`IdeMultiSel` → `#4D8DFF`. *(CI; parity 53/53 verified)*
- **CLI (jqcp):** `pair-qr --raw` prints a stderr secret-warning. *(CI)*
- **UI (3e6g/54h5/00ae):** default theme → light-first (saved pref still overrides); raw error strings (paths) sanitized out of the DOM (console-only); IMAGE chip sky color confirmed **intentional** per `1hqt` comment (not-a-bug). *(verified, pnpm)*
- **Daemon (ugv7):** startup TTL purge before socket bind — verified no general-history data loss (`delete_expired` uses per-item expiry). *(CI + test)*
- **Docs/CI (17lj/g4rs/2915/4rui/m7mm):** relay-api.md rewritten to the shipped protocol; SECURITY.md corrected to the (stronger) shipped design; ARCHITECTURE/README/protocol crate+method lists completed; `audit.yml`/`ci.yml` retry no longer masks advisories; every `#[allow]` now has a reason comment. *(verified, text)*
- **o8ew:** orphan `ipc_win.rs` wired under `#[cfg(windows)]` (was falsely self-described as declared). *(CI, cfg-gated out on unix)*

### P3 — cheap cluster done
MSRV `1.89`→`1.96` comments across 6 Cargo.toml + scrubber.rs; `deny.toml` stale `RUSTSEC-2024-0429` ignore removed; `ci-matrix.yml` scope broadened to `[main, "release/**"]`; ADR-010 artifact path fixed; daemon `AppConfig::load` warns on TOML parse error; file read-before-size-gate adds a `metadata().len()` pre-check; `poll_interval_ms` restart-required TODO noted.

---

## Files changed (by area)
- **Rust (CI-COMPILE-REQUIRED):** daemon `daemon.rs` (+312), `ipc.rs` (+150), `relay.rs`, `cloud.rs`, `sync_orch.rs`, `lib.rs`, `main.rs`, `keychain/acl.rs`, daemon/cli/core/relay/android/bench `Cargo.toml`; cli `cloud.rs`, `pair_qr.rs`; core `sensitive/{patterns,detector}.rs`; relay `routes/mod.rs`; android `lib.rs`; ui-tauri `event_tap.rs`.
- **Kotlin (Android build blocked):** `CopypasteBindings.kt` (Panicked variant), `Color.kt`.
- **UI (verified pnpm):** `tauri.conf.json`, `index.html`, `store.ts`, `App.tsx`, `ErrorBoundary.tsx`, `SettingsView.test.tsx`, `package.json`.
- **Docs/config (verified text):** README, SECURITY, ARCHITECTURE, `docs/{protocol,relay-api,known-issues}.md`, ADR-010, `contrib/systemd/*.service`, `.github/workflows/{audit,ci,ci-matrix}.yml`, `deny.toml`, android gradle.

## Tests added
- daemon: `open_plan_requires_ready_key`, `startup_ttl_purge_removes_expired_sensitive_items`, lsappinfo contract doc test.
- relay: `push_loop_skips_sensitive_items`.
- android: 3 cache-hash/eviction tests.
- core/detector: 20 FP/new-pattern tests.
- ui: `SettingsView.test.tsx` updated to assert sanitized error behavior (171/171 pass).

## What remains
| Item | bd | Why |
|---|---|---|
| P1-6 residual: dedicated `store_supabase_password` IPC verb; non-macOS no-disk-persist | **nq39** | new IPC method unverifiable without compiler; boundary violation already fixed |
| Move `init_global`/`tracing-subscriber` out of core | **k89j** (deferred) | cross-crate refactor; breaks-build risk with no compiler; P2 hygiene |
| Rust P3 defense-in-depth: empty-AAD `pub(crate)`, relay HKDF salt (migration-sensitive), export/PoP `Zeroizing` | — | unverifiable; relay salt is migration-sensitive per audit |
| Android feature parity: cloud-transport model, bulk copy, localization, diagnostics, a11y, desktop history-limit, export/import UI | dtq3,g3z4,ojsq,otb7,q649,2b1g,85n9 | feature work needing the blocked Android build; pre-existing roadmap, not audit bugs |
| Pre-existing Android sync P1s (not in this audit) | lcmq,0qpn,vfai,f797,vjqc | separate Liquid-Glass/Android campaign; need Android build |

## Risks
1. **No Rust compile/test ran.** The highest-risk diffs are P1-6 (Zeroizing deref-coercion in `ipc.rs`), the 9 `iqkm` sites, and field-name assumptions in the relay/cloud filter (`item.id` vs `item.item_id`). CI must be green before merge.
2. **P1-1 behavior change:** sensitive items now never leave the device on any transport. If any user relied on sensitive items syncing, this is a deliberate behavior change (now matches the documented guarantee).
3. **Light-first default** changes first-paint for fresh installs only (saved prefs preserved; legacy v1 migration drops the old theme once — same as the prior migration design).

## Manual QA needed (post-CI)
- Copy an API key with relay sync on → confirm no ciphertext row in the relay inbox (P1-1).
- Copy from 1Password with it excluded, then break `lsappinfo` → confirm capture is skipped (P1-2).
- `cloud setup` → confirm password lands in Keychain via daemon, not written by CLI (P1-6).
- Tauri app loads with the new CSP (no console CSP violations); QR renders (P1-7).
- Android: open→close→reopen DB; confirm no panic and key not retained (P1-8).
