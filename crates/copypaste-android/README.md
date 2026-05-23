# copypaste-android

## Purpose
Android JNI / UniFFI bindings for `copypaste-core`. Compiles to a `cdylib` (`libcopypaste_android.so`) and ships generated Kotlin bindings via the UniFFI scaffolding.

## Public API
UDL-driven (`uniffi/copypaste_android.udl`). Rust functions exposed to Kotlin:

- `encrypt_text(bytes, key) -> EncryptedBlob` — AEAD encrypt via `copypaste_core::encrypt_item`.
- `decrypt_text(ciphertext, nonce, key) -> Vec<u8>`.
- `is_sensitive(text) -> bool`, `sensitive_kind(text) -> Option<String>`.
- `open_database(path, key) -> u64` / `close_database(handle)` — opaque DB handle table.
- `add_clipboard_item(db_path, key, text) -> String` — returns row UUID, or empty string if content is sensitive. Stub unless `android-uniffi-live` feature is on.
- `get_history_count(db_path, key) -> u64`.

Errors flow through `CopypasteError` (`EncryptionFailed`, `DecryptionFailed`, `DatabaseError`, `InvalidKeyLength`).

## Platform support
Android only. Build via `cargo ndk` or the project's Gradle wrapper.

## Status
beta. Two execution modes:

- default — stub IO returns deterministic values (CI-friendly).
- `--features android-uniffi-live` — real DB I/O via `copypaste-core`.

## Internal vs published
Internal workspace crate. Not published to crates.io. The `.so` and generated Kotlin sources are consumed by the Android app under `android/`.

## Quick example (Kotlin, generated bindings)

```kotlin
val blob = encryptText("hello".toByteArray(), key)
val plain = decryptText(blob.ciphertext, blob.nonce, key)
val handle = openDatabase("/data/data/com.copypaste/cp.db", key)
val rowId  = addClipboardItem("/data/data/com.copypaste/cp.db", key, "hello")
```

## Tests
Unit tests inline in `src/lib.rs` (round-trip, key validation, sensitive skip). No `tests/` directory.

```bash
cargo test -p copypaste-android
cargo test -p copypaste-android --features android-uniffi-live
```
