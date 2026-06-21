# CopyPaste — Prioritized Fix Plan

Derived from `AUDIT_FINDINGS.md`. Each finding is a bd issue tagged `[AUDIT-0620 src:...]`. Fixes must preserve the verified contracts: CLI/UI stay IPC-only, relay stays ciphertext-only, core stays free of daemon/UI/CLI deps, no plaintext storage/sync, no secret logging. Prefer small reviewable commits; every behavior fix ships with the regression test named in its bd issue.

---

## Phase 1 — P0/P1: security, data-loss, reliability (23 issues)

No P0 exist. Suggested order (highest user-trust/data-loss risk first):

**1a. Destructive-action safety (7)** — add confirm dialog + undo where missing:
`fjvz` macOS bulk delete · `uw45` macOS revoke-all (real modal) · `w6xc` macOS clear-history · `2ifa` Android single delete · `yel4` Android Clear-All (also surface errors + run drain) · `kaf6` Android delete-undo · plus `d7um` revoke_device atomicity (below).
*Shared models:* consider one cross-platform "destructive action" contract (confirm + undo window) instead of per-screen reinvention.

**1b. Android crypto-failure hardening (3)** — `fkx7` make ABI mismatch fatal in `Application.onCreate`; `hh3w` add explicit ProGuard `-keep` rules for UniFFI entry points + a release-build smoke test that asserts real (not stub) FFI; `mp1x` show a clear unavailable/disabled state when `READ_LOGS` is ungranted.

**1c. DB atomicity & key hygiene (3)** — `j9pv` wrap `upsert_fts` in a transaction; `d7um` wrap `revoke_device` in a transaction; `liaz` propagate `Result` to `main` so `Zeroizing` drops run instead of `process::exit`.

**1d. Export & IPC safety (3)** — `phit` add explicit warning/confirm + audit log to `export(include_sensitive)` and a CLI stderr warning before plaintext; `crol` unify the wire protocol on `id:String` and make daemon+CLI consume the `copypaste-ipc` types (publish what the wire actually is); `ki7p` write the private-mode flag via `write_text_atomic_0600`.

**1e. Lifecycle TOCTOU (1)** — `dl1e` validate process identity (lockfile owner / socket peer creds) before SIGTERM in `evict_stale_daemon`.

**1f. State-truth (2)** — `ei27` reconcile the dark default vs PARITY-SPEC light-first (pick one, fix both platforms + remove stale comments); `8jx8` add Android export/import/backup over UniFFI (or formally document the limitation with a visible disabled state).

**1g. Missing P1 tests (3)** — `sxr1` cross-device relay-auth → 401; `ekzn` `delete_expired_also_cleans_fts`; `ian9` `pake_missing_confirm_rejected`.

## Phase 2 — P2: correctness & reliability (66 issues)

Priority sub-clusters:
- **Reliability:** P2-R03 fail-closed `has_sensitive_items`; P2-R04 move `revoked_devices` into a versioned migration; P2-R06 persistent relay retry queue; P2-R07 supervise relay/P2P tasks; P2-R08 treat poisoned `SyncCrypto` mutex as fatal; D-D3.3 per-request IPC read timeout; D-D3.1 flock-guarded socket takeover.
- **Privacy/UX hygiene:** F-F5/F-F7/F-F9 strip socket paths/usernames/raw IPC text from UI; P2-UX-05/06 Android friendly error mapping; G-F6 re-blur QR after inactivity; G-F7 disable copy on SAS field; P2-UX-08 SyncStatusChip recency gate; CMP-002 remove stale sync warning.
- **Secret hygiene:** B-F02/B-F04 return `Zeroizing` from `derive_storage_key_v1`/`ecdh`; B-F03 extend scrubber to dot-less base64url; E-C1 move `apikey` out of the WSS URL.
- **Storage correctness:** C-F03/C-F05 unify sensitive TTL semantics; C-F09 extend detector keywords; C-F01 document FTS plaintext-at-rest in THREAT-MODEL.
- **Parity features:** I-P2-1..7 (Android P2P sync, additive backends, GUI backup/restore, vacuum/stats UI, degraded-DB recovery, SAS metadata card, all-IPs peer row).
- **Relay hardening:** E-R1 per-device stream cap; E-R2 key rate-limit on authenticated identity; E-S2 enforce `clamp_timestamps` at deserialize; E-P1-1 cert-expiry floor.
- **CI (move to Phase 3):** H-F01..F05.

## Phase 3 — Tests & CI hardening

`H-F01` ESLint + frontend CI (lint/typecheck/test/build) · `H-F02` `--all-features` test job · `H-F03` Android lint on PR · `H-F04`/`H-F06` gitignore + rotate generated keystores · `H-F09` make fuzz blocking · `H-F12` make machete blocking · `H-F10` add emulator step or fix comment · plus the P2/P3 test-coverage backlog (TC-04..10, TC-P3).

## Phase 4 — UI parity & polish (P3 parity/UX)

Android Sort-by-device, sensitive-chip label alignment, glass alpha rationale, Android PairActivity countdown; macOS LogView relative paths, popup-shortcut single source of truth, QR drain-bar 0s flash; CLI migrate to current IPC methods.

## Phase 5 — Performance, release & manual QA hardening

VACUUM scheduling (C-F08); image-dedup full-width hash; launchd per-user templating; doc refresh (ARCHITECTURE.md dep pins, relay token derivation, sync→core edge); then real-device manual QA matrix (macOS↔Android sync, pairing/SAS, private/sensitive, offline transitions, daemon/relay/cloud unavailable).

---

### Cross-cutting recommendation
Several P1/P2 parity issues stem from macOS and Android **independently interpreting the same runtime state** (sync status, device metadata, content-type/sensitive labels). Introduce **shared protocol/state definitions** (served by the daemon over IPC, mirrored 1:1 in the UniFFI layer) rather than letting each client map state on its own — this prevents the whole drift class from recurring. Do **not** achieve parity by letting the UI talk to core directly; the daemon/IPC remain the source of runtime truth.
