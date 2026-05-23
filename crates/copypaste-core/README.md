# copypaste-core

## Purpose
Foundational library for CopyPaste: encrypted-at-rest SQLite storage, AEAD crypto, image encoding/chunking, and sensitive-content detection. Used by every other workspace crate.

## Public API
Top-level modules (from `src/lib.rs`):

- `config` — `AppConfig` (user-facing tunables: history limits, TTLs, image quality).
- `crypto` — `DeviceKeypair`, `KeyError`; AEAD via `encrypt_item` / `decrypt_item` (XChaCha20-Poly1305, see ADR-001); per-version AAD (`AAD_SCHEMA_VERSION`), `NONCE_SIZE`; chunked encryption (`encrypt_chunks`, `decrypt_chunks`, `EncryptedChunk`).
- `image` — `encode_image`, `decode_image`, `chunks_to_blob`, `chunks_from_blob`, `thumbnail`, `ImageMeta`, `IMAGE_CHUNK_SIZE`, `MAX_IMAGE_BYTES`.
- `sensitive` — `detect`, `redact`, `luhn_valid`, `is_sensitive_app`, `SensitiveKind`, `SensitiveCategory`, `SensitiveDetector`, `PatternMatch`.
- `storage` — `Database`, `DbError`; item ops in `storage::items` (`insert_item`, `get_page`, `delete_expired`, `delete_sensitive_expired`, `delete_item`, `count_items`, `upsert_fts`, `delete_fts`, `search_items`, `pin_item`, `find_recent_by_hash`); `ClipboardItem`, `ItemsError`.
- `logging` — daily-rotating tracing setup shared by every binary.

## Platform support
All platforms (macOS, Linux, Windows, Android). No platform-specific code at the surface.

## Status
beta — wire format and AAD schema frozen for the 0.2.0-beta series.

## Internal vs published
Internal workspace crate. Not published to crates.io.

## Quick example

```rust,no_run
use copypaste_core::{Database, insert_item, ClipboardItem, encrypt_item};
use std::path::Path;

let key = [0u8; 32]; // load from keychain in real code
let db = Database::open(Path::new("/tmp/cp.db"), &key)?;
let (nonce, ciphertext) = encrypt_item(b"hello", &key)?;
let item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
insert_item(&db, &item)?;
# Ok::<_, Box<dyn std::error::Error>>(())
```

## Tests
17 integration tests under `tests/` (AEAD tamper, concurrent writers, corruption, dedup, encryption-at-rest, HKDF versioning, FTS5 search, image formats, sensitive corpus, …).

```bash
cargo test -p copypaste-core
```

## Related ADRs
- [ADR-001](../../docs/adr/ADR-001-xchacha20-not-aesgcm.md) — XChaCha20-Poly1305 over AES-GCM.
- [ADR-003](../../docs/adr/ADR-003-sqlcipher-at-rest.md) — SQLCipher for at-rest encryption.
- [ADR-004](../../docs/adr/ADR-004-sqlite-wal.md) — SQLite WAL journaling.
