# CopyPaste — Production Audit Report

**Date:** 2026-06-19 · **Version audited:** 0.7.4 (`Cargo.toml`) · **Tracking:** CopyPaste-o7me
**Scope:** full repository — architecture, crypto, storage, daemon, IPC, relay, sync/P2P/cloud, Android/UniFFI, Tauri/UI + parity, CLI, CI/release/packaging, docs.
**Method:** 10 parallel read-only review streams over the source tree, plus the runnable quality gates. Details in `AUDIT_FINDINGS.md`; remediation order in `AUDIT_FIX_PLAN.md`.

---

## Executive summary

CopyPaste is a **well-engineered, security-conscious** codebase. The cryptographic core, sync conflict-resolution, transport authentication, and storage-at-rest are implemented to a high standard and, in several places, **the shipped code is stronger than its own documentation claims**. The architecture contract (CLI/UI never link core; relay handles only ciphertext; daemon owns crypto/Keychain/socket) holds, with **one** boundary leak (CLI writes the Keychain directly).

**No P0 (critical, exploitable-now) issues were found.** There is no plaintext exfiltration, no key leakage to the relay/cloud, no MITM (cert-fingerprint pinning + PAKE/TLS channel binding are correct), no nonce-reuse, and no data-loss-on-delete (FTS plaintext is cleaned on every delete path).

The findings cluster at **P1/P2** and fall into four themes:

1. **One stated security guarantee is not enforced in code** — "sensitive items are never uploaded" (relay-api.md:105). They *are* uploaded (E2E-encrypted) because the push paths have no `is_sensitive` filter. Encrypted, so not a plaintext breach, but the guarantee is false.
2. **Reliability/lifecycle sharp edges in the daemon** — a reachable-by-refactor `unreachable!()`, a blocking subprocess on the async tick, fail-open app-exclusion (password managers get captured if `lsappinfo` fails), and a systemd unit that silently breaks DB writes on Linux.
3. **Documentation drift** — `relay-api.md`, `SECURITY.md`, `ARCHITECTURE.md`, `protocol.md` describe older designs; a README link 404s; version strings are inconsistent (`package.json`/Android at 0.7.1 vs 0.7.4).
4. **Hardening + hygiene** — Tauri WebView has no CSP; several detector regexes auto-wipe legitimate data; a handful of unzeroized key copies; Android key-cache retention.

## Overall verdict

**Conditionally production-ready for the macOS-primary, P2P/relay use case** — after the P1 set is addressed. The crypto/sync/storage foundations are sound enough to ship; the blockers are (a) the sensitive-items-to-relay guarantee mismatch, (b) the daemon reliability edges, (c) the CSP gap, and (d) release/version/doc correctness. None require architectural rework.

## Risk level

| Dimension | Level | Basis |
|---|---|---|
| Cryptography & key management | **Low** | XChaCha20 + correct AAD/HKDF/zeroize/constant-time; PAKE+SAS+pinning verified |
| Data-at-rest | **Low** | SQLCipher fails-closed; FTS plaintext cleaned on all delete paths |
| Sync correctness | **Low** | Lamport LWW + tombstones convergent; resurrection bug already fixed/tested |
| Privacy guarantees | **Medium** | sensitive-to-relay mismatch; fail-open exclusion; mDNS name broadcast |
| Daemon reliability | **Medium** | panic path, blocking subprocess, Linux systemd misconfig |
| UI hardening | **Medium** | null CSP (no current sink) |
| Release/packaging/docs | **Medium** | version drift, arm64-only vs README, dead link, doc drift |
| **Aggregate** | **Medium** | no P0; concentrated, fixable P1s |

---

## What was verified (high-confidence, code-cited)

- **Boundaries:** CLI & UI do not link `copypaste-core` (IPC-only); relay is standalone, ciphertext-only, never calls `PRAGMA key`; daemon is the sole crypto/Keychain/socket owner (except CLI Keychain write, P1-6).
- **Crypto:** XChaCha20-Poly1305 / 24-byte OsRng nonce; AAD = `(item_id, schema_version, key_version)`; distinct HKDF info-strings; `ZeroizeOnDrop` on all secret types; Keychain `ThisDeviceOnly`; constant-time compares; Argon2id (m=19 MiB); OPAQUE-PAKE bound to TLS exporter; SAS un-bypassable; no secret in QR image.
- **Transport:** mTLS pins peer cert SHA-256 fingerprint (rejects others); per-transport key separation; cloud key only over post-PAKE tunnel.
- **Storage:** SQLCipher enabled + fails closed; FTS plaintext isolated and cleaned on every delete/clear/TTL/evict path; atomic, additive migrations; private mode skips store+sync.
- **Daemon:** socket `chmod 0600` (dir 0700); self-copy echo-loop guard; graceful shutdown + stale-socket cleanup; no prod `unwrap()`.
- **Relay:** constant-time token compare; per-device rate limit 60/min; quota 500/inbox; TTL; no secrets logged; no enumeration oracle.
- **Android:** panic boundary on every FFI export; ABI-17 equality enforced; no secrets in logcat; FGS lifecycle correct; no committed release keystore.
- **Quality gates that ran:** `cargo audit` clean, `cargo deny` clean, design-token parity 53/53, UI 171 tests + production build green.

## What could NOT be verified

- **Rust compile-time gates** (`cargo fmt --check`, `clippy -D warnings`, `cargo test`, `cargo check`) — **BLOCKED**: local toolchain is rustc **1.95.0**, workspace MSRV is **1.96** (`Cargo.toml:26`). Builds hard-error. CI installs ≥1.96, so this is a local-environment limitation; it was not forced via an MSRV edit (out of scope for a non-destructive audit). **All correctness findings are from static reading, not execution.**
- Android `.aar`/Gradle/UniFFI builds — not run (same toolchain block + OOM guard on cross-compile).
- Runtime/behavioral confirmation of sync convergence, MITM-abort, tombstone resurrection — backed by in-repo tests (cited) but not executed here.
- Server-side Supabase RLS SQL (lives outside the repo); published-DMG SHA-256; presence of CI secrets (`ANDROID_KEYSTORE_BASE64`, `TAP_DISPATCH_TOKEN`).
- Very large files were not read line-by-line (daemon `ipc.rs` ~683 KB, `cloud.rs` ~261 KB); some method-level param/error-code coverage is sampled, not exhaustive.

---

## Findings rollup

| Severity | Count | Headline items |
|---|---|---|
| **P0** | 0 | — |
| **P1** | 13 | sensitive-items-to-relay vs guarantee; lsappinfo fail-open captures password managers; daemon `unreachable!()`; Linux systemd `ReadWritePaths`; lsappinfo blocking on async tick; CLI Keychain write + plaintext password over IPC; Tauri null CSP; Android key retained in cache; daemon missing `rust-version`; version drift 0.7.1/0.7.4; known-issues.md 404; README x86_64 claim; protocol.md missing 3 error codes |
| **P2** | ~26 | detector false-positives above auto-wipe floor; no startup purge; export unlogged; IPC error-code gaps; core ships `tracing-subscriber`; 9 unzeroized key copies; Android cache use-after-close; relay-api.md/SECURITY.md/ARCHITECTURE.md/protocol.md drift; audit.yml retry; `#[allow]` comments; orphan `ipc_win.rs`; relay unauth `GET /devices`; UI parity gaps |
| **P3** | ~24 | deprecated empty-AAD `pub`; relay HKDF None salt; rapid-burst item loss; config-error swallowed; mDNS name broadcast; stale MSRV comments; relay dead_code allows; Cask Sonoma floor; temp-file cleanup; ADR-010 path; placeholder security email |

Biggest risks, in order: **(1)** sensitive-items-to-relay guarantee mismatch (privacy/trust), **(2)** daemon reliability trio (panic path + blocking subprocess + fail-open exclusion), **(3)** Linux systemd silent data loss, **(4)** Tauri null CSP, **(5)** release version drift + arm64-only-vs-README.

## Commands run (and results)

```
cargo audit                         → ✅ no advisories (1 stale ignore in deny.toml)
cargo deny check                    → ✅ advisories/bans/licenses/sources ok
node scripts/parity-check.mjs       → ✅ PASS 53/53 tokens within ±5
pnpm run test  (vitest, in UI)      → ✅ 171 passed / 18 files
pnpm run build (tsc && vite build)  → ✅ clean
cargo fmt --check / clippy / test   → ⛔ BLOCKED: rustc 1.95.0 < MSRV 1.96
cargo check -p copypaste-ipc        → ⛔ "rustc 1.95.0 is not supported … requires rustc 1.96"
```

## Recommended next steps

1. **Decide & enforce the sensitive-sync contract** (P1-1): add an `is_sensitive` filter to the relay/cloud/P2P push paths, or correct the docs. This is the one finding that touches a user-facing trust promise.
2. **Land the daemon reliability fixes** (P1-2…P1-5): `spawn_blocking` the `lsappinfo` call + warn-on-failure (consider fail-closed), replace `unreachable!()`, fix the systemd `ReadWritePaths`.
3. **Set a Tauri CSP** (P1-7) and close the CLI→Keychain boundary leak (P1-6).
4. **Fix release correctness** (P1-9…P1-13): daemon `rust-version`, single-source the version across `package.json`/Gradle, create `known-issues.md`, reconcile README x86_64 (universal DMG or Rosetta note), add the 3 missing protocol error codes.
5. **Refresh the docs** that drifted (`relay-api.md`, `SECURITY.md`, `ARCHITECTURE.md`, `protocol.md`) — several currently understate the implemented security.
6. **Tune the detector** (P2): drop the over-broad patterns below the 0.70 auto-wipe floor or add validators; add the missing cloud-credential patterns.
7. Re-run the full Rust gate suite on a ≥1.96 toolchain to convert the BLOCKED gates to verified.

See `AUDIT_FIX_PLAN.md` for the phased plan.
