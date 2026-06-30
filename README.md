# CopyPaste

End-to-end encrypted clipboard sync for macOS and Android, with full
feature parity between the two platforms.

## Sync

CopyPaste ships three independent sync transports. They can be used alone or
together; each encrypts items end-to-end before they leave the device
(XChaCha20-Poly1305), and the relay/cloud paths only ever handle opaque
ciphertext.

- **P2P (LAN, direct):** mutual-TLS transport with mDNS-SD service discovery.
  Devices pair over a PAKE bootstrap channel and confirm a 6-digit short
  authentication string (SAS) — both users compare the same digits before the
  pairing is accepted. Works macOS↔macOS and macOS↔Android.
- **Relay-as-database:** an optional self-hostable relay server
  (`copypaste-relay`). Each device co-registers a shared inbox id derived from
  the sync key (`derive_relay_inbox_id`, HKDF over the 32-byte sync key) and
  POSTs encrypted blobs for the others to poll. The relay never holds keys or
  sees plaintext, and persists device records, tokens, and inbox items to a
  local SQLite database so state survives a restart.
- **Cloud (Supabase):** opt-in cloud sync. Enabled with the `cloud-sync`
  feature on the daemon and valid `SUPABASE_URL` / `SUPABASE_ANON_KEY`.

QR pairing fully provisions **all** sync paths on the scanning device with zero
manual entry: over the already-authenticated bootstrap tunnel the host sends the
Supabase connection params and `relay_url` (non-secret) plus the derived cloud
sync key (never embedded in the QR image), so the new phone is configured for
P2P, relay, and Supabase in one scan.

### Upgrading to v0.6 — one-time re-pair required

Upgrading from an earlier release requires a **one-time re-pair of all
devices**. The P2P bootstrap protocol (`BOOTSTRAP_PROTO_VERSION` 2) and the
Android UniFFI ABI (bumped to 13 at v0.6; current ABI is 18 — see the
[UDL / Rust API contract](#udl--rust-api-contract) section below) were bumped,
so old pairings will not connect until re-paired. Re-scan the pairing QR, or
re-run LAN discovery + SAS confirmation, on each device.

## Supported platforms

- **macOS** — arm64 (Apple Silicon) native; Intel (x86_64) is supported via
  Rosetta 2 (release builds are arm64-only — no universal DMG is published);
  install via Homebrew Cask (see `docs/release/brew-tap-setup.md`).
  Requires macOS 14 (Sonoma) or later.
- **Android** — arm64-v8a; UniFFI bindings ship as an `.aar`.

Windows is **frozen** as of 2026-05-23 — see
[`docs/adr/ADR-012-windows-frozen-homebrew-only.md`](docs/adr/ADR-012-windows-frozen-homebrew-only.md).
Windows users can run the daemon under WSL2 or wait for the freeze to be
lifted; no ETA.

## Architecture

```
crates/
  copypaste-core/       — pure-Rust library (encryption, detection, database, config)
  copypaste-ipc/        — IPC request/response types shared by daemon, CLI, and UI
  copypaste-daemon/     — long-running background process (clipboard, IPC server, sync)
  copypaste-cli/        — user-facing CLI (speaks IPC only, no core dep)
  copypaste-ui/         — Tauri v2 + React desktop UI (speaks IPC only, no core dep)
  copypaste-relay/      — Axum HTTP relay server (standalone, no core dep)
  copypaste-p2p/        — mTLS P2P transport + mDNS-SD discovery
  copypaste-sync/       — sync engine (CRDT, Lamport timestamps, protocol types)
  copypaste-supabase/   — Supabase cloud sync (opt-in via cloud-sync feature)
  copypaste-android/    — UniFFI FFI crate (cdylib + bindgen binary)
  copypaste-telemetry/  — telemetry, opt-out, PII scrubbing
  copypaste-bench/      — Criterion benchmarks
android/                — Android Studio project
```

## Android UniFFI Bindings

The Android app uses [UniFFI](https://github.com/mozilla/uniffi-rs) to call Rust code from Kotlin.

### Regenerating Kotlin bindings

Run after any change to the UDL (`crates/copypaste-android/uniffi/copypaste_android.udl`) or the public Rust API (`crates/copypaste-android/src/lib.rs`):

```bash
./scripts/generate-android-bindings.sh
```

This command:
1. Builds `copypaste-android` (cdylib) and the `uniffi-bindgen` binary in debug mode.
2. Runs `uniffi-bindgen generate <udl-path> --language kotlin` from within the crate directory.
3. Writes generated Kotlin to `android/app/src/main/java/com/copypaste/generated/uniffi/copypaste_android/`.

### Manual invocation (equivalent to the script)

```bash
# Build library and bindgen tool
cargo build -p copypaste-android
cargo build -p copypaste-android --bin uniffi-bindgen

# Generate Kotlin — must be run from the crate directory
cd crates/copypaste-android
../../target/debug/uniffi-bindgen generate uniffi/copypaste_android.udl \
    --language kotlin \
    --out-dir ../../android/app/src/main/java/com/copypaste/generated/
cd ../..
```

### UDL / Rust API contract

The full UDL surface is defined in
[`crates/copypaste-android/uniffi/copypaste_android.udl`](crates/copypaste-android/uniffi/copypaste_android.udl).
As of ABI 18 the namespace exposes **55 functions** across the following categories:

**Crypto / sensitive detection (5 functions)**

| UDL function | Description |
|---|---|
| `encrypt_text(item_id, bytes, key, key_version)` | XChaCha20-Poly1305 encrypt — binds `item_id` + `key_version` in AAD |
| `decrypt_text(item_id, ciphertext, nonce, key, key_version)` | Decrypt a single item |
| `decrypt_text_batch(items, key)` | Batch decrypt; skips (does not throw on) individual AEAD failures |
| `is_sensitive(text)` | Returns `true` when confidence ≥ 0.70 |
| `sensitive_kind(text)` | Returns pattern label (e.g. `"AwsKey"`) or `null` when not sensitive |

**Sensitive capture helpers (3 functions)**

| UDL function | Description |
|---|---|
| `sensitive_capture_decision(text, now_unix_ms, sensitive_ttl_secs)` | One-call verdict: `{is_sensitive, kind, expires_at_ms}` |
| `sensitive_expires_at_ms(now_unix_ms, sensitive_ttl_secs)` | Compute auto-wipe expiry; `null` when TTL is 0 |
| `detect_sensitive_spans(text)` | All sensitive match spans for in-list masking |

**Storage (5 functions)**

| UDL function | Description |
|---|---|
| `open_database(path, key)` → `u64` | Open the SQLCipher DB; returns an opaque handle |
| `close_database(handle)` | Release the handle |
| `add_clipboard_item(db_path, key, text)` → `String` | Store a text item (uses default TTL) |
| `store_clipboard_item(db_path, key, text, sensitive_ttl_secs)` → `String` | Store with explicit TTL |
| `get_history_count(db_path, key)` → `u64` | Count stored items |

**History / search (2 functions)**

| UDL function | Description |
|---|---|
| `fts_search(db_path, key, query, limit)` → `sequence<SearchResultItem>` | FTS5-indexed search (O(log N), metadata only) |
| `get_history_page(db_path, key, limit, offset)` → `sequence<HistoryItem>` | Lamport-ordered page (pinned first) |

**Cloud sync crypto (5 functions)**

| UDL function | Description |
|---|---|
| `derive_cloud_sync_key(passphrase, account_id)` → `sequence<u8>` | Argon2id KDF (per-account salt) → 32-byte sync key |
| `cloud_encrypt(item_id, plaintext, sync_key_bytes)` | AEAD encrypt for cross-device sync |
| `cloud_decrypt(item_id, blob, sync_key_bytes)` | AEAD decrypt |
| `relay_inbox_id(sync_key)` → `String` | Deterministic shared relay inbox UUID |
| `relay_public_key_b64(sync_key)` → `String` | Relay registration public key (base64) |
| `relay_registration_pop(sync_key, device_id)` | HMAC-SHA256 proof-of-possession for relay registration |

**Text classification (1 function)**

| UDL function | Description |
|---|---|
| `classify_text_kind(text)` → `String` | Returns `"TEXT"`, `"URL"`, `"EMAIL"`, `"PHONE"`, `"COLOR"`, `"JSON"`, `"CODE"`, `"NUMBER"`, or `"PATH"` |

**Private mode (2 functions)**

| UDL function | Description |
|---|---|
| `set_private_mode(enabled)` | Seed/update the Rust-side private-mode flag |
| `get_private_mode()` → `bool` | Read the current Rust-side private-mode flag |

**Key rotation / revocation (2 functions)**

| UDL function | Description |
|---|---|
| `revoke_device_and_rotate_key(db_path, key, fingerprint, name, new_passphrase)` | Revoke peer + rotate sync key atomically; returns new 32-byte key |
| `rotate_sync_key(new_passphrase)` | Rotate sync key without revoking a peer |

**QR pairing (2 functions)**

| UDL function | Description |
|---|---|
| `build_pairing_qr(fingerprint, device_id, device_name, addr_hint)` → `PairingQrPayload` | Build QR payload (display side) |
| `parse_pairing_qr(payload)` → `ScannedPairing` | Parse scanned QR (scan side) |

**STUN / network (1 function)**

| UDL function | Description |
|---|---|
| `resolve_stun_public_ip()` → `String?` | Discover public WAN IPv4 via STUN (blocking) |

**Version / ABI (3 functions)**

| UDL function | Description |
|---|---|
| `core_version()` → `String` | Rust crate version string |
| `uniffi_abi_version()` → `u32` | Current ABI integer (bump on any UDL/contract break) |
| `check_compatibility(kotlin_abi_version)` | Throws `VersionError::Incompatible` on mismatch |

**P2P cert + pairing (5 functions)**

| UDL function | Description |
|---|---|
| `generate_device_cert()` → `DeviceCert` | Generate self-signed mTLS cert + device UUID |
| `bootstrap_pair_initiator(addr_hint, cert_der, key_der, pake_password, sync_addr, local_provisioning, …)` → `BootstrapResult` | PAKE pairing, initiator side |
| `sync_with_peer(peer_addr, peer_fingerprint, session_key, cert_der, key_der, local_items, revoked_fingerprints, device_id)` → `P2pSyncResult` | One P2P sync session |
| `start_p2p_listener(listen_port, cert_der, key_der, allowed_fingerprints, …)` → `P2pListenerHandle` | Bind inbound mTLS listener |
| `poll_p2p_listener(listener_id)` → `sequence<SyncedItem>` | Drain inbound items since last poll |
| `update_p2p_listener_peers(listener_id, allowed, revoked, session_keys)` | Live roster/denylist refresh |
| `stop_p2p_listener(listener_id)` | Cancel and deregister listener |

**LAN discovery + SAS pairing (7 functions)**

| UDL function | Description |
|---|---|
| `start_discovery(device_id, device_name, sync_port, bport, cert_der, key_der, …)` | Start mDNS-SD discovery + standing SAS responder |
| `stop_discovery()` | Stop discovery + responder |
| `list_discovered(paired_fingerprints)` → `sequence<DiscoveredPeer>` | Snapshot discovered LAN peers |
| `pair_with_discovered(device_id, cert_der, key_der, sync_addr, local_provisioning, …)` | Begin initiator SAS pairing with discovered peer |
| `pair_get_sas()` → `PairStatus` | Poll pairing state machine |
| `pair_confirm_sas(accept)` | Deliver accept/reject SAS decision |
| `pair_abort()` | Cancel in-flight pairing |
| `pair_reset()` | Reset state machine to `idle` |

**Config (2 functions)**

| UDL function | Description |
|---|---|
| `default_config()` → `Config` | Canonical defaults (pure, no I/O) |
| `clamp_config(cfg)` → `Config` | Enforce floors/ceilings (pure, no I/O) |

**Device revocation audit (3 functions)**

| UDL function | Description |
|---|---|
| `revoke_device_audit(db_path, key, fingerprint, name)` → `u64` | Record revocation and remove from devices table |
| `list_revoked_fingerprints(db_path, key)` → `sequence<String>` | List revoked device fingerprints |
| `list_revoked_peers(db_path, key)` → `sequence<RevokedPeer>` | Richer audit listing with timestamps |

Error types: `CopypasteError` (`EncryptionFailed`, `DecryptionFailed(reason)`, `DatabaseError(reason)`, `InvalidKeyLength`, `P2pError(reason)`, `Panicked(reason)`) and `VersionError` (`Incompatible(rust_abi, kotlin_abi)`).

## Building

```bash
cargo build            # all Rust crates
cargo test             # run tests
```

## Relay server

```bash
cargo run -p copypaste-relay
```

## CLI

```bash
cargo run -p copypaste-cli -- --help
```

## Known issues

See [`docs/known-issues.md`](docs/known-issues.md) for the current
limitations and deferred work, including Intel/Rosetta 2, Linux daemon-only
support, Windows frozen status, Android pairing requirements, relay TTL
behaviour, sensitive-item sync, and mDNS privacy notes.
