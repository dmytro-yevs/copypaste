# CopyPaste

Cross-platform clipboard sync with end-to-end encryption.

## Architecture

```
crates/
  copypaste-core/      — pure-Rust library (encryption, detection, database)
  copypaste-android/   — UniFFI FFI crate (cdylib + bindgen binary)
  copypaste-relay/     — Axum relay server
  copypaste-cli/       — CLI frontend
android/               — Android Studio project
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

## Alpha caveats (v0.1.0-alpha)

This is an early alpha. Known limitations:

- **P2P sync is scaffolding:** the `copypaste-p2p`, `copypaste-sync` crates exist but are not wired into the daemon yet. Pairing UI is a preview.
- **Cloud sync (Supabase) is integration-tested only:** end-to-end push/pull requires valid `SUPABASE_URL`/`SUPABASE_ANON_KEY` and the schema migration applied to your project.
- **Relay is in-memory only:** the optional `copypaste-relay` service does not persist devices or items across restart. Use only for testing.
- **IPC protocol is unversioned:** the daemon, CLI, and UI must be built from the same commit. Mixed versions may break silently.
- **Windows daemon is a stub:** named-pipe IPC scaffolding exists but the daemon does not run on Windows yet.
- **Android is preview only:** UniFFI bindings + a Kotlin skeleton exist, but no signed APK is validated end-to-end.
- **No code signing or notarisation yet.** macOS builds are unsigned.
