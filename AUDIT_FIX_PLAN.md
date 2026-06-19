# CopyPaste — Audit Fix Plan

Prioritized remediation for the findings in `AUDIT_FINDINGS.md`. Each item lists the finding id, primary file(s), and whether tests are needed. No P0 issues exist, so Phase 1 targets the security-guarantee mismatch + daemon reliability + data-loss-class issues.

Suggested workflow: one bd issue per item (titles in Ukrainian per repo convention), claim → fix → `--notes` → close. Re-run the full Rust gate suite on a **rustc ≥ 1.96** toolchain before/after each phase (local rustc is 1.95 and cannot build this workspace).

---

## Phase 1 — Security guarantee + reliability + data-loss (do first)

| # | Finding | Files | Tests |
|---|---|---|---|
| 1.1 | **Enforce or retract the "sensitive never uploaded" guarantee** — add `if item.is_sensitive { continue; }` to relay/cloud/P2P push paths, OR correct `relay-api.md:105` + README | `crates/copypaste-daemon/src/relay.rs:564-684`, `sync_orch.rs` push, `cloud.rs`; `docs/relay-api.md:105` | Yes — assert a sensitive item never enters the push channel/`pending_uploads` |
| 1.2 | **App-exclusion fail-open** — warn on `lsappinfo` failure; consider fail-closed when exclusion list non-empty | `crates/copypaste-daemon/src/daemon.rs:1596-1597` | Yes — tick test w/ stubbed lsappinfo failure |
| 1.3 | **Blocking `lsappinfo` on async tick** — wrap in `spawn_blocking` | `daemon.rs:1594-1610` | Async tick test w/ exclusion set |
| 1.4 | **`unreachable!()` crash path** — replace with `run_degraded` + `error!` | `daemon.rs:152` | Unit test for `Open`+`Locked` |
| 1.5 | **systemd `ReadWritePaths` macOS path on Linux** → silent capture loss | `contrib/systemd/copypaste-daemon.service:18` | Linux systemd smoke |
| 1.6 | **Android raw DB key retained unzeroized in `DB_BY_PATH`, survives close** | `crates/copypaste-android/src/lib.rs:2257-2277,2335-2341` | Open→close→assert no live key bytes |
| 1.7 | **Tauri null CSP** — set strict CSP | `crates/copypaste-ui/src-tauri/tauri.conf.json:32` | CSP-present integration test |
| 1.8 | **CLI→Keychain boundary leak + plaintext password over IPC** — move password storage into a daemon IPC verb; drop CLI `security-framework`; remove plaintext field | `cli/commands/cloud.rs:42-58,163-180`, `cli/Cargo.toml:31-34`, `daemon ipc.rs:4559-4596` | Daemon Keychain read test; assert CLI never sends password |

---

## Phase 2 — Correctness, IPC contract, detector, key hygiene

| # | Finding | Files | Tests |
|---|---|---|---|
| 2.1 | **No startup purge of expired sensitive items** — run `run_ttl_cleanup` once after DB open, before socket bind | `daemon.rs:1178-1256` | Startup-purge test |
| 2.2 | **Detector false-positives above 0.70 auto-wipe floor** — lower/validate Discord, Twilio(SID), SSN, IBAN, generic-bearer | `core/src/sensitive/patterns.rs:53,71-77,113-117,139-145` | FP tests per pattern |
| 2.3 | **Missing cloud-cred detector patterns** — Azure/GCP-SA-JSON/Cloudflare/SendGrid/Terraform | `core/src/sensitive/patterns.rs` | Detection tests |
| 2.4 | **IPC version gate emits `invalid_argument` not `version_mismatch`** | `daemon ipc.rs:3202-3209` | Version-mismatch e2e |
| 2.5 | **Legacy IPC arms drop `error_code`** — tag `INVALID_ARGUMENT` on `delete`/`copy`/`paste`/`set_private_mode` | `daemon ipc.rs:3288-3298,5077` | Error-code assertions |
| 2.6 | **`export` unlogged bulk plaintext** — add audit log + optional `include_sensitive` flag | `daemon ipc.rs:7605-7757` | — |
| 2.7 | **9 unzeroized `[u8;32]` key copies in spawn_blocking** — wrap in `Zeroizing` | `daemon ipc.rs:3882,4015,4208,7449,7616,7838,8005,8102,8159` | Drop-zeroization test |
| 2.8 | **`copypaste-core` ships `tracing-subscriber` as prod dep + exports `init_global`** — move to binary crates; core dev-dep only | `core/Cargo.toml:34,38`, `core/src/logging.rs` | — |
| 2.9 | **Android `close_database` doesn't evict cache (use-after-close)** | `android/lib.rs:2335-2341` | Close-then-reuse test |
| 2.10 | **Android `eprintln!` → logcat** — switch to `tracing`/`android_logger` | `android/lib.rs:~1694` | — |
| 2.11 | **`pair-qr --raw` token to stdout, no warning** | `cli/commands/pair_qr.rs:34-36` | — |
| 2.12 | **Relay unauth `GET /devices` leaks inbox UUIDs** — require token or remove | `relay/src/routes/mod.rs:293-298` | Auth test |
| 2.13 | **`audit.yml` retry masks advisories** — split cache-miss recovery from failure | `.github/workflows/audit.yml:46`, `ci.yml:118` | — |

---

## Phase 3 — Tests & CI coverage

| # | Item | Files |
|---|---|---|
| 3.1 | Add `cargo test --no-default-features` and `--all-features` jobs (neither exists) | `.github/workflows/ci.yml` |
| 3.2 | Run `pnpm build` in PR CI (currently release-only) | `ci.yml` |
| 3.3 | Broaden `ci-matrix.yml` from `release/v0.2.0-beta` to `[main, "release/**"]` | `.github/workflows/ci-matrix.yml:8-10` |
| 3.4 | Wire orphan scripts into CI or delete: `check-license-headers.sh`, `check-adr-format.sh`, `e2e.sh`, `release-gate.sh` | `scripts/`, workflows |
| 3.5 | Regression tests behind Phase 1/2 fixes (sensitive-no-upload, fail-closed exclusion, CSP, key-zeroize) | various |
| 3.6 | Remove `deny.toml` stale ignore `RUSTSEC-2024-0429`; track upstream glib | `deny.toml:33-37` |
| 3.7 | Daemon `unreachable!()` → covered by 1.4; add MSRV-presence lint awareness | — |

---

## Phase 4 — UI parity & polish

| # | Item | Files |
|---|---|---|
| 4.1 | Default theme → light-first (or update spec) | `src/index.html:12`, `src/store.ts:96` |
| 4.2 | Finish Liquid-Blue: `IdeSelection`/`IdeMultiSel` → `#4D8DFF` | `android/.../Color.kt:16,34` |
| 4.3 | Add export/import + backup/restore UI (CLI parity) | `src/lib/ipc.ts`, new views |
| 4.4 | Android Appearance/palette screen (palette already in `Theme.kt`) | `android/.../SettingsActivity.kt` |
| 4.5 | Sanitize raw error strings in DOM | `src/App.tsx:482-484`, `ErrorBoundary.tsx:58-60` |
| 4.6 | `open_item_file` temp cleanup (TTL/unique subdir) | `ui src-tauri/src/ipc.rs:402-403` |
| 4.7 | Android `Panicked` Kotlin exception variant | `CopypasteBindings.kt:80-85` |

---

## Phase 5 — Release hardening, docs, performance

| # | Item | Files |
|---|---|---|
| 5.1 | daemon `Cargo.toml` add `rust-version.workspace = true` | `crates/copypaste-daemon/Cargo.toml` |
| 5.2 | Single-source version: patch `package.json` + Gradle defaults in release flow | `release.yml:108-130`, `package.json:4`, `android/app/build.gradle.kts:106-107` |
| 5.3 | Create `docs/known-issues.md` (or fix README link) | `README.md:128` |
| 5.4 | Universal macOS DMG (lipo) **or** README Rosetta note | `release.yml:77`, `README.md:42` |
| 5.5 | Add 3 missing error codes to `docs/protocol.md` | `docs/protocol.md` |
| 5.6 | Rewrite `relay-api.md` (random-token auth, wall-clock fields, all routes) | `docs/relay-api.md` |
| 5.7 | Update `SECURITY.md` to match shipped code (constant-time token, random token, ephemeral Win/Linux keystore, real disclosure email) | `SECURITY.md:14,33-36,45,49` |
| 5.8 | Extend `ARCHITECTURE.md` + README crate graphs to all 12 crates | `ARCHITECTURE.md:4-11`, `README.md:55-59` |
| 5.9 | Resolve `#[allow]` missing comments; delete/include orphan `ipc_win.rs` | `daemon/src/lib.rs:14`, `main.rs:1`, `event_tap.rs:1`, `keychain/acl.rs:1`, `daemon/src/ipc_win.rs` |
| 5.10 | `pub(crate)`/feature-gate deprecated empty-AAD `encrypt_item`/`decrypt_item` | `core/src/crypto/encrypt.rs` |
| 5.11 | Relay HKDF salt constant; promote `r2d2` to workspace deps; align `core-foundation`; refresh MSRV comments; ADR-010 path | `core/src/relay.rs:59,85`, `core/Cargo.toml`, `daemon`/`ui` Cargo.toml, `ADR-010:63` |
| 5.12 | Perf: file/image read-before-size-gate pre-check; `poll_interval_ms` hot-reload; config-load warn | `daemon.rs:1733-1742,1178,2538`, `clipboard.rs:461-465` |
| 5.13 | Wire or feature-gate the relay quota/tier system (15+ `dead_code` allows) | `relay/src/state.rs`, `quota.rs` |
