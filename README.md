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
Android UniFFI ABI (version 13) were bumped, so old pairings will not connect
until re-paired. Re-scan the pairing QR, or re-run LAN discovery + SAS
confirmation, on each device.

## Supported platforms

- **macOS** — arm64 (Apple Silicon) native; Intel (x86_64) is supported via
  Rosetta 2 (release builds are arm64-only — no universal DMG is published);
  install via Homebrew Cask (see `docs/release/brew-tap-setup.md`).
  Requires macOS 14 (Sonoma) or later.
- **Android** — arm64-v8a; UniFFI bindings ship as an `.aar`.

Windows is **frozen** as of 2026-05-23 — see
[`docs/adr/ADR-012-windows-frozen-homebrew-only.md`](docs/adr/ADR-012-windows-frozen-homebrew-only.md)
and [`docs/release/v0.3-plan.md`](docs/release/v0.3-plan.md). Windows users
can run the daemon under WSL2 or wait for the freeze to be lifted; no ETA.

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

| UDL function | Rust signature |
|---|---|
| `encrypt_text(bytes sequence<u8>, key sequence<u8>)` | `fn encrypt_text(bytes: &[u8], key: &[u8]) -> Result<EncryptedBlob, CopypasteError>` |
| `decrypt_text(ciphertext sequence<u8>, nonce sequence<u8>, key sequence<u8>)` | `fn decrypt_text(ciphertext: &[u8], nonce: &[u8], key: &[u8]) -> Result<Vec<u8>, CopypasteError>` |
| `is_sensitive(text string)` | `fn is_sensitive(text: String) -> bool` |
| `sensitive_kind(text string)` | `fn sensitive_kind(text: String) -> Option<String>` |
| `open_database(path string, key sequence<u8>)` | `fn open_database(path: String, key: &[u8]) -> Result<u64, CopypasteError>` |
| `close_database(handle u64)` | `fn close_database(handle: u64)` |

Error variants with associated data (`DecryptionFailed { message }`, `DatabaseError { message }`) are declared as `[Error] interface` in the UDL, matching the Rust struct-variant form.

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
