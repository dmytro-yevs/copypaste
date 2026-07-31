# Port Manifest 02 — Crypto Subsystem

> Harvested from `copypaste` v0.4.1 (`crates/copypaste-core/src/crypto/**`, plus
> `copypaste-daemon/src/keychain/**`, `copypaste-p2p/src/pake.rs`, and
> `copypaste-core/src/relay.rs`).
> This is an **implementation-independent specification**. It exists so a
> from-scratch, library-first rewrite can (a) keep every security property the
> current code earned through bug fixes, and (b) still decrypt data written by
> v0.4.x installs.
>
> **Rule for the rewrite:** every byte string in section 3 is a frozen external
> contract. Changing any of them is a hard fork of existing user data and MUST
> be accompanied by an explicit, tested migration.

## 1. Purpose & scope

### 1.1 What the crypto subsystem is responsible for

Five independent cryptographic domains, deliberately key-separated so a
compromise in one cannot read another:

| Domain | Key source | Cipher / AAD | Persisted where |
|---|---|---|---|
| **D1 — Database at rest** | seed K1 (from the X25519 device secret) | SQLCipher AES-256-CBC via `PRAGMA key` | the SQLite file |
| **D2 — Per-item local AEAD** | seed K1 (v1 rows) / `derive_v2(seed)` K2 (v2 rows) | XChaCha20-Poly1305, AAD A-v3 / A-v4 | `clipboard_items.content` + `.content_nonce` |
| **D3 — Chunked blobs (image / file / thumbnail)** | same as D2, selected by the row's `key_version` | XChaCha20-Poly1305, AAD A-chunk | `clipboard_items.content` as a `CHUNK_FORMAT_V1` container |
| **D4 — Cloud / relay sync** | Argon2id over (passphrase, per-account salt) → `SyncKey` | XChaCha20-Poly1305, AAD A-cloud | relay inbox / Supabase, never local |
| **D5 — Device pairing** | OPAQUE PAKE over a QR-transported 256-bit token, then HKDF | session key → TLS channel binding → confirmation tags / SAS / peer content key | `peers.json` (`PasswordFile`), Keychain |

D2 is layered *inside* D1: item content is AEAD-encrypted **and then** stored in
an already-encrypted database. That is intentional — SQLCipher protects the file,
the per-item AEAD binds each ciphertext to its `item_id` so a row-swap inside an
already-decrypted DB is still detected.

### 1.2 What this document covers

* Every key derivation, with verbatim salt/`info` bytes and the reason each
  exists (§3.2).
* Every AAD construction, verbatim (§3.3).
* Every externally-visible byte layout: the chunk format, the image/file blob
  container, the cloud blob, the pairing QR envelope, the PAKE password file
  (§3.4–3.7).
* Keychain and file-store identifiers and fallback behaviour (§3.8).
* The invariants that were *earned* — each one traceable to a specific bug or
  audit finding (§2).
* The tests that must exist for the rewrite to be trustworthy (§5).
* What to drop (§6).

### 1.3 Out of scope

mTLS certificate generation and fingerprint pinning (`copypaste-p2p::cert`,
`::verifier`), the relay HTTP API surface, the IPC protocol, and the FTS
sensitive-exclusion rules. The PAKE handshake state machine is covered only to
the extent that it produces key material or persists a byte format; see ADR-008
and a separate p2p manifest for the handshake itself.

### 1.4 Source files harvested

```
crates/copypaste-core/src/crypto/mod.rs
crates/copypaste-core/src/crypto/keys.rs            (647 lines, incl. tests)
crates/copypaste-core/src/crypto/encrypt.rs         (474)
crates/copypaste-core/src/crypto/chunks.rs          (388)
crates/copypaste-core/src/crypto/sync_key/{mod,tests}.rs      (452 + 397)
crates/copypaste-core/src/crypto/pairing_qr/{mod,payload,token,wire,tests}.rs
crates/copypaste-core/src/image/blob.rs             (chunk container)
crates/copypaste-core/src/image/crypt.rs, src/file.rs
crates/copypaste-core/src/relay.rs                  (relay HKDF + PoP)
crates/copypaste-core/src/storage/items/{mod,types,ids,query,insert}.rs
crates/copypaste-core/src/storage/migration_v4/{mod,text,images,repair}.rs
crates/copypaste-core/src/storage/db/{mod,pragmas}.rs
crates/copypaste-daemon/src/keychain/{mod,device_key,file_store,secure_write,acl,signing,supabase_password}.rs
crates/copypaste-daemon/src/daemon/startup/keyload.rs
crates/copypaste-daemon/src/ipc/pairing_ops_provisioning.rs
crates/copypaste-p2p/src/pake.rs
docs/adr/ADR-001-xchacha20-not-aesgcm.md, ADR-003, ADR-008
docs/security/THREAT-MODEL.md
```

## 2. Invariants (MUST hold)

### 2.1 Backward compatibility with data written by v0.4.x

**I-1 — Two local key generations must both be readable, forever.**
A v0.4.x database can contain `clipboard_items` rows with `key_version = 1` and
`key_version = 2` simultaneously (the rotation sweep is incremental and
resumable). The reader MUST dispatch on the row's `key_version` column and pick
the matching *(key, AAD format)* pair:

| `key_version` | key | AAD |
|---|---|---|
| 1 | the seed (K1 output) used **directly** | A-v3 = `"{item_id}\|3"` |
| 2 | `derive_v2(seed)` (K2) | A-v4 = `"{item_id}\|4\|2"` |
| anything else | — | hard error `UnknownKeyVersion(v)` |

`crypto/encrypt.rs:182-204`. Never fall through to a "try v1 then v2" trial
decryption — the current code deliberately does not, and adding it would create
a decryption oracle and mask corruption.

**I-2 — `key_version` outside {1, 2} is a hard error, never a panic and never a
silent fallback.** `UnknownKeyVersion(v)` is surfaced to the IPC layer as
`auth_failed`. Storage-side, `validate_key_version` rejects anything other than
1 or 2 at write time (`core/src/storage/items/types.rs:31-38`), and a column
value that does not fit in `u8` becomes the typed `CorruptKeyVersion(v)` rather
than a cast panic (`items/query.rs`, `items/types.rs:17-22`).

**I-3 — The SQLCipher database key is the seed (K1), not K2.** Changing which
value opens the DB orphans every existing install. `PRAGMA key` must be applied
before any other statement.

**I-4 — Every frozen byte string in §3 is a hard fork if changed.** Specifically:
`HKDF_SALT_V1`, `HKDF_SALT_V2` (the 32 literal bytes), `PER_ACCOUNT_SALT_IKM`,
all `info` strings, the AAD schema-version integers 3/4/5, the chunk magic
`b"CHUNK_FORMAT_V1\0"`, the `CPPAIR1`/`CPPAIR2` magics, `PASSWORD_FILE_VERSION`,
and the relay `info`/PoP prefixes. The relay salt is `None` and **must stay
`None`** — switching to a real salt silently reassigns every account to a
different inbox (`core/src/relay.rs:44-53`, migration note `le8w`).

**I-5 — v0.2 empty-AAD ciphertexts stay unreadable.** Do not re-introduce the
`COPYPASTE_ALLOW_LEGACY_AAD` fallback (§3.3.1).

**I-6 — Chunked blobs written by v0.4.x must still decode.** That means keeping
`CHUNK_FORMAT_VERSION = 1`, the 41-byte AAD (§3.4.1), the 34-byte wire header
(§3.4.2), the `[count][len][record]…` container (§3.4.3), and the `blob_ref`
JSON `file_id` array-of-numbers encoding (§3.4.4). The per-chunk AAD does **not**
bind `key_version`, so the row's `key_version` column is the *only* record of
which key generation decrypts a blob — it must be read and honoured.

**I-7 — A `key_version = 2` stamp on a blob row does not guarantee v2
encryption.** Pre-fix `handle_image` / `handle_file` encrypted chunks with the
**v1** seed key while `ClipboardItem::new_image` / `new_file` unconditionally
stamped `ITEM_KEY_VERSION_CURRENT = 2`. The `WHERE key_version = 1` sweep never
saw those rows. A repair sweep probes v1-decrypt on kv=2 blob rows; success means
mislabeled → re-encrypt with v2 (the stamp stays 2); failure means correctly v2 →
skip (`core/src/storage/migration_v4/repair.rs:1-18`). **The rewrite must carry
this repair sweep, or those users' images become permanently unreadable.**

**I-8 — Cloud blobs are keyed only by passphrase + account id.** There is no
stored salt, no version byte, and no trial decryption. `account_id` must be the
canonical `"<project_ref>|<user_id>"` and must be byte-identical on both devices
or they will not agree on a key (`crypto/sync_key/mod.rs:341-369`).

**I-9 — Both CPPAIR1 and CPPAIR2 must decode.** `encode` emits CPPAIR2 only.
Unknown magic → `BadMagic`, never a permissive fallback.

**I-10 — Keychain service/account names and file-store filenames are frozen.**
Renaming `com.copypaste.daemon` / `device-secret-key` orphans the DB. The
v0.5.1→v0.5.2 file-store migration path (read legacy Keychain entry, adopt it
into the 0600 file, never overwrite an existing file) must be preserved
(`file_store.rs:116-160`).

### 2.2 Cryptographic invariants

**I-11 — Nonces are always freshly drawn from the OS CSPRNG (`OsRng`), never
`thread_rng`, never a counter, never reused.** Fixed under **CopyPaste-crh3.5**
in `chunks.rs:113-116`, `keys.rs:159-161`; already the case in `encrypt.rs:114`
and `sync_key/mod.rs:396`. Rationale for `OsRng` over the userspace-reseeded
`thread_rng`: a single consistent entropy source across the whole crypto surface.

**I-12 — Secret material is zeroize-on-drop at every boundary.** The set of
types/returns that MUST keep this property:

| value | wrapper | bug id |
|---|---|---|
| `DeviceKeypair` | `ZeroizeOnDrop` | — |
| `DeviceKeypair::ecdh` / `ecdh_zeroizing` return | `Zeroizing<[u8;32]>` | **CopyPaste-1qht** |
| `secret_key_bytes_zeroizing` return | `Zeroizing<[u8;32]>` | audit MED #3 |
| `derive_storage_key_v1` return | `Zeroizing<[u8;32]>` | **CopyPaste-ddp0** |
| `derive_v2` return | `Zeroizing<[u8;32]>` | item 5 |
| `derive_enc_key` return | `Zeroizing<[u8;32]>` | audit MED #3 |
| `SyncKey` | `Zeroize + ZeroizeOnDrop`, no `Debug`/`Display`/`Clone`/`Copy` | — |
| `PairingToken` | `Zeroize + ZeroizeOnDrop`, no `Debug`/`Display`/`Clone` | — |
| `SessionKey`, `PasswordFile` | `ZeroizeOnDrop` | — |
| Keychain-returned `Vec<u8>` | wrapped in `Zeroizing` at the call site | audit MED #4 |
| Argon2id output buffer | `Zeroizing<[u8;32]>` before moving into `SyncKey` | — |
| `PRAGMA key`/`rekey` SQL string | `Zeroizing<String>` | — |
| Pairing code (PAKE password) | `Zeroizing<String>`, never written to disk | ADR-008 |
| `decode_file_zeroizing` / `decode_image_zeroizing` / `decode_thumbnail_zeroizing` | `Zeroizing<Vec<u8>>` | **CopyPaste-dgqm** |

The tests that pin these are *type-level* (`let x: Zeroizing<[u8;32]> = f(…)`)
so a regression is a compile error, not a silent runtime weakening. Keep that
pattern.

**I-13 — Secret comparison is constant-time.** Never `==` on key bytes.

| comparison | mechanism | source |
|---|---|---|
| `PairingToken == PairingToken` | `subtle::ConstantTimeEq`, `PartialEq` impl never short-circuits | `pairing_qr/token.rs:71-78` |
| `SyncKey::ct_eq_bytes` (routine re-provision vs rotation) | `subtle::ConstantTimeEq` | `sync_key/mod.rs:245-248` |
| PAKE channel-confirmation tags | constant-time compare | `p2p/src/pake.rs` (`channel_confirmation_tag` consumers) |
| Relay PoP verification | constant-time compare server-side | `core/src/relay.rs:118-135` |

`subtle` is pinned `>= 2.5` for the constant-time fixes in that line (root
`Cargo.toml:44-47`).

**I-14 — No AEAD call may panic on user-supplied data.** Every failure is a typed
`Result` (audit medium #10). The panic sites that were specifically removed:
`slice::chunks(0)` → `InvalidChunkSize` (`chunks.rs:89-92`); `chunks.last().expect()`
→ `ok_or(Empty)` (`chunks.rs:155`); `as u32` truncation → `u32::try_from` +
`TooManyChunks` (`chunks.rs:100-102`, `:150`); `bytes.try_into().unwrap()` on a
Keychain blob → checked `InvalidLength(n)` (`device_key.rs:66-71`); oversized
AEAD input → `CipherFailed(String)` / `EncryptFailed{index}` instead of
`.expect()`.

**I-15 — Decryption failures are indistinguishable from tampering.** All AEAD
errors collapse to one variant (`AuthFailed` / `DecryptFailed`) with no detail
about *why*, to avoid an oracle (`encrypt.rs:150`, `sync_key/mod.rs:283-284`).
Structural framing errors (`BlobTooShort`, `MissingChunk`, `TruncatedStream`,
`UnknownKeyVersion`) are *deliberately* distinguishable — they carry no secret
information and callers need them to request a targeted re-send or to surface an
upgrade prompt.

**I-16 — Domain separation must be provable, not assumed.** Every pair of derived
keys that share IKM must differ by construction: v1 vs v2 differ by *both* hash
(SHA-256 vs SHA-512) *and* salt; storage vs sync vs telemetry differ by `info`;
local vs cloud differ by AAD schema version (3/4 vs 5); relay inbox id vs relay
public key differ by `info`; initiator vs responder confirmation tags differ by
`info`; SAS differs from both confirmation tags by `info`.

**I-17 — Argon2id parameters are a floor, not a suggestion.** `m = 19456 KiB`
(19 MiB, the OWASP interactive minimum), `t = 2`, `p = 1`, `Version::V0x13`,
32-byte output. `p = 1` is deliberate for cross-platform reproducibility
(single-threaded/WASM targets must derive the same key).

**I-18 — Passphrase floor is 12 characters, counted in `chars()` not bytes,**
checked **before** Argon2id runs so the error is fast and user-actionable
(`sync_key/mod.rs:321-323`). Raising the floor only validates *new* entry; it
does not invalidate already-derived stored keys.

**I-19 — Empty `account_id` is rejected loudly** (`EmptyAccountId`), never
allowed to collapse the per-account salt toward a shared constant — that would
reintroduce the cross-user Argon2id precompute weakness the design exists to
prevent (`sync_key/mod.rs:296-303`, `:364-366`).

**I-20 — Keychain read failures must be classified, not lumped together.** Only
`errSecItemNotFound` (**-25300**) authorises minting a fresh device key. Every
other OSStatus (locked **-25308**, auth-failed **-25293**, missing-entitlement
**-34018**, timeout, I/O) means the entry's status is *unknown* and must
propagate as `Locked(code)` so startup degrades with
`degraded_reason = "keychain_locked"` and **leaves the encrypted DB untouched**.
The old code matched `Err(_)` and minted an ephemeral key, producing
`SQLITE_NOTADB` and a dead daemon (`device_key.rs:116-135`, `:278-300`).

**I-21 — A mid-rotation crash must not orphan the DB.** If the primary Keychain
entry is absent, check `device-secret-key.rotate-backup` before generating a new
key; if a valid 32-byte backup exists, promote it to primary and clean up. A
non-not-found failure reading the *backup* also propagates as `Locked`
(`device_key.rs:136-201`).

**I-22 — Startup key read is time-bounded.** The blocking Security-framework call
runs on a dedicated OS thread with a hard timeout; on timeout the daemon reports
`Locked` and abandons the thread rather than hanging on an unanswered GUI prompt
(`keyload.rs:104-168`).

**I-23 — The dev/test bypass must short-circuit before any Security-framework
call**, and in production it must be read **once** and cached
(`OnceLock`) so an attacker who can mutate the running process's environment
mid-session cannot flip it into ephemeral-key mode after the real key is in use
(**CopyPaste-qvtg.5**, `keychain/mod.rs:55-59`).

**I-24 — Full fingerprints stay out of info-level logs.** Log only the first 23
chars at `info`; the full 64-char value goes to `debug` only. The fingerprint is
the mTLS pinning identity and info-level logs land in persistent stores
(**CopyPaste-qvtg.1**, `keyload.rs:190-202`, `device_key.rs:212-219`).

**I-25 — The relay inbox id is secret-derived and must never be logged**
(`core/src/relay.rs:11-17`). Treat it as a credential.

**I-26 — The v4 migration sweep's `WHERE key_version = 1` predicate is
load-bearing for crash-safety.** There is no per-row progress write; resumption
works *because* already-rotated rows are excluded by the predicate. Removing or
weakening it makes a restarted sweep re-process rotated rows, which then fail
AEAD authentication and get logged as undecryptable
(`core/src/storage/migration_v4/mod.rs:28-42`).

**I-27 — Ciphertext-migration AAD constants must be pinned locally with
compile-time equality guards.** `migration_v4` re-declares `AAD_SCHEMA_V3` /
`AAD_SCHEMA_V4` and `const _: () = assert!(…)`s them against the canonical
constants, so an unrelated bump cannot silently desync the migration
(`migration_v4/mod.rs:100-118`).

**I-28 — Key-version arguments must be type-distinguished at the API boundary.**
`decrypt_item_by_version` takes `V1Key<'_>` / `V2Key<'_>` newtypes; before
**CopyPaste-crh3.82** both were `&[u8; 32]`, so swapping them compiled cleanly
and produced a silent `AuthFailed` (`encrypt.rs:153-163`). Apply the same
treatment to any new pair of same-shaped key parameters.

## 3. Exact formats

Notation: `b"…"` is a Rust byte-string literal (ASCII, no NUL terminator unless
shown). `||` is concatenation. `BE` = big-endian. All `format!`-derived strings
are UTF-8 with **no** trailing newline and **no** length prefix unless stated.

### 3.1 AEAD primitive (all uses)

| Property | Value | Source |
|---|---|---|
| Cipher | XChaCha20-Poly1305 (`chacha20poly1305` 0.10.1, `XChaCha20Poly1305`) | ADR-001; `crypto/encrypt.rs:2-5` |
| Key size | 32 bytes | `crypto/encrypt.rs:112` |
| Nonce size | **24 bytes** (`NONCE_SIZE = 24`) | `crypto/encrypt.rs:9` |
| Tag size | **16 bytes** (`TAG_SIZE = 16`), appended to ciphertext by the crate | `crypto/encrypt.rs:10` |
| Nonce source | `rand::rngs::OsRng` (OS CSPRNG), fresh per message | `crypto/encrypt.rs:114`, `crypto/chunks.rs:116`, `crypto/sync_key/mod.rs:396` |
| Ciphertext layout returned | `ciphertext_body || tag[16]` (RustCrypto convention) | — |

There is **no** AES-GCM path and no algorithm negotiation anywhere. ADR-001
rationale: 192-bit nonce removes the random-nonce birthday bound, no AES-NI
dependency (ARM/Android), XChaCha's HChaCha subkey derivation gives partial
nonce-misuse tolerance.

### 3.2 HKDF derivations — verbatim salt and info strings

Every derivation in the product, in one table. **Bytes are frozen.**

| # | Purpose | Hash | Salt (verbatim) | IKM | `info` (verbatim) | Out | Source |
|---|---|---|---|---|---|---|---|
| K1 | v1 **local storage** key (a.k.a. the *seed*) | SHA-256 | `b"copypaste-v1-salt"` | X25519 static secret (32 B) | `b"copypaste-local-storage-v1"` | 32 B | `crypto/keys.rs:137-143`, `:255-262` |
| K2 | v2 **local storage** key | SHA-512 | `HKDF_SALT_V2` (32 raw bytes, see 3.2.1) | **K1's output** (the seed), 32 B | `b"copypaste-local-storage-v2"` | 32 B | `crypto/keys.rs:119-125` |
| K3 | v1 **network/pair** key (dead in prod, see §6) | SHA-256 | `b"copypaste-v1-salt"` | X25519 ECDH shared secret | `format!("copypaste-v1\|{}:{}\|{}:{}", sender.len(), sender, recipient.len(), recipient)` | 32 B | `crypto/keys.rs:233-245` |
| K4 | v2 per-pair storage/sync/telemetry (dead in prod, see §6) | SHA-512 | `SHA-256(b"copypaste-v2-salt" \|\| pair_id_utf8)` | caller-supplied | `format!("copypaste-hkdf-v2\|{pair_id}\|{purpose}")`, `purpose ∈ {"storage","sync","telemetry"}` | 32 B | `crypto/keys.rs:42-88` |
| K5 | Per-account **Argon2id salt** for the cloud sync key | SHA-256 | `None` (RFC-5869 zero block) | `PER_ACCOUNT_SALT_IKM` (32 raw bytes, see 3.2.1) | `b"copypaste/cloud-sync-key/per-account-salt\|"` `\|\| account_id_utf8` | 32 B | `crypto/sync_key/mod.rs:113`, `:131-148` |
| K6 | Relay **inbox id** | SHA-256 | `None` | sync key (32 B) | `b"copypaste/relay/inbox-id/v1"` | 16 B → UUID | `core/src/relay.rs:63`, `:80-94` |
| K7 | Relay **registration public key** | SHA-256 | `None` | sync key (32 B) | `b"copypaste/relay/public-key/v1"` | 32 B | `core/src/relay.rs:67`, `:106-112` |
| K8 | PAKE session key finalisation | SHA-256 | `b"copypaste-pake-session-v1"` | raw opaque-ke session key | `b"session-key"` | 32 B | `p2p/src/pake.rs:532-534` |
| K9 | PAKE → XChaCha envelope key | SHA-256 | caller-supplied salt (see K10) | `SessionKey` (32 B) | `b"copypaste-xchacha20-key-v1"` | 32 B | `p2p/src/pake.rs:121-126` |
| K10 | P2P content sync key (K9 with a fixed salt) | SHA-256 | `b"copypaste/p2p/content-sync-key/v1"` | `SessionKey` | `b"copypaste-xchacha20-key-v1"` | 32 B → base64 **STANDARD** | `daemon/src/ipc/pairing_ops_provisioning.rs:420-422` |
| K11 | TLS channel binding | SHA-256 | `tls_binder` (32 B from RFC-5705 `export_keying_material`, label `"EXPORTER-copypaste-channel-binding"`, context `None`, len 32) | `SessionKey` | `b"copypaste/p2p/channel-binding/v1"` | 32 B | `p2p/src/pake.rs:174-179` |
| K12 | Pairing confirmation tag — initiator | SHA-256 | `None` | channel-bound key (K11) | `b"copypaste/p2p/channel-confirm/v1/initiator"` | 32 B | `p2p/src/pake.rs:222-227` |
| K13 | Pairing confirmation tag — responder | SHA-256 | `None` | channel-bound key (K11) | `b"copypaste/p2p/channel-confirm/v1/responder"` | 32 B | `p2p/src/pake.rs:222-227` |
| K14 | 6-digit SAS | SHA-256 | `None` | channel-bound key (K11) | `b"copypaste/p2p/sas/v1"` | 4 B → `u32::from_be_bytes % 1_000_000`, `{:06}` | `p2p/src/pake.rs:264-271` |

Non-HKDF keyed derivation:

| # | Purpose | Construction | Source |
|---|---|---|---|
| M1 | Relay proof-of-possession | `HMAC-SHA256(key = sync_key, msg = b"relay-registration-pop-v1:" \|\| device_id_utf8)` → 32 B, base64 for wire | `core/src/relay.rs:59`, `:118-135` |
| A1 | Cloud sync key | `Argon2id(v=0x13, m=19456 KiB, t=2, p=1, out=32)` over `passphrase_utf8` with salt = K5 | `crypto/sync_key/mod.rs:315-339` |

#### 3.2.1 Frozen 32-byte constants (raw bytes, not strings)

`HKDF_SALT_V2` — `crypto/keys.rs:100-103`. Defined as
`SHA-256(b"copypaste/storage-key/v2/hkdf-salt")` and **pinned as a literal** so
it can never drift from what is written on disk:

```
dd 4e b4 9c 1e 2e 3c 66 11 a4 1b 03 3c ea 9a 50
5c 91 a3 09 09 a6 67 bb 3f 42 b3 d7 f3 33 02 8e
```

`PER_ACCOUNT_SALT_IKM` — `crypto/sync_key/mod.rs:102-106`. Defined as
`SHA-256(b"copypaste/cloud-sync-key/per-account-salt-ikm")`, pinned as a literal:

```
7c ac 08 f5 29 43 41 2f 83 07 a7 2f 21 19 2c 95
46 b3 fe 01 3a 69 25 b4 1d d0 12 32 33 7a fe 8a
```

Both have golden tests asserting they equal the SHA-256 of their canonical
preimage (`crypto/keys.rs:483-491`, `crypto/sync_key/tests.rs:20-27`). Keep those
tests: they make a change a deliberate, visible act.

#### 3.2.2 Why length-prefixing exists (K3) — bug **CopyPaste-lkmy**

The v1 network info string was originally `copypaste-v1|{sender}|{recipient}`.
An adversarial device id containing `|` collides across the boundary:
`sender="a|b", recipient="c"` and `sender="a", recipient="b|c"` both produce
`copypaste-v1|a|b|c` → **identical derived keys**. The fix length-prefixes each
id as `{len}:{id}`; `len` is a decimal integer that cannot contain `|`, so the
encoding is unambiguous. `crypto/keys.rs:225-239`; regression test
`derive_enc_key_v1_info_string_is_collision_resistant` (`crypto/keys.rs:616-631`).

**Rule for the rewrite:** any info string that concatenates two or more
caller-controlled strings MUST length-prefix each component. K4's info
(`copypaste-hkdf-v2|{pair_id}|{purpose}`) does **not** length-prefix `pair_id` —
it is only safe because `purpose` is a closed set of three literals and
`pair_id` is the last free field before a fixed suffix. Do not copy that pattern.

#### 3.2.3 The production local-storage key chain (critical, non-obvious)

This is the single most easily-broken thing in the port. The v1 *item* key is
**not** `derive_storage_key_v1(seed)` — it is the seed itself.

```
X25519 static secret (32 B, from Keychain / 0600 file)
  └─ K1: HKDF-SHA256(salt=b"copypaste-v1-salt", info=b"copypaste-local-storage-v1")
       = SEED  (32 B)
         ├── used DIRECTLY as the SQLCipher database key
         ├── used DIRECTLY as the v1 item AEAD key   (key_version = 1 rows)
         └─ K2: HKDF-SHA512(salt=HKDF_SALT_V2, ikm=SEED,
                            info=b"copypaste-local-storage-v2")
              = v2 item AEAD key                     (key_version = 2 rows)
```

Sources: `daemon/src/daemon/startup/keyload.rs:29-36` (`sweep_keys`), `:203`
(`KeyLoad::Ready(kp.local_enc_key(), …)`), and the read path in
`core/src/storage/items/query.rs:552-563`.

Recorded bug: an earlier build passed `derive_storage_key_v1(seed)` as the sweep's
v1 key — double-deriving it. That key never matched any real v1 row, so **every**
legacy `key_version = 1` row failed the auth tag and was never rotated
(`keyload.rs:24-28`, regression test `sweep_rotates_real_v1_row_and_it_stays_readable`
at `keyload.rs:398-433`).

SQLCipher key application: `PRAGMA key = "x'<64 lowercase hex chars>'"` applied
before any other statement (`core/src/storage/db/pragmas.rs:2,15`); rekey uses
`PRAGMA rekey = "x'<hex>'"` (`core/src/storage/db/mod.rs:309`). ADR-003.

### 3.3 AAD constructions — verbatim layouts

**Wrong AAD = silent `AuthFailed` on existing user data.** All AAD is UTF-8
bytes of a `format!` string with `|` separators, except the chunk AAD (binary).

| AAD | Applies to | Verbatim layout | Example bytes | Source |
|---|---|---|---|---|
| **A-v3** (local, v1 key) | `clipboard_items` rows with `key_version = 1` | `"{item_id}\|{schema_version}"` with `schema_version = 3` | `b"item-abc\|3"` | `crypto/encrypt.rs:74-76`, const `:31` |
| **A-v4** (local, v2 key) | `clipboard_items` rows with `key_version = 2` | `"{item_id}\|{schema_version}\|{key_version}"` with `schema_version = 4`, `key_version = 2` | `b"item-abc\|4\|2"` | `crypto/encrypt.rs:93-95`, const `:38` |
| **A-cloud** | relay / Supabase blobs | `"{item_id}\|{CLOUD_AAD_SCHEMA_VERSION}"` with `CLOUD_AAD_SCHEMA_VERSION = 5` | `b"item-abc\|5"` | `crypto/sync_key/mod.rs:172-174`, const `:165` |
| **A-chunk** | every chunked blob (image, file, thumbnail) | binary, 41 bytes — see 3.4 | — | `crypto/chunks.rs:50-59` |

`item_id` is the **cross-device logical identity** (`clipboard_items.item_id`), a
UUID string — *not* the per-row primary key `clipboard_items.id`. The two were a
bare `String` before **CopyPaste-crh3.80** split them into `ItemId` / `RowId`
newtypes precisely because passing the wrong one produced a ciphertext whose AAD
bound the wrong identifier — an undetectable bug
(`core/src/storage/items/ids.rs:1-22`). `ItemId`'s `Display` is the inner string
verbatim, and it is `#[serde(transparent)]`, so on-disk/on-wire bytes are
identical to a bare string.

Schema-version numbering is a **single ordered namespace across domains** so a
blob can never cross domains silently:

| value | meaning |
|---|---|
| 3 | local, v1-key ciphertext |
| 4 | local, v2-key ciphertext |
| 5 | cloud (sync-key) ciphertext |

The AAD `key_version` field must be bound to the *matched* version, not a
hardcoded literal — `decrypt_item_by_version` does
`build_item_aad_v2(item_id, 4, u32::from(v))` inside the `v @ 2` arm
(`crypto/encrypt.rs:195-201`).

> **Doc/impl delta to resolve in the rewrite:** `docs/security/THREAT-MODEL.md`
> lines 158, 214, 253 claim the AAD binds `(device_id, item_id, content_hash,
> schema_version)`. The implementation binds only `(item_id, schema_version)`
> and, for v4, `key_version`. Neither `device_id` nor `content_hash` is in any
> AAD. Either fix the doc or genuinely add the fields (which is a hard fork).

#### 3.3.1 Removed legacy behaviour — do NOT re-introduce

v0.2 encrypted with an **empty** AAD. v0.3 briefly shipped an env-gated fallback
`COPYPASTE_ALLOW_LEGACY_AAD`; it was **deleted**. Decryption now requires the
exact AAD supplied at encrypt time; a mismatch is always `AuthFailed`
(`crypto/encrypt.rs:12-14`, `:131-135`). The `AadMismatch` error variant was also
removed (**CopyPaste-crh3.91**, `crypto/encrypt.rs:44-47`) because it became
unconstructible and misled exhaustive-match callers.

Consequence: v0.2-era rows are permanently unreadable unless `copypaste migrate v3`
ran before the v0.3 upgrade. The rewrite inherits this — do not attempt to
resurrect empty-AAD decryption.

### 3.4 CHUNK_FORMAT_V1 — chunked AEAD (external, on-disk)

`CHUNK_FORMAT_VERSION: u8 = 1` (`crypto/chunks.rs:8`). Used by images, files, and
thumbnails; the resulting blob is stored in `clipboard_items.content`.

#### 3.4.1 Per-chunk AAD (41 bytes, binary)

```
offset  len  field
------  ---  --------------------------------------------------
  0     16   b"CHUNK_FORMAT_V1\0"      <- 15 ASCII chars + one 0x00 byte
 16     16   file_id                   <- caller-supplied [u8; 16]
 32      4   chunk_index   (u32 BE)
 36      4   total_chunks  (u32 BE)
 40      1   is_final      (0x01 if final, else 0x00)
------  ---
total   41
```

Source: `crypto/chunks.rs:50-59`. Note the magic is **16 bytes including the
trailing NUL** — `b"CHUNK_FORMAT_V1\0"`. Getting this wrong (15 bytes, or a
NUL-free 15-char string) makes every existing image and file undecryptable.

`total_chunks` is bound into **every** chunk's AAD, so truncating a stream changes
the AAD of all surviving chunks. `file_id` binding is what makes cross-stream
chunk injection fail.

#### 3.4.2 Per-chunk wire record (`EncryptedChunk::to_wire`)

```
offset  len        field
------  ---------  -------------------------------------------
  0        1       version = 0x01  (CHUNK_FORMAT_VERSION)
  1        4       chunk_index (u32 BE)
  5        1       is_final    (0x01 / 0x00)
  6       24       nonce       (random, XChaCha20 24-byte)
 30        4       ciphertext_len (u32 BE)
 34        N       ciphertext || poly1305 tag[16]
------  ---------
header = 34 bytes; record = 34 + N
```

Source: `crypto/chunks.rs:70-81`; parser at `core/src/image/blob.rs:80-105`.
`total_chunks` and `file_id` are **not** in the wire record — they are recovered
from the container (`chunk_count` in the blob header / `file_id` from
`blob_ref` JSON) and must match, or the AAD fails.

#### 3.4.3 Container blob (`chunks_to_blob` — what lands in SQLite)

```
offset  len   field
------  ----  -------------------------------------------
  0      4    chunk_count (u32 BE)
  4      -    repeat chunk_count times:
                 4     wire_len (u32 BE)
                 wire_len   the 34+N wire record from 3.4.2
```

Source: `core/src/image/blob.rs:19-45` (writer), `:48-109` (reader).
Minimum per-chunk footprint is `4 + 34 = 38` bytes; the reader clamps
`Vec::with_capacity` to `(blob.len() - 4) / 38` so a corrupt `chunk_count` cannot
trigger a multi-GB allocation (`blob.rs:54-64`). The reader rejects
`version != 1` with a decode error (`blob.rs:84-89`).

#### 3.4.4 `file_id` provenance (part of the on-disk contract)

`file_id` is a 16-byte value persisted in `clipboard_items.blob_ref` as a JSON
array of decimal numbers, produced by Rust's `{:?}` debug formatting of a
`[u8; 16]`:

```json
{"width":W,"height":H,"original_size":N,"chunk_count":C,
 "file_id":[12, 34, ...16 numbers...],
 "thumb_file_id":[...], "thumb_w":W2, "thumb_h":H2}
```

Sources: `daemon/src/clipboard/meta.rs:52`,
`core/src/storage/migration_v4/images.rs:125-127`, parser `:136-170` (rejects a
`file_id` array whose length ≠ 16 or whose elements exceed a `u8`).
Thumbnails use a **distinct** `thumb_file_id` so the thumbnail's chunk AAD is
isolated from the full image's (`core/src/image/crypt.rs`, `types.rs:135`).

#### 3.4.5 Chunk-stream structural validation (before any AEAD call)

`decrypt_chunks` validates the *sequence* before decrypting, so a gap surfaces as
a targeted error rather than an opaque `AuthFailed`
(**edge-case audit high #14**, `crypto/chunks.rs:163-187`):

1. empty slice → `Empty`
2. last chunk `is_final == false` → `TruncatedStream { expected: n+1, got: n }`
3. `chunk.chunk_index != position` → `MissingChunk { position, expected, got }`
4. `chunk.is_final && position != total-1` → `PrematureFinal { position, total }`

These four checks together force `is_final == (index == total-1)` for every
chunk, so the AAD reconstructed at decrypt is byte-identical to the one used at
encrypt even though `is_final` is read from attacker-controllable wire bytes.
**Keep all four, in this order, before the first AEAD call.**

### 3.5 Cloud blob format (`encrypt_for_cloud`)

```
offset  len   field
------  ----  ---------------------------------
  0     24    nonce (random, XChaCha20)
 24     N+16  ciphertext || poly1305 tag
```

Total length is exactly `24 + plaintext.len() + 16`
(`crypto/sync_key/mod.rs:409-413`; pinned by test
`blob_format_nonce_then_ciphertext_plus_tag`, `sync_key/tests.rs:178-184`).
`decrypt_from_cloud` rejects `blob.len() < 24` with `BlobTooShort(len)` rather
than panicking (`sync_key/mod.rs:431-433`). AAD is **A-cloud** (3.3).

### 3.6 Pairing QR envelope — `CPPAIR…`

Two versions are recognised. `decode` rejects any other magic with `BadMagic` —
this is the **anti-downgrade guard** (`pairing_qr/payload.rs:235-239`).

Base64 engine for every binary field: **`URL_SAFE_NO_PAD`** (URL-safe alphabet,
no `=` padding) — `pairing_qr/mod.rs:139-141`. Note K10's peer sync key uses
base64 **STANDARD**; do not conflate the two engines.

#### 3.6.1 CPPAIR2 (current, emitted by `encode`)

```
CPPAIR2.<fp_b64url>.<token_b64url>.<device_id_b64url>.<name_b64url>.<addr_b64url>[.<prov_b64url>]
```

| field | content | encoded length | notes |
|---|---|---|---|
| magic | literal `CPPAIR2` | 7 | `PAIRING_QR_MAGIC_V2`, `pairing_qr/mod.rs:113` |
| `fp_b64url` | 32 raw fingerprint bytes | **43 chars** | hex/colon-hex input → colons stripped → hex-decoded → b64url. Decode: b64url → must be exactly 32 bytes (`FingerprintLength` otherwise) → re-emitted as **lowercase bare hex** (64 chars, no colons) |
| `token_b64url` | 32 raw token bytes | **43 chars** | must decode to exactly 32 bytes or `TokenLength(n)` |
| `device_id_b64url` | 16 raw UUID bytes | **22 chars** | UUID string, hyphens stripped, hex-decoded. Decode: must be exactly 16 bytes (`DeviceIdLength`) → re-formatted as lowercase hyphenated UUID `8-4-4-4-12` |
| `name_b64url` | device name UTF-8 | var | must be valid UTF-8 after decode or `Utf8` |
| `addr_b64url` | `host:port` UTF-8, base64url | var | **v2-only**: base64url'd so IPv4 dots cannot collide with the `.` delimiter. Empty hint → b64url of empty string (empty field) |
| `prov_b64url` | optional 6th field, compact JSON, base64url | var | decode failure is **silently ignored** (`provisioning: None`) so a corrupt blob cannot break pairing |

Split rule on decode: `body.splitn(6, '.')`; `< 5` parts → `FieldCount(n)`
(`pairing_qr/payload.rs:295-298`).

Provisioning JSON is hand-built, compact, keys in this fixed order, only
non-empty fields emitted (`pairing_qr/payload.rs:65-84`):

```json
{"ru":"<relay_url>","su":"<supabase_url>","sk":"<supabase_anon_key>"}
```

String escaping covers only `"` and `\` (`pairing_qr/wire.rs:164-176`); the
parser is a hand-rolled key scanner that ignores unknown keys and non-string
values (`wire.rs:183-213`). All three values are non-secret (the Supabase
`anon` key is a publishable JWT by design).

#### 3.6.2 CPPAIR1 (legacy — decode-only, still accepted)

```
CPPAIR1.<fp_hex>.<token_b64url>.<device_id>.<name_b64url>.<host:port>
```

* `fp_hex` — lowercase hex, colons **preserved** (only `to_ascii_lowercase()` is
  applied, `wire.rs:107-109`). Empty → `EmptyFingerprint`.
* `device_id` — raw UUID string, passed through verbatim.
* `host:port` — **raw, un-encoded**, and it is the terminal field: decode uses
  `body.splitn(5, '.')` so IPv4 dots all land in `parts[4]`
  (`payload.rs:250-255`).
* **No provisioning field** in v1 — a 6th field would be ambiguous against IPv4
  dots. This is why v2 base64url-encodes `addr_hint`.

#### 3.6.3 Deep-link wrapper

```
cppair://pair?p=<percent-encoded CPPAIR2 payload>
```

`PAIRING_DEEPLINK_PREFIX = "cppair://pair?p="` (`pairing_qr/mod.rs:131`).
Percent-encoding is minimal RFC 3986: unreserved set is `A-Z a-z 0-9 - _ . ~`,
everything else becomes `%XX` with **uppercase** hex digits
(`wire.rs:34-47`). The decoder also maps `+` → space (`wire.rs:66-69`) and
tolerates malformed `%` sequences by emitting a literal `%`.
`strip_deeplink` accepts both wrapped and bare forms (`wire.rs:19-25`).

### 3.7 PAKE `PasswordFile` blob (on-disk, `peers.json`)

```
offset  len        field
------  ---------  ----------------------------------------
  0       1        version = 0x01  (PASSWORD_FILE_VERSION)
  1       2        setup_len (u16 BE)
  3     setup_len  opaque-ke ServerSetup bytes
  ...   rest       opaque-ke ServerRegistration bytes
```

Source: `p2p/src/pake.rs:49`, `:349-357` (writer), `:369-408` (reader; rejects
empty, wrong version, `len < 3`, and `body.len() < setup_len`).
Ciphersuite: `OprfCs = KeGroup = Ristretto255`, `KeyExchange = TripleDh`
(`pake.rs:63-67`). OPAQUE username is the fixed literal `b"copypaste-pair"`
(`pake.rs:57`). Currently persisted as standard-base64 `password_file_b64`
**in plaintext** inside `peers.json` (0600) — see ADR-008 "Security delta" and
**CopyPaste-5lm**.

### 3.8 Keychain / key-at-rest storage (verbatim identifiers)

macOS Keychain generic-password items, all under one service
(`daemon/src/keychain/mod.rs:27-37`, `acl.rs:278`):

| service | account | contents |
|---|---|---|
| `com.copypaste.daemon` | `device-secret-key` | 32-byte X25519 device secret |
| `com.copypaste.daemon` | `device-secret-key.rotate-backup` | transient copy staged during ACL rotation |
| `com.copypaste.daemon` | `cloud-sync-key` | 32-byte passphrase-derived sync key |
| `com.copypaste.daemon` | `supabase-password` | Supabase GoTrue account password (UTF-8) |

Write attributes (all secret writes go through one helper,
`keychain/secure_write.rs:39-80`, **CopyPaste-nkro**):

* `kSecAttrAccessControl` = `SecAccessControl` with
  `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` — the only protection class
  that suppresses iCloud Keychain sync **and** Time Machine inclusion.
* `kSecAttrSynchronizable` = `false` (defence-in-depth duplicate).
* On `errSecDuplicateItem`, fall back to `SecItemUpdate` with the same
  access-control attribute (upgrades legacy items in place).

File-store fallback (`daemon/src/keychain/file_store.rs`):

| file | contents |
|---|---|
| `<app_support_dir>/device_secret.key` | 32-byte X25519 device secret |
| `<app_support_dir>/cloud_sync.key` | 32-byte cloud sync key |

Written atomically: temp file `.<base_name>.tmp.<pid>.<nanos>` in the same
directory, opened with `mode(0o600)` **at create time** so the secret is never
momentarily group/other-readable, `chmod 0600` re-asserted, then `rename`
(`file_store.rs:261-310`).

Backend selection and env overrides:

| env var | effect |
|---|---|
| `COPYPASTE_KEY_BACKEND` = `file` \| `keychain` | forces the backend (`keychain/signing.rs:30,54-59`) |
| `COPYPASTE_EPHEMERAL_KEY` (any value) | dev/test bypass: every keychain entry point short-circuits **before** any Security-framework call (`keychain/mod.rs:55-68`) |
| `COPYPASTE_KEY_FILE_PATH` | test override for the device-key file path (`file_store.rs:75-82`) |

Default backend without an override: `Keychain` only when this process's code
signature carries a stable **Team Identifier** (Developer-ID); ad-hoc/unsigned or
undeterminable → `File` (`signing.rs:54-84`). Rationale: an ad-hoc signature's
designated requirement is its `cdhash`, which changes on every rebuild, so the
Keychain ACL breaks and macOS raises an interactive password prompt on every
update (`file_store.rs:1-41`).

## 4. Constants & tunables

### 4.1 Frozen — changing any of these is a hard fork of user data

| Constant | Value | Why this value | Source |
|---|---|---|---|
| `NONCE_SIZE` | `24` | XChaCha20's 192-bit nonce; the whole point of ADR-001 | `encrypt.rs:9` |
| `TAG_SIZE` | `16` | Poly1305 | `encrypt.rs:10` |
| `HKDF_SALT_V1` | `b"copypaste-v1-salt"` | versioned salt; a bump deterministically rotates every derived key | `keys.rs:12` |
| `HKDF_SALT_V2` | 32 raw bytes = `SHA-256(b"copypaste/storage-key/v2/hkdf-salt")` | fixed-length salt for the single-device local path (no `pair_id` exists there); pinned as a literal so it cannot drift from disk | `keys.rs:100-103` |
| `HKDF_SALT_V2_BASE` | `b"copypaste-v2-salt"` | *prefix* for per-pair salts — distinct from `HKDF_SALT_V2`; **dead in production** (§6) | `keys.rs:37` |
| `HKDF_VERSION` | `2` | snapshot guard; a bump to 3 must be a deliberate edit | `keys.rs:31` |
| `AAD_SCHEMA_VERSION` | `3` | local, v1-key ciphertexts | `encrypt.rs:31` |
| `AAD_SCHEMA_VERSION_V4` | `4` | local, v2-key ciphertexts; the schema bump and the key-version field were tied together so **one integer comparison** disambiguates legacy from post-migration at decrypt time | `encrypt.rs:38` |
| `CLOUD_AAD_SCHEMA_VERSION` | `5` | strictly greater than 3 and 4, so a cloud blob can never be read as a local one | `sync_key/mod.rs:165` |
| `ITEM_KEY_VERSION_CURRENT` | `2` | value stamped on freshly-inserted rows; pinned in the storage layer (not re-exported from crypto) because its meaning is "which key/AAD format at decrypt time" — a storage concern | `items/mod.rs:26` |
| `CHUNK_FORMAT_VERSION` | `1` | chunk wire version byte; reader rejects anything else | `chunks.rs:8` |
| chunk AAD magic | `b"CHUNK_FORMAT_V1\0"` (16 bytes) | domain-separates chunk AEAD from item AEAD | `chunks.rs:53` |
| `PAIRING_QR_MAGIC` | `"CPPAIR1"` | legacy, decode-only | `pairing_qr/mod.rs:105` |
| `PAIRING_QR_MAGIC_V2` | `"CPPAIR2"` | current; saves 35 chars vs v1 (fp 64→43, uuid 36→22) → lower QR density | `pairing_qr/mod.rs:113` |
| `PAIRING_DEEPLINK_PREFIX` | `"cppair://pair?p="` | makes external scanners offer "open in app" | `pairing_qr/mod.rs:131` |
| `PAIRING_TOKEN_LEN` | `32` | 256 bits from the OS CSPRNG — strictly stronger than the 6-char password it replaced | `pairing_qr/mod.rs:136` |
| `FP_BYTE_LEN` / `UUID_BYTE_LEN` | `32` / `16` | CPPAIR2 field-length validation | `pairing_qr/mod.rs:120,123` |
| `PAIRING_QR_FIELD_COUNT` | `5` | mandatory fields; a 6th optional field carries provisioning | `pairing_qr/mod.rs:117` |
| `PER_ACCOUNT_SALT_IKM` | 32 raw bytes (§3.2.1) | non-secret HKDF IKM for the per-account Argon2id salt | `sync_key/mod.rs:102-106` |
| `SYNC_SALT_INFO` | `b"copypaste/cloud-sync-key/per-account-salt\|"` | trailing `\|` separates the prefix from the appended account id | `sync_key/mod.rs:113` |
| `RELAY_HKDF_SALT` | `None` | **must stay `None`** — every existing registration derives with no salt | `relay.rs:53` |
| `RELAY_INBOX_INFO` | `b"copypaste/relay/inbox-id/v1"` | changing it re-points every account at a different inbox | `relay.rs:63` |
| `RELAY_PUBKEY_INFO` | `b"copypaste/relay/public-key/v1"` | domain-separated from the inbox id | `relay.rs:67` |
| `RELAY_POP_PREFIX` | `b"relay-registration-pop-v1:"` | changing it invalidates every existing PoP | `relay.rs:59` |
| `PASSWORD_FILE_VERSION` | `0x01` | lets both formats ship during a PAKE transition | `pake.rs:49` |
| `PAIRING_USERNAME` | `b"copypaste-pair"` | fixed OPAQUE "username" — pairing is 1:1, there is no user directory | `pake.rs:57` |
| `CONFIRM_TAG_LEN` | `32` | channel-confirmation tag | `pake.rs:~205` |
| `SAS_DIGITS` | `6` | ≈20 bits; with single-shot ephemeral keys + abort-on-mismatch this caps an online MitM at ~1-in-10⁶ per attempt (Bluetooth numeric-comparison / Magic-Wormhole pattern) | `pake.rs:~250` |
| Keychain `SERVICE` / accounts | see §3.8 | renaming orphans the DB | `keychain/mod.rs:27-37` |
| key filenames | `device_secret.key`, `cloud_sync.key` | file-store fallback | `file_store.rs:50,53` |

### 4.2 Security parameters — tunable upward, never downward

| Constant | Value | Rationale |
|---|---|---|
| `ARGON2_M_COST_KIB` | `19_456` (19 MiB) | OWASP interactive-login minimum. Higher raises brute-force cost at the price of memory on every device that derives the key — acceptable for a rare passphrase-entry flow. |
| `ARGON2_T_COST` | `2` | OWASP recommendation at the 19 MiB level; extra mixing without doubling memory. |
| `ARGON2_P_COST` | `1` | Deliberately 1 for **cross-platform reproducibility** — single-threaded / WASM / embedded targets must derive the identical key. Raising it would not meaningfully help. |
| Argon2 `Version` | `V0x13` | Argon2 v1.3. |
| `MIN_PASSPHRASE_LEN` | `12` chars (`chars().count()`, not bytes) | OWASP/NIST floor for human-chosen secrets; raises the dictionary-attack floor materially while staying enterable. Raising it only gates **new** entry — already-derived stored keys remain valid. |

### 4.3 Performance / operational tunables (safe to change)

| Constant | Value | Rationale |
|---|---|---|
| `FILE_CHUNK_SIZE` | `512 * 1024` | `file.rs:21` |
| `IMAGE_CHUNK_SIZE` | `512 * 1024` | `image/mod.rs:33` |
| `THUMBNAIL_MAX_DIM` | `192` | downscale-only, aspect preserved; `image/mod.rs:49` |
| `BATCH_SIZE` (v4 sweep) | `100` | **fetch page size only** — each row is rotated in its own implicit autocommit transaction. Keeps write transactions short enough not to block readers while amortising WAL fsync. `migration_v4/mod.rs:92` |
| `INTER_BATCH_SLEEP` | `50 ms` | applied **between fetch pages**, not between rows; yields the SQLite write lock to the daemon's hot path. `migration_v4/mod.rs:97` |
| `KEYCHAIN_READ_TIMEOUT` | see `daemon/src/daemon` | bounds the startup Keychain read so a GUI prompt cannot hang startup |
| max chunks per stream | `u32::MAX` | chunk indices are `u32` in the wire format; exceeding it is `TooManyChunks` |
| max AEAD message | `(2³² − 1) × 64` bytes (~256 GiB) | ChaCha20-Poly1305 per-message limit; exceeding it is `CipherFailed`, never a panic |

### 4.4 Error taxonomy to preserve

Callers depend on these distinctions; collapsing them loses recoverability.

| Error | Meaning | Caller action |
|---|---|---|
| `AuthFailed` / `DecryptFailed` | wrong key, wrong AAD, or tampering — **deliberately indistinguishable** | skip-and-count; never treat as success |
| `UnknownKeyVersion(u8)` | row written by a newer build | hard error → IPC `auth_failed`; prompt upgrade. Never fall through to v1/v2 logic |
| `CorruptKeyVersion(i64)` | column value does not fit in `u8` | schema corruption, distinct from a SQLite error |
| `CipherFailed(String)` / `EncryptFailed{index}` | AEAD rejected oversized input | chunk it or reject the request |
| `InvalidChunkSize` | `chunk_size == 0` | programmer error, caught before `slice::chunks(0)` panics |
| `TooManyChunks` | stream > `u32::MAX` chunks | reject |
| `Empty` | zero-chunk stream | reject |
| `TruncatedStream{expected,got}` | tail dropped | request re-send of the tail |
| `MissingChunk{position,expected,got}` | gap in the middle | request re-send of **that index** — this is why it is distinct from `AuthFailed` |
| `PrematureFinal{position,total}` | reorder or missing tail | re-request the stream |
| `BlobTooShort(usize)` | cloud blob < 24 bytes | reject, do not slice |
| `PassphraseTooShort(usize)` | < 12 chars | user-actionable validation error at the IPC layer |
| `EmptyAccountId` | no stable account identifier | refuse to derive a degenerate key |
| `KeychainError::Locked(i32)` | keychain locked/denied/timeout — **not** not-found | degrade; leave the encrypted DB untouched |
| `KeychainError::InvalidLength(usize)` | stored blob ≠ 32 bytes | do not `unwrap` a `try_into` |
| `PairingQrError::{BadMagic, FieldCount, Base64, Utf8, TokenLength, EmptyFingerprint, FingerprintLength, DeviceIdLength, AddrHintDecode}` | QR parse failures | reject the scan; `BadMagic` is the anti-downgrade guard |

## 5. Acceptance tests to re-create

Grouped by what they protect. Old-file references are given so the original can
be consulted; the rewrite should re-express them against its own API.

### 5.1 Frozen-constant golden tests (highest priority — they make a hard fork visible)

| Test | Assertion | Old location |
|---|---|---|
| `hkdf_salt_v2_is_sha256_of_canonical_input` | `HKDF_SALT_V2 == SHA-256(b"copypaste/storage-key/v2/hkdf-salt")` | `crypto/keys.rs:483-491` |
| `per_account_salt_ikm_is_sha256_of_canonical_input` | `PER_ACCOUNT_SALT_IKM == SHA-256(b"copypaste/cloud-sync-key/per-account-salt-ikm")` | `crypto/sync_key/tests.rs:20-27` |
| `hkdf_uses_versioned_salt` | `HKDF_SALT_V1 == b"copypaste-v1-salt"`, and swapping the salt changes the key | `crypto/keys.rs:363-401` |
| `salt_v1_and_v2_derive_different_keys_from_same_input` | independently re-derives with the documented salt bytes and asserts the production API matches byte-for-byte — catches salt drift from *outside* the crate | `tests/hkdf_versioning.rs:80-101` |
| `build_item_aad_v2_golden_bytes` | `build_item_aad_v2("item-abc", 4, 2) == b"item-abc\|4\|2"` and `build_item_aad("item-abc", 3) == b"item-abc\|3"` | `crypto/encrypt.rs:385-395`, `tests/key_version_tests.rs:302-332` |
| `build_cloud_aad_format` | `== b"item-abc\|5"` | `crypto/sync_key/tests.rs:211-214` |
| `cloud_aad_schema_version_is_5`, `aad_schema_version_v4_constant_is_4`, `hkdf_version_is_2`, `min_passphrase_len_is_twelve`, `argon2_params_are_expected_values` | pin the integers | `sync_key/tests.rs:205-223,296-298`, `encrypt.rs:373-375`, `keys.rs:470-472` |
| **NEW — required** | pin the chunk AAD prefix as exactly the 16 bytes `b"CHUNK_FORMAT_V1\0"` and the full 41-byte AAD for a fixed `(file_id, index, total, is_final)` | *no such test exists today — add it* |
| **NEW — required** | pin the 34-byte chunk wire header layout against a hand-written expected byte vector | *no such test exists today — add it* |

### 5.2 Decrypt-data-written-by-the-old-version (the critical class)

The current suite tests these by **re-encrypting with the old scheme inside the
test**, not from a checked-in binary fixture. The rewrite should do both: keep
the reproduce-then-decrypt tests **and** check in real byte fixtures captured
from a v0.4.1 build, because the rewrite may not contain the old encrypt path at
all.

| Test | What it proves | Old location |
|---|---|---|
| `sweep_rotates_real_v1_row_and_it_stays_readable` | a row written **exactly** as real legacy rows were (key = the seed used directly, AAD = `"{item_id}\|3"`, `key_version = 1`) is rotated to `key_version = 2` **and** still reads back as the original plaintext through the normal v2 read path. This is the test that caught the double-derivation bug. | `daemon/src/daemon/startup/keyload.rs:398-433` |
| `sweep_keys_match_read_path_keys` | `v1_key == seed` and `v2_key == derive_v2(seed)` — pins §3.2.3 | `keyload.rs:505-518` |
| `sweep_leaves_existing_v2_row_untouched_and_readable` | an already-v2 row's ciphertext is **byte-for-byte unchanged** by the sweep | `keyload.rs:438-501` |
| `t1_2_mixed_v1_and_v2_rows_decrypt_by_version` | a DB holding both generations decrypts both correctly | `tests/key_version_tests.rs:128-172` |
| `t1_3_v2_row_inserted_while_v1_rows_exist_decrypts_correctly` | mid-migration writes are safe | `tests/key_version_tests.rs:173-217` |
| `v3_and_v4_aad_are_incompatible` | a v3-AAD ciphertext cannot be read with a v4 AAD even when the shared fields match | `crypto/encrypt.rs:361-370`, `tests/key_version_tests.rs:333-356` |
| `decrypt_without_aad_fails` | v0.2 empty-AAD rows stay rejected (the accepted one-way break) | `tests/aead_tamper.rs:321+` |
| `from_bytes_decrypts_blob_encrypted_by_derive_sync_key` | a cloud blob survives the `spawn_blocking` key-snapshot round-trip | `crypto/sync_key/tests.rs:233-254` |
| **NEW — required** | checked-in v0.4.1 fixtures: (a) a `key_version = 1` text row (`item_id`, nonce, ciphertext, and the seed) that must decrypt; (b) a `key_version = 2` text row; (c) a real chunked image blob + its `blob_ref` JSON + key; (d) a cloud blob + passphrase + `account_id`; (e) a real `CPPAIR2` string and a real `CPPAIR1` string that must both decode | *does not exist — this is the single biggest gap* |

### 5.3 AEAD tamper detection (`tests/aead_tamper.rs`)

Re-create all of: unmodified round-trip succeeds; single-bit flip in the
ciphertext body, in the nonce, and in **every byte** of the 16-byte tag is
detected; truncated ciphertext and sub-tag-length ciphertext return `Err` without
panicking; wrong key returns `Err`; empty plaintext round-trips; two ciphertexts
under the same key cross-paired with each other's nonce **both fail** (nonce
binding); AAD swap between two `item_id`s fails; schema-version downgrade in the
AAD fails; matching AAD rebuilt independently still decrypts.

Plus `tampering_key_version_in_aad_fails_decryption` (`encrypt.rs:342-355`) —
changing only the `key_version` field of the AAD must be rejected.

### 5.4 Chunked-AEAD structural tests (`crypto/chunks.rs` tests)

`small_data_splits_into_chunks_and_reassembles`; `single_chunk_when_data_fits`;
`reordered_chunks_fail_decryption`; `truncated_stream_fails_decryption`;
`cross_stream_chunk_injection_fails` (chunk from `file_id_a` spliced into a
`file_id_b` stream must fail — this is the `file_id` AAD binding);
`gap_in_middle_fails_decryption` asserting the **specific**
`MissingChunk { position: 2, expected: 2, got: 3 }` values, not just "an error";
`zero_chunk_size_returns_invalid_chunk_size_error`;
`empty_chunks_returns_err_not_panic`; `chunk_wire_format_has_version_byte`;
empty-input produces exactly one empty final chunk (`file.rs` empty-file test).

### 5.5 Key-derivation properties

`ecdh_shared_secret_is_symmetric`; `derive_enc_key_is_deterministic`;
`derive_enc_key_differs_for_different_device_ids`;
`derive_enc_key_v1_info_string_is_collision_resistant` (the **CopyPaste-lkmy**
`("a|b","c")` vs `("a","b|c")` case); `local_enc_key_differs_from_network_key`;
`derive_v2_differs_from_v1`; `derive_v2_differs_from_derive_storage_key_v2`;
`hkdf_v2_purposes_are_domain_separated`;
`hkdf_v2_different_pair_ids_produce_different_keys`;
`derive_storage_key_v1_matches_local_enc_key` (free fn must not drift from the
instance method); `public_key_fingerprint_is_64_chars_hex`.

Zeroize contract tests are **type annotations** that fail to compile on
regression — `ecdh_returns_zeroizing_shared_secret`,
`derive_storage_key_v1_returns_zeroizing`, `derive_v1_and_v2_zeroize_return_parity`.

### 5.6 Sync-key / cloud

`same_account_same_passphrase_is_deterministic`;
`different_accounts_same_passphrase_derive_different_keys` (**and** that account
B's key fails to decrypt account A's blob); `empty_account_id_is_rejected`;
`wrong_passphrase_decrypt_fails`; `wrong_item_id_aad_fails`;
`tampered_ciphertext_fails`; `nonce_unique_across_two_encrypts`;
`blob_format_nonce_then_ciphertext_plus_tag` (length is exactly `24+N+16`);
`blob_too_short_returns_error_not_panic` (`BlobTooShort(10)`);
`short_passphrase_returns_passphrase_too_short` including the 11-char boundary
and `passphrase_at_min_length_succeeds` at exactly 12;
`rotated_key_cannot_decrypt_pre_rotation_blob` (the only real cloud revocation);
`ct_eq_bytes_matches_byte_equality`;
`per_account_salt_is_deterministic_and_unique` (and never collapses to the raw IKM).

### 5.7 Nonce uniqueness & at-rest

`nonces_are_unique_over_100k_encryptions` and `no_zero_nonces_in_1000_encryptions`
(`tests/nonce_uniqueness.rs`).

`tests/encryption_at_rest.rs`: the raw DB file bytes must not contain the
plaintext payload or plaintext image bytes (with a WAL checkpoint first, and
checking the `-wal`/`-shm` sidecars too); the file must **not** start with the
plain `SQLite format 3\0` header; opening with the wrong key returns an invalid-key
error; after rekey the old key fails and the new key works.

### 5.8 Pairing QR

`encode_decode_roundtrip`; `encoded_starts_with_magic` (`CPPAIR2.`);
`decode_rejects_bad_magic` (`CPPAIR0` → `BadMagic` — the anti-downgrade guard);
`decode_rejects_missing_magic`; `decode_rejects_wrong_field_count`;
`decode_rejects_short_token`; `cppair2_fingerprint_field_is_43_chars`;
`cppair2_device_id_field_is_22_chars`; `cppair2_decode_rejects_bad_fp_length`;
`cppair2_decode_rejects_bad_device_id_length`;
`cppair2_fingerprint_recovered_as_lowercase_hex` (no colons on output);
`cppair2_colon_hex_fingerprint_roundtrips_without_colons`;
`cppair1_legacy_decode_still_works` against a **hardcoded** CPPAIR1 string whose
`addr_hint` is `192.168.1.1:1234` (proves the IPv4-dot `splitn(5)` rule);
`payload_without_provisioning_has_5_fields`; `payload_with_provisioning_roundtrips`;
`malformed_provisioning_field_does_not_break_decode`;
`deeplink_with_provisioning_roundtrips`; `qr_provisioning_json_golden_schema`
(pins the `{"ru":…,"su":…,"sk":…}` key names);
`token_generate_is_correct_length_and_random`;
`token_eq_is_constant_time_and_correct`; `token_from_bytes_rejects_wrong_length`;
`pake_password_is_stable_for_same_token`.

### 5.9 Keychain classification (pure, hermetic — no Keychain syscall)

`classify_read_failure_only_not_found_creates` — `-25300` → create; `-25308`,
`-25293`, `-34018` → `Locked`. `is_missing_entitlement_matches_only_minus_34018`.
`decide_db_startup_ready_key_opens_normally`;
`decide_db_startup_locked_key_with_existing_db_degrades` (and that the reason
string is literally `"keychain_locked"` — the UI consumes it);
`decide_db_startup_locked_key_without_db_uses_ephemeral`;
`encrypted_db_exists_classifies_file_states` (missing and zero-length → no DB).

Keep the pattern of extracting a **pure classifier** so the decision logic is
testable without an interactive Keychain.

### 5.10 What an interactive Keychain now covers

§5.9's hermetic classifier is not enough on its own: it pins what the code does
with an OSStatus, never that the OSStatus is the one macOS returns. Until this
job existed nothing had watched `crypto/keystore/macos.rs` store or retrieve
anything, and the backend it replaced — a `0600` file — was what every shipped
binary actually used.

**All four passed on the first run** (CI 30632553103, `macos-14`, macOS
14.8.7), against a throwaway keychain created, unlocked and made default by the
job. So: `SecItemAdd`/`SecItemCopyMatching` work non-interactively on a
headless runner, `-25300` is the status the framework really returns for a
missing entry, and the Keychain — not the file store — is what answers a
macOS build.

`.github/workflows/ci.yml`'s `macos-check` job creates a throwaway keychain,
unlocks it, makes it the default, and runs
`cargo test -p copypaste-core -- --ignored`. **Every assertion names which
backend answered**, because a test satisfied by bytes coming back is satisfied
by the file store (CLAUDE.md rule 4).

| Rule | What is driven |
|---|---|
| I-10 | The minted secret is readable under the frozen `com.copypaste.daemon` / `device-secret-key` pair by an independent query, and the data directory is left empty — so the Keychain answered, not `device_secret.key` |
| I-20 | With no entry, `load` returns `Absent` — which is what checks `ERR_SEC_ITEM_NOT_FOUND` against what the framework really returns. Get it wrong and every fresh Mac refuses to start |
| I-20 | A wrong-length entry is `KeystoreEntryUnusable`, is not minted over, and is left byte-identical |
| F-11 | A history sitting beside no entry is refused rather than re-keyed, and nothing is written to the Keychain |

**Not covered, and two of these are gaps in the code rather than in the tests:**

- **§3.8's write attributes are not set.** `set_generic_password` sends class,
  service, account and data — no `kSecAttrAccessControl`, no
  `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, no explicit
  `kSecAttrSynchronizable = false`. The item therefore takes the default
  accessibility and is eligible for inclusion in a backup, which is the
  property §3.8 records as load-bearing.
- **I-22 has no equivalent.** `Keyring::load_or_create` runs inline in
  `main`, with no timeout and no dedicated thread. On a machine where the read
  raises a GUI prompt the daemon blocks at startup instead of degrading.
- **A prompting Keychain is untested by construction.** The runner is headless,
  so a read that would prompt returns `errSecInteractionNotAllowed` instead.
  That is the fail-closed direction, and it means these tests say nothing about
  what a user sees when the ACL no longer matches — which is the next item.
- **The ad-hoc-signature question.** §3.8 records that v1 chose the file store
  unless the binary carried a stable Team Identifier, because an ad-hoc
  signature's designated requirement is its `cdhash` and the ACL therefore
  breaks on every rebuild. v2 uses the Keychain unconditionally and ADR-0001
  signs ad-hoc. `scripts/release/smoke-macos-dmg.sh` re-signs the installed
  daemon and starts it again to find out what that combination does; the result
  is reported, not asserted, because the answer is a design decision rather
  than a regression.
- **I-21 (rotate-backup) and the rotation sweep** do not exist in v2 and are
  reference-only under CLAUDE.md rule 3.

## 6. Known-unjustified complexity we should NOT port

### 6.1 Dead key-derivation surface (drop outright — zero migration cost)

Verified by grepping the whole workspace: the following are **exported from
`crypto::mod` and `lib.rs` but called only from unit tests and benches** — there
is no production call site.

| Item | Only referenced by | Source |
|---|---|---|
| `derive_storage_key_v2(ikm, pair_id)` | its own tests; a doc-comment in `migration_v4/mod.rs:198` describing a design that was never built | `keys.rs:72-74` |
| `derive_sync_key_v2(ikm, pair_id)` | its own tests | `keys.rs:79-81` |
| `derive_telemetry_key_v2(ikm, pair_id)` | its own tests ("reserved for future use") | `keys.rs:86-88` |
| `derive_key_v2` (private core of the above) | the above | `keys.rs:60-68` |
| `hkdf_v2_pair_salt(pair_id)` | the above + tests | `keys.rs:42-51` |
| `HKDF_SALT_V2_BASE` | the above | `keys.rs:37` |
| `HKDF_VERSION` | a snapshot test asserting it equals 2 | `keys.rs:31` |
| `DeviceKeypair::derive_enc_key` (K3, the v1 network key) | `benches/crypto.rs`, `tests/core_integration.rs`, `tests/hkdf_versioning.rs` | `keys.rs:214-245` |
| `DeviceKeypair::ecdh_zeroizing` | alias that just delegates to `ecdh` (**CopyPaste-crh3.81** already collapsed the duplicated body) | `keys.rs:210-212` |

The "per-device-pair salt so each pair can rotate independently" story in the
`HKDF_VERSION` doc-comment (`keys.rs:14-31`) describes an architecture that never
shipped: the actual v2 local key is the pair-less `derive_v2`
(`copypaste-local-storage-v2`), and cross-device sync uses the Argon2id
`SyncKey`, not a pair-derived key. **Do not port the pair-salt family.** Keep
only K1 (`derive_storage_key_v1` / `local_enc_key`) for reading legacy rows and
K2 (`derive_v2`) for writing.

Similarly, `DeviceKeypair::ecdh` / `derive_enc_key` implement an X25519 key
agreement that the product does not use — pairing goes through OPAQUE, and cloud
sync through Argon2id. If the rewrite keeps `DeviceKeypair` at all, it needs
only: generate, `from_secret_bytes`, `public_key_bytes`,
`secret_key_bytes_zeroizing`, `fingerprint`, and `local_enc_key`.

**Migration implication: none.** No on-disk or on-wire bytes are produced by any
of these. Deleting them cannot break a single existing install.

### 6.2 Custom chunked framing duplicates `aead::stream` (STREAM)

**Claim verified.** `crypto/chunks.rs` hand-rolls the STREAM construction:
per-chunk AEAD, a sequence counter, and a last-block flag, all bound as
associated data. `chacha20poly1305` 0.10.1 is already in the dependency tree and
ships `aead::stream` behind the `stream` feature (currently not enabled).
`StreamBE32<XChaCha20Poly1305>` gives:

* a 19-byte nonce **prefix** (24 − 4 counter − 1 last-block byte), generated once
  per stream instead of a 24-byte random nonce per chunk;
* a 32-bit BE position counter and a last-block flag folded into the nonce, which
  is exactly what `chunk_index` / `is_final` are doing in the AAD today;
* ordering, truncation, reordering, and duplication resistance **by construction**,
  removing the need for the four hand-written structural checks in
  `decrypt_chunks` (`chunks.rs:163-187`);
* per-chunk AAD still available via `Payload`, so `file_id` binding survives.

Two properties the current format has that STREAM does **not** give for free, and
which must be re-established deliberately if the format changes:

1. **`total_chunks` binding.** Today every chunk's AAD carries `total_chunks`, so
   any change to the stream length invalidates all surviving chunks. STREAM binds
   only position + last-block. The last-block flag is the property that actually
   matters (truncation resistance); `total_chunks` is redundant with it. If you
   want belt-and-braces, put the count in the *container header* and pass it as
   first-chunk AAD.
2. **`file_id` binding.** Must be passed explicitly as per-chunk AAD; it is not
   part of STREAM.

**Migration implication — this is the expensive one.** The chunk format is a
**persisted on-disk format**, not a transient wire format. Every image, file, and
thumbnail in every existing user's `clipboard_items.content` is a
`CHUNK_FORMAT_V1` container. Switching to STREAM changes:

* the AAD (no more 41-byte `CHUNK_FORMAT_V1\0` prefix),
* the nonce derivation (deterministic prefix+counter instead of 24 random bytes
  per chunk),
* the per-chunk record layout (no per-chunk nonce field).

Recommended path:

1. Keep a **read-only** `CHUNK_FORMAT_V1` decoder in the rewrite, permanently.
   It is ~60 lines (`chunks.rs:50-59` + `image/blob.rs:48-109`) and it is the
   only thing standing between existing users and unreadable images.
2. Introduce `CHUNK_FORMAT_V2` = STREAM, distinguished by the **existing version
   byte** at offset 0 of every wire record — the format was designed for exactly
   this and the reader already rejects unknown versions (`blob.rs:84-89`).
3. Re-encode lazily on read, or via a background sweep modelled on
   `migration_v4` (page by `rowid`, one row per autocommit transaction,
   `INTER_BATCH_SLEEP` between pages, resumable via a predicate rather than a
   progress cursor — see I-26).
4. Do **not** bump `key_version` for this; it tracks the HKDF generation, not the
   framing. Add a separate discriminator or rely on the wire version byte.

Net assessment: worth doing, but it is a data migration, not a refactor. If the
rewrite is time-boxed, porting `CHUNK_FORMAT_V1` verbatim is the safe choice and
the STREAM switch can be a later, independently-shippable change.

### 6.3 OPAQUE / `opaque-ke` is heavier than the pairing problem requires

**Claim verified, with one correction to the framing.** ADR-008 chose
`opaque-ke` 3.0.0 for its *augmented* property: "server DB exfiltration does not
reveal the pairing code". Three facts undercut that in the shipped system:

1. **The augmented property is already forfeited.** ADR-008's own "Security delta
   vs original design" section records that the `PasswordFile` is stored as
   `password_file_b64` — **plaintext base64 in `peers.json`** (0600), not in the
   SQLCipher DB as designed. Tracked as **CopyPaste-5lm**, unresolved. The whole
   reason for preferring aPAKE over a balanced PAKE is therefore not realised in
   production.
2. **The pairing secret is no longer low-entropy.** The threat aPAKE defends
   against is an offline dictionary attack on a human-typed code. The QR flow
   replaced the 6-character password with a **256-bit `PairingToken` from the OS
   CSPRNG**, base64url-encoded into the PAKE password slot
   (`pairing_qr/token.rs:61-63`). A 256-bit password is not dictionary-attackable;
   the PAKE's defining property is doing no work.
3. **The deployment is symmetric.** Both endpoints are peer daemons owned by the
   same user; there is no asymmetric client/server trust relationship for the
   augmented model to exploit. `PAIRING_USERNAME` is a fixed literal
   (`b"copypaste-pair"`) because there is no user directory
   (`pake.rs:57`).

`spake2` (balanced) would cover the remaining requirement — mutual proof of the
same secret without transmitting it — at a fraction of the cost. What OPAQUE
costs today: `voprf` + `curve25519-dalek` + `argon2` + `generic-array`, ~150 KB
of binary, ~30 s of clean release build, ~50 ms of Argon2 per handshake, and an
API heavy enough in generics that it needed newtype wrappers to be usable
(ADR-008 "Cons").

There is also a **third, genuinely non-substitutable** path in the system that
should not be confused with this: the LAN/SAS discovery flow has **no**
pre-shared secret and authenticates entirely through the human comparing a
6-digit SAS derived from the TLS-channel-bound key (`pake.rs`, `derive_sas`).
That path's security comes from K11 channel binding + K14 SAS + abort-on-mismatch,
not from OPAQUE's augmented property. Whatever PAKE the rewrite picks must keep
K11/K12/K13/K14 intact.

**Migration implications of swapping OPAQUE → spake2:**

* `PasswordFile` (§3.7) becomes obsolete. Its version byte `0x01` was added
  precisely so both formats can coexist (`pake.rs:49`, ADR-008 "Migration Path").
  Emit `0x02` for the new scheme and keep the `0x01` reader only if you intend to
  preserve existing pairings.
* **Existing pairings do not survive a PAKE swap.** The `PasswordFile` encodes
  OPAQUE-specific `ServerSetup` + `ServerRegistration`; there is no conversion.
  Every paired device pair must re-pair (scan a fresh QR). This is a **user-visible
  one-time action**, not silent data loss — but it must be planned, messaged, and
  it must not orphan anything else. Critically: the **peer content sync key**
  (K10) is derived from the PAKE `SessionKey` and stored in `peers.json`, so
  re-pairing rotates it; any relay/cloud items encrypted under the old peer key
  become unreadable to the re-paired device unless the sync key is re-provisioned
  (which the existing `apply_peer_provisioning` path already does).
* The QR envelope (§3.6) is **unaffected** — it transports a fingerprint and a
  raw token, with no PAKE-specific fields. `to_pake_password()` (base64url of the
  32 token bytes) feeds whatever PAKE is chosen.
* K11/K12/K13/K14 are **unaffected** — they take a 32-byte `SessionKey` as IKM and
  do not care how it was produced. Keep their `info` strings verbatim.
* ADR-008 explicitly anticipated this: the public API
  (`PakeInitiator` / `PakeResponder` / `SessionKey`) is crate-agnostic and hides
  `opaque_ke::*` behind newtypes, so the swap is scoped to one module.

**Prerequisite either way:** fix **CopyPaste-5lm** (plaintext `PasswordFile` in
`peers.json`). If the rewrite keeps OPAQUE, that file must move into SQLCipher or
the ADR's rationale is fiction. If the rewrite moves to spake2, the stored
material shrinks and the question partly evaporates — but whatever is stored must
still not be plaintext-at-rest.

### 6.4 Smaller things not worth carrying

| Thing | Why drop | Source |
|---|---|---|
| Hand-rolled JSON builder + key-scanner for the QR provisioning field | ~50 lines re-implementing a subset of `serde_json`, escaping only `"` and `\`, with a parser that silently returns `None` on any structural issue. `serde_json` is already a workspace dependency. Keep the **key names** `ru`/`su`/`sk` and the compact shape (they are on-wire); drop the hand-rolled codec. | `pairing_qr/wire.rs:164-213`, `payload.rs:65-112` |
| Hand-rolled percent-encode/decode | ~60 lines; the decoder additionally maps `+` → space (a form-encoding rule, not RFC 3986) and swallows malformed `%` sequences. Behaviour is load-bearing for existing deep links, so if replaced, pin it with tests. | `pairing_qr/wire.rs:34-95` |
| Hand-rolled UUID formatting (`uuid_bytes_to_str`) and hex/UUID↔b64url helpers whose error path encodes the raw UTF-8 bytes so the *decoder* can reject them | "encode garbage, let decode reject it" is an awkward contract; parse-and-fail at encode time instead. Output bytes are unchanged for valid input. | `pairing_qr/wire.rs:117-154` |
| `AAD_SCHEMA_VERSION` duplicated as a local constant in `encrypt.rs` rather than sourced from `storage::schema` | The comment itself flags this as a merge-race workaround to reconcile later. Make it one source of truth. | `encrypt.rs:16-24` |
| `migration_v4`'s locally re-declared `AAD_SCHEMA_V3`/`V4` + `const _: () = assert!(…)` guards | A symptom of the above duplication. Once there is one source of truth the guards are unnecessary — but **do not remove them before** unifying the constant. | `migration_v4/mod.rs:100-118` |
| Three separate v4 sweep clusters (`text`, `images`, `repair`) | Justified today only because the writer bug (I-7) created a third population. Once the repair sweep has run everywhere, `repair` can retire — but it must ship in the rewrite first. | `migration_v4/{text,images,repair}.rs` |

### 6.5 Open discrepancies to resolve during the rewrite

1. **THREAT-MODEL vs implementation (§3.3).** The doc claims the AAD binds
   `device_id` and `content_hash`; it does not. Decide which is true and make
   them agree. Adding the fields is a hard fork.
2. **ADR-008 storage claim vs reality.** `paired_peers.pake_password_file BLOB`
   does not exist; the `PairedDevice` struct has no such field and extra JSON is
   silently dropped on deserialisation. (**CopyPaste-5lm**.)
3. **`HKDF_VERSION` doc-comment vs reality (§6.1).** It documents a per-pair
   rotation architecture that was never wired up.
4. **No checked-in old-version byte fixtures** anywhere in the suite (§5.2). Every
   "can we still read old data" test reproduces the old format in-process, which
   will be impossible once the rewrite drops the old encrypt paths. Capture the
   fixtures from a v0.4.1 build **before** starting the rewrite.
