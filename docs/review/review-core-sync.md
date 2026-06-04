# Code Review — Core + Sync + Transport Crates

**Reviewer:** Senior Rust engineer (read-only, no code changes)
**Branch:** `v0.6.1-integration`
**Date:** 2026-06-04
**Scope:** `crates/copypaste-core/src/`, `crates/copypaste-sync/src/`,
`crates/copypaste-p2p/src/`, `crates/copypaste-relay/src/`,
`crates/copypaste-supabase/src/`, `crates/copypaste-android/src/`

---

## 1. Code Duplication

### 1.1 `MIN_PASSPHRASE_LEN` defined in two places
**File:line** `crates/copypaste-core/src/crypto/sync_key.rs:185` and
`crates/copypaste-android/src/lib.rs:175`
**Severity: P2**
Both define `const MIN_PASSPHRASE_LEN: usize = 8`. The Android FFI should
import `copypaste_core::MIN_PASSPHRASE_LEN` (which is already `pub`) instead of
re-declaring it. A future bump to the core constant will silently diverge the
Android enforcement unless the local copy is also updated.

### 1.2 `build_cloud_aad` is `fn` (private), not a shared constant
**File:line** `crates/copypaste-core/src/crypto/sync_key.rs:114`
**Severity: P3**
`build_cloud_aad` is a private free function. The Android FFI crate constructs
compatible cloud AAD inline via `encrypt_for_cloud` / `decrypt_from_cloud`
(which call the private function), so there is no immediate duplication bug —
but the function could be exposed as `pub(crate)` with a doc comment and the
symmetry documented more explicitly. Low priority.

### 1.3 Base64 helper duplicated between `pairing_qr.rs` and the QR round-trip in `sync_key`
**File:line** `crates/copypaste-core/src/crypto/pairing_qr.rs:135–137`
**Severity: P3**
`fn b64()` (a trivial engine alias) is defined locally in `pairing_qr.rs`
rather than shared from a crate-level constant or the `base64::engine::general_purpose::URL_SAFE_NO_PAD` constant. Minor but creates a second instantiation point.

### 1.4 INSERT SQL duplicated verbatim between `insert_item` and `insert_item_with_fts`
**File:line** `crates/copypaste-core/src/storage/items.rs:274–300` and `:356–382`
**Severity: P2**
Both functions contain the **identical 19-column INSERT** statement. A future
schema addition that adds a column must be done in two places; the March 2026
`deleted` column was clearly added to both (so no drift exists today), but the
duplication is a maintenance hazard. A helper `fn execute_insert(conn, item,
key_version)` should absorb the common SQL.

### 1.5 Key-hex formatting loop duplicated four times in `db.rs`
**File:line** `crates/copypaste-core/src/storage/db.rs:172–180`, `:429–432`,
`:559–563`, `:585–590`
**Severity: P2**
The loop `for b in key { write!(*hex, "{:02x}", b).unwrap(); }` appears four
times (key_pragma, encrypt_existing, rekey – in-memory path, rekey – file
path). The `key_pragma` function already exists; the other three sites should
call it or a shared `fn key_to_hex(key: &[u8; 32]) -> Zeroizing<String>`
helper. The four `write!(...).unwrap()` inside those loops are justified
(infallible `fmt::Write for String`) but the comment explaining that fact is
repeated in each copy rather than being in one place.

### 1.6 Migration-state table DDL repeated three times in `db.rs`
**File:line** `crates/copypaste-core/src/storage/db.rs:691–699`, `:740–748`,
`:895–905`
**Severity: P2**
`CREATE TABLE IF NOT EXISTS migration_state (…)` is inlined in
`migration_state()`, `migration_v4_sweep_resumable()`, and
`force_migration_complete()`. Extract to a `const MIGRATION_STATE_DDL: &str`
used by all three callers.

### 1.7 `SyncError` is not `thiserror`-derived but `SyncEngine`'s error is hand-implemented
**File:line** `crates/copypaste-sync/src/engine.rs:55–90`
**Severity: P3**
`SyncError` has a hand-rolled `Display` impl and manual `From` conversions
while every other crate uses `thiserror`. No correctness issue but it is
inconsistent with project style.

### 1.8 `content_type` magic strings duplicated without a shared constant
**File:line** `crates/copypaste-core/src/storage/items.rs:132,187,235`,
`crates/copypaste-android/src/lib.rs:1332,1489,1531–1533`,
`crates/copypaste-sync/src/merge.rs:127`
**Severity: P2**
The strings `"text"`, `"image"`, `"file"` are compared by equality in at least
seven distinct files across three crates without a shared enum or constant set.
A misspelled string (e.g. `"image "` with a trailing space) produces a silent
wrong-type classification rather than a compile-time error. A `ContentType`
enum or `pub mod content_type { pub const TEXT: &str = "text"; … }` in
`copypaste-core` would give every crate one authoritative reference.

---

## 2. Dead / Unused Code

### 2.1 `encrypt_item` / `decrypt_item` (bare empty-AAD functions) are `pub` despite being deprecated
**File:line** `crates/copypaste-core/src/crypto/encrypt.rs:200–224`
**Severity: P2**
Both are marked `#[deprecated]` but still `pub`-re-exported from `lib.rs:28`
with `#[allow(deprecated)]`. The `pub` export is reachable from every
downstream crate. Only the benchmarks in `copypaste-bench` and one comment in
`android/lib.rs:2168` still reference them; no production call site uses them.
They should either be deleted or downgraded to `pub(crate)` with a planned
removal milestone. Leaving them `pub` risks a future caller who imports them
without noticing the deprecation.

### 2.2 `DeviceKeypair::secret_key_bytes` is deprecated but retained for "cross-crate ABI stability"
**File:line** `crates/copypaste-core/src/crypto/keys.rs:179–191`
**Severity: P2**
The deprecation comment says "a follow-up patch should migrate every caller and
delete or `#[deprecated]` it." It has been formally `#[deprecated]` for some
time; the surviving callers should be found (they are `copypaste-daemon`'s
platform/macOS module, per the comment) and migrated to `secret_key_bytes_zeroizing`.
The method is a security risk: it returns an unscrubbed `Copy` type.

### 2.3 `PAIRING_QR_MAGIC_V2` is not publicly exported from `copypaste-core`
**File:line** `crates/copypaste-core/src/crypto/mod.rs:13` and
`crates/copypaste-core/src/lib.rs:34`
**Severity: P3**
`crypto/mod.rs` re-exports `PAIRING_QR_MAGIC` (the v1 constant) but not
`PAIRING_QR_MAGIC_V2`. Any caller that needs to emit or match only the v2 prefix
would either hard-code the string `"CPPAIR2"` or re-import from
`copypaste_core::crypto::pairing_qr::PAIRING_QR_MAGIC_V2`. The v2 constant
should be added to the re-export list alongside the v1 one.

### 2.4 `derive_key_v2` is `fn` (private) but its three public wrappers are thin facades
**File:line** `crates/copypaste-core/src/crypto/keys.rs:60–88`
**Severity: P3**
`derive_key_v2` / `derive_storage_key_v2` / `derive_sync_key_v2` / `derive_telemetry_key_v2`
are all public but `derive_key_v2` itself is private. The three wrappers are
correct and domain-separating; no issue other than the asymmetry could confuse
a reviewer into thinking the separation is accidental.

### 2.5 `CloudClipboardRow::is_tombstone()` is a one-liner that just returns `self.deleted`
**File:line** `crates/copypaste-supabase/src/models.rs:94–97`
**Severity: P3**
The method exists for readability but is inlined at exactly one call site in
supabase/lib.rs. Not harmful, but surfaces as a thin wrapper with no additional
behaviour.

---

## 3. Competing / Duplicate State / Sources of Truth

### 3.1 Two schema-version namespaces with overlapping numeric values
**File:line** `crates/copypaste-core/src/storage/schema.rs:57` (`SCHEMA_VERSION = 10`)
vs `crates/copypaste-core/src/crypto/encrypt.rs:30,37`
(`AAD_SCHEMA_VERSION = 3`, `AAD_SCHEMA_VERSION_V4 = 4`)
vs `crates/copypaste-core/src/crypto/sync_key.rs:107`
(`CLOUD_AAD_SCHEMA_VERSION = 5`)
**Severity: P2**
`SCHEMA_VERSION` (SQLite schema, now 10) and `AAD_SCHEMA_VERSION*` (crypto
binding, 3/4/5) are unrelated integers but share the word "schema" without a
clear naming separation. A comment in `encrypt.rs:19` acknowledges this
confusion: *"Stored locally as a compile-time constant rather than
re-exporting from `storage::schema` to avoid a cross-module merge race."* That
rationale is obsolete (no concurrent workers here); the two namespaces should
be explicitly documented as independent counters with different semantics, and
the crypto ones renamed (e.g. `AEAD_BINDING_VERSION`, `AEAD_BINDING_VERSION_V4`)
to prevent accidental conflation.

### 3.2 `device_id` in `CloudClipboardRow` vs `origin_device_id` in `ClipboardItem` / `WireItem`
**File:line** `crates/copypaste-supabase/src/models.rs:55` vs
`crates/copypaste-core/src/storage/items.rs:61` and
`crates/copypaste-sync/src/protocol.rs:78`
**Severity: P2**
The Supabase row calls the origin field `device_id`; the core storage layer and
wire protocol call it `origin_device_id`. The daemon's cloud.rs must map between
these two names on every upload and download. Any new code path that forgets
the rename silently produces an empty origin (the empty-string default) which
breaks LWW tie-breaking determinism. The Supabase model should use `origin_device_id`
as its field name to match the rest of the stack, or the discrepancy should be
documented as intentional at the Supabase schema level.

### 3.3 `WireItem.key_version` defaults to `2` in `default_key_version()` for backward compat
**File:line** `crates/copypaste-sync/src/protocol.rs:140–142`
**Severity: P1**
The default chosen for forward-compat (when an old peer omits the field) is 2,
because "defaulting to 1 would resurrect the original bug." This is correct for
**today's** codebase, but it creates a hidden invariant: any future third key
version must also change this default or the "unknown version from old peer"
case will silently attempt v2 decryption of a v3 ciphertext and fail with
`AuthFailed` instead of `UnknownKeyVersion`. The default must be documented as
"the minimum version all supported peers are guaranteed to use", and its value
must be updated in lockstep with any future HKDF v3 introduction.

### 3.4 `ITEM_KEY_VERSION_CURRENT = 2` in `items.rs` and `P2P_WIRE_KEY_VERSION` in `android/lib.rs`
**File:line** `crates/copypaste-core/src/storage/items.rs:264` and
`crates/copypaste-android/src/lib.rs:779`
**Severity: P2**
Android defines `const P2P_WIRE_KEY_VERSION: u8 = ITEM_KEY_VERSION_CURRENT as u8`
which is a fine derivation, but the cast means a future `ITEM_KEY_VERSION_CURRENT > 255`
would truncate silently. More importantly, `ITEM_KEY_VERSION_CURRENT` is an
`i64` (storage column type) while the wire type is `u8`; the cast is currently
lossless but the semantic mismatch across layers — storage uses `i64`, wire uses
`u8`, the crypto dispatcher (`decrypt_item_by_version`) takes `u8` — should be
unified behind a single typed version newtype.

### 3.5 QR format version encoding (`CPPAIR1` / `CPPAIR2`) with single-char-digit embedded in magic
**File:line** `crates/copypaste-core/src/crypto/pairing_qr.rs:94–100`
**Severity: P2**
The version is embedded as a digit in the magic string (`CPPAIR1`, `CPPAIR2`)
rather than as a separate version field. `PAIRING_QR_MAGIC` (v1 string) is
exported but `PAIRING_QR_MAGIC_V2` is not (see §2.3). The decode dispatcher
does a string match on the first component; a v3 format would require yet
another constant and another match arm but there is no version constant (`1u8`,
`2u8`) that can be compared numerically — the version is encoded only as an
ASCII digit in the string. A future migrator cannot programmatically compare
`version < 2` without re-parsing the string.

---

## 4. Weird / Buggy Behavior

### 4.1 `encrypt_text` / `decrypt_text` (Android FFI) uses `AAD_SCHEMA_VERSION` (v3) not v4
**File:line** `crates/copypaste-android/src/lib.rs:89,118`
**Severity: P1 (latent data-loss bug)**
The exported `encrypt_text` / `decrypt_text` pair builds the AAD as
`build_item_aad(&item_id, AAD_SCHEMA_VERSION)` — i.e. `"{item_id}|3"` — but
the daemon's read path calls `decrypt_item_by_version(key_version=2, …)` which
constructs `build_item_aad_v2(item_id, 4, 2)` → `"{item_id}|4|2"`. Any
item encrypted via the exported `encrypt_text` function on Android and then
read by the daemon (e.g. after P2P sync) would fail with `AuthFailed` because
the AAD bound into the ciphertext does not match what the daemon reconstructs.
The live `add_clipboard_item` (feature-gated) correctly uses `build_item_aad_v2`
(see lib.rs:2207), but `encrypt_text` does not. This means the raw
`encrypt_text` / `decrypt_text` FFI functions are **only safe when both encrypt
and decrypt stay on Android**, which is a narrow and fragile invariant.
The fix is to make `encrypt_text` take a `key_version: u8` parameter and
branch on v1/v2 AAD format, or to document explicitly that the function is
local-only and mark it with a comment warning against cross-device use.

### 4.2 `last_err.expect("loop runs at least once so last_err is set on failure")` in production code
**File:line** `crates/copypaste-p2p/src/transport.rs:606`
**Severity: P2**
`connect_with_retry` uses `.expect(…)` on `last_err` at the end of the retry
loop. The comment is correct (the guard at line 547 ensures `MAX_CONNECT_ATTEMPTS > 0`)
and the logic is sound — but `expect` in production code violates the project
convention ("no `unwrap()` in non-test code unless provably infallible and
explained in a comment"). The comment is there but the preferred form would be
to use an `unreachable!("…")` or restructure to avoid the `Option<>` by using
`return Err(err)` inline at the last iteration. With `MAX_CONNECT_ATTEMPTS = 4`
hardcoded this is hard to trigger, but the guard against `MAX_CONNECT_ATTEMPTS == 0`
is itself reached via the `if` at line 547 which is dead code for the current
const value — it can never be zero.

### 4.3 `HKDF_SALT_V2` (local-storage v2) is a different constant from `HKDF_SALT_V2_BASE` (per-pair v2)
**File:line** `crates/copypaste-core/src/crypto/keys.rs:100–103` and `:37`
**Severity: P1 (confusion risk, potential hard-fork hazard)**
Two constants with nearly identical names serve different purposes:
- `HKDF_SALT_V2_BASE`: a *prefix* used to compute per-pair salts for sync/telemetry keys.
- `HKDF_SALT_V2`: the *exact* 32-byte salt for the single-device local-storage derivation.
These are used by completely separate code paths (`derive_v2` vs `hkdf_v2_pair_salt`).
A developer reading the names might confuse them and apply the wrong one. The local-storage
constant should be renamed `HKDF_SALT_V2_LOCAL` (or `LOCAL_STORAGE_HKDF_SALT_V2`) to
make the distinction clear. There is a golden-byte test pinning `HKDF_SALT_V2` which would
catch an accidental substitution, but the naming confusion is a maintenance hazard.

### 4.4 `DeviceKeypair::ecdh()` contains a misleading intermediate `Zeroizing` pattern
**File:line** `crates/copypaste-core/src/crypto/keys.rs:211–219`
**Severity: P2**
```rust
let buf: zeroize::Zeroizing<[u8; 32]> = zeroize::Zeroizing::new(*shared.as_bytes());
*buf
```
The comment correctly notes "this only narrows the leak window — it does not eliminate it."
The pattern zeroizes `buf` on drop but immediately copies out of it via `*buf` (a `Copy`
deref), leaving the raw bytes on the caller's stack. The method is `#[deprecated]` in spirit
(the comment says to prefer `ecdh_zeroizing`) but is NOT formally `#[deprecated]`, unlike
`secret_key_bytes`. It should receive the same formal deprecation so callers get a compile
warning.

### 4.5 Migration-state sweep treats "permanently unrotatable" rows as complete
**File:line** `crates/copypaste-core/src/storage/db.rs:800–832`
**Severity: P2 (data accuracy)**
`migration_v4_sweep_resumable` marks the sweep `Complete` even when
`remaining > 0` (unrotatable v1 rows), emitting a warning. The logic is
intentional (to release the write-gate) but the `MigrationState::Complete`
state is now semantically ambiguous: it can mean "all rows rotated cleanly" OR
"all rows attempted, some permanently failed." Callers that check
`migration_state() == Complete` cannot distinguish these two cases.
`count_dead_v1_rows()` exists for the second case but the distinction is not
enforced at the type level.

### 4.6 `sanitize_fts5_query` drops `-` (hyphen) by replacing with space, affecting FTS behavior
**File:line** `crates/copypaste-core/src/storage/items.rs:1124–1127`
**Severity: P2 (subtle behavior)**
The sanitizer rewrites `"-"` to `" "` (space) so `foo-bar` becomes two AND-ed
terms `foo* AND bar*`. The comment explains the reason (FTS5 treats `-` as
a NOT operator). However, this means a query for a hyphenated product code like
`MX-5` will search for `MX* AND 5*` — all items containing both `MX` and `5`
separately — rather than the hyphenated string. Users expecting exact hyphenated
matches will get over-broad results. This may be the intended tradeoff, but
there is no test for the case and no comment explaining the user-visible impact.

### 4.7 `wall_time` upper-bound clamp in `SyncEngine::run_session` is absolute, not relative
**File:line** `crates/copypaste-sync/src/engine.rs:419–425`
**Severity: P2**
`MAX_WALL_TIME_SKEW_MS = 10^12 ms ≈ year 2001` (Unix ms from epoch). Wait — it
is 10^12 ms from the epoch which is roughly 2001-09-09, already in the past for
any real-world host. This means **any real item** captured after 2001-09-09 that
a peer transmits will have its `wall_time` silently clamped to `10^12 ms` if a
malicious peer sends `wall_time > 10^12`. However, *honest peers also have
`wall_time > 10^12`* for current timestamps (2026 ≈ 1.75 × 10^12 ms). Inspecting
the constant: `1_000_000_000_000_i64` — this is indeed in the past for a 2026
host. The intent was clearly "far future" but the constant is "recent past".

> Actual calculation: `10^12 ms / (1000 * 60 * 60 * 24 * 365.25)` ≈ 31.7 years from epoch = year ~2001.7.

Any honest peer sending a 2026 item will have `wall_time ≈ 1.75 × 10^12`, which
is `> MAX_WALL_TIME_SKEW_MS = 10^12`, causing it to be **clamped with a warning
logged**. The item is still accepted (not dropped), but the stored `wall_time`
is set to the constant (2001), which corrupts the LWW wall-time tie-break and
the UI display order. This is a **critical latent bug** — it fires on every
incoming sync item from every peer.

**Note:** The `lamport_ts` ceiling is relative (`local_clock + MAX_LAMPORT_SKEW`)
which is correct; the `wall_time` ceiling should similarly be relative
(`now_ms + some_margin`) rather than absolute.

### 4.8 `SyncEngine::run_session` sends ITEMS before receiving peer ITEMS, potential deadlock on small buffers
**File:line** `crates/copypaste-sync/src/engine.rs:328–357`
**Severity: P2**
The protocol order is: both sides send ITEMS before either reads ITEMS. For
small payloads this works because OS TCP buffers absorb the data. For large
batch syncs (hundreds of image items approaching the 16 MiB frame cap) both
sides can fill the TCP send buffer simultaneously, each waiting for the other
to read — a classic head-of-line block that degrades to a deadlock.
The comment at line 325 acknowledges the send-then-receive order but does not
address the flow-control risk. The fix is an interleaved or channel-based
approach; as-is the 16 MiB single-frame limit mitigates the worst case but does
not eliminate it for two large ITEMS frames.

---

## 5. Architecture Smells

### 5.1 `build_cloud_aad` is `fn` (crate-private) but its schema-version constant is `pub`
**File:line** `crates/copypaste-core/src/crypto/sync_key.rs:107–116`
**Severity: P2**
`CLOUD_AAD_SCHEMA_VERSION = 5` is `pub` and exported from `lib.rs`, but
`build_cloud_aad` (which formats it) is `fn` (crate-private). Any caller outside
`copypaste-core` that wants to verify AAD compatibility must re-implement the
format string `"{item_id}|5"` themselves. Making `build_cloud_aad` public (or
exporting it as `pub use`) would complete the abstraction.

### 5.2 `copypaste-android/src/lib.rs` is a 3000+ line monolith
**File:line** `crates/copypaste-android/src/lib.rs` (entire file)
**Severity: P2**
The Android FFI layer contains crypto, storage, P2P sync, cloud sync, relay
derivation, QR pairing, device management, and retention logic all in one file.
The file exceeds 3,000 lines. Module split mirrors already exist (`pairing.rs`,
`p2p_listener.rs`) for newer features. The core FFI surface (`encrypt_text`,
`decrypt_text`, cloud-sync functions, `add_clipboard_item`) should be extracted
into `crypto.rs`, `storage.rs`, `sync.rs` sub-modules to reduce the cognitive
surface and the likelihood that a future feature inadvertently imports the wrong
constant or function from the wrong context.

### 5.3 FFI ABI versioning is a single monotonic integer with no backward-compat range
**File:line** `crates/copypaste-android/src/version.rs:212`
**Severity: P2**
`UNIFFI_ABI_VERSION = 15` (current). The `check_compatibility` function does an
exact equality check — the Kotlin bindings must match exactly. This means every
additive-only ABI change (adding an optional field) forces a full Kotlin
regeneration. The version.rs comment documents why each bump happened, which is
excellent, but the design has no concept of a minimum compatible version or a
range (`>= 14 && <= 15`). For future optional-field additions, a minimum-version
comparison (`kotlin_abi >= MIN_SUPPORTED_ABI`) would avoid unnecessary forced
regenerations.

### 5.4 `CloudClipboardRow` does not carry `key_version` — the Supabase transport is blind to encryption generation
**File:line** `crates/copypaste-supabase/src/models.rs:28–97`
**Severity: P1**
The Supabase row (`CloudClipboardRow`) stores `payload_ct` as an opaque string
but has no `key_version` field. The receiving daemon must decrypt `payload_ct`
using the cloud `SyncKey` path (Argon2id-derived, `CLOUD_AAD_SCHEMA_VERSION = 5`),
which is correct for cloud-only ciphertexts. However, if the system ever
transitions from a passphrase-derived cloud key to a per-pair key (v2 HKDF
family), there is no wire-level field to signal which generation encrypted the
payload. The design is currently sound *only* because there is a single cloud
key generation. Any future cloud key rotation must either introduce a new
column or embed the version in the payload itself; the absence of `key_version`
is an architectural assumption that should be documented explicitly in the struct.

### 5.5 `PeerCertVerifier` duplicates both `verify_tls12_signature` and `verify_tls13_signature` — server and client share identical bodies
**File:line** `crates/copypaste-p2p/src/verifier.rs:128–158` (ClientCertVerifier)
and `:214–244` (ServerCertVerifier)
**Severity: P3**
Both `ClientCertVerifier` and `ServerCertVerifier` impls on `PeerCertVerifier`
provide identical `verify_tls12_signature`, `verify_tls13_signature`, and
`supported_verify_schemes` implementations (four methods × 2 traits = 8 total
method bodies, all identical). A shared helper function or a `default` impl
macro would remove the duplication.

---

## Top 10 Issues to Fix First

Ranked by risk (security / data-loss first, then correctness, then maintainability):

| # | Severity | Finding | Risk |
|---|----------|---------|------|
| 1 | **P1** | §4.7 — `MAX_WALL_TIME_SKEW_MS = 10^12` is in the past; ALL current-timestamp sync items are clamped and get `wall_time` set to 2001-09-09, corrupting LWW and UI ordering | Data corruption on every P2P sync |
| 2 | **P1** | §4.1 — `encrypt_text`/`decrypt_text` use v3 AAD (`"{item_id}\|3"`); daemon decrypts v2-key items with v4 AAD (`"{item_id}\|4\|2"`); cross-device use fails with `AuthFailed` | Silent decryption failure for Android items synced to macOS |
| 3 | **P1** | §3.3 — `WireItem.key_version` default is hardcoded `2`; undocumented invariant breaks when v3 keys ship | Future-proof: must be updated before any new HKDF version |
| 4 | **P1** | §5.4 — `CloudClipboardRow` has no `key_version` field; Supabase transport is blind to encryption generation | Breaks cloud sync if cloud-key generation ever changes |
| 5 | **P2** | §4.3 — `HKDF_SALT_V2` vs `HKDF_SALT_V2_BASE` naming confusion; wrong constant used = silent hard-fork of all local-storage or all per-pair keys | Naming hazard; hard-fork if swapped |
| 6 | **P2** | §3.2 — `device_id` (Supabase) vs `origin_device_id` (core/wire) naming divergence; silent empty-origin on wrong mapping | LWW tie-break breaks for cloud-synced items with missing origin |
| 7 | **P2** | §1.4 — 19-column INSERT duplicated in `insert_item` and `insert_item_with_fts` | Schema drift risk on future column additions |
| 8 | **P2** | §4.8 — Both peers send ITEMS before reading; can deadlock on large batch syncs near the 16 MiB frame cap | Latent deadlock for large image batches |
| 9 | **P2** | §2.1 — Deprecated bare `encrypt_item`/`decrypt_item` remain `pub` | Any future call site bypasses AAD binding, enabling replay |
| 10 | **P2** | §1.1 — `MIN_PASSPHRASE_LEN` duplicated in `sync_key.rs` and `android/lib.rs`; divergence risk | Enforcement drift between platforms |

---

## Summary

The reviewed crates are generally well-engineered: AEAD AAD binding is correct
in the storage and cloud paths, constant-time comparisons are used where
required (relay token, QR token equality, cert fingerprint), the migration
system is atomic and resumable, and P2P mTLS has solid defenses (SNI sentinel,
fingerprint pinning, cert-rotation grace). The top issues are:

- **A critical wall_time constant bug** (§4.7) that silently corrupts every
  inbound sync item's recency ordering because `10^12 ms` is the year 2001.
- **An AAD mismatch** (§4.1) in the exported Android `encrypt_text` / `decrypt_text`
  FFI functions — v3 AAD used, but daemon expects v4 AAD for `key_version = 2`
  rows.
- **A missing `key_version` field** on `CloudClipboardRow` (§5.4) that makes
  the Supabase transport architecture load-bearing on a single crypto generation.
- Naming confusion between `HKDF_SALT_V2` and `HKDF_SALT_V2_BASE` (§4.3) and
  between `device_id` vs `origin_device_id` (§3.2).
- A cluster of code-duplication P2s (duplicated INSERT SQL, migration DDL,
  key-hex loop, content_type strings) that create maintenance hazard.
