# CopyPaste — Architecture

## Crate Dependency Graph

```
copypaste-core          (library — no binary)
  ├── copypaste-daemon  (long-running background process)
  ├── copypaste-cli     (user-facing CLI, no core dep — speaks IPC only)
  └── copypaste-ui      (Tauri v2 + React desktop UI, speaks IPC only)

copypaste-relay         (optional HTTP sync server — no core dep, standalone)
```

`copypaste-cli` and `copypaste-ui` never link `copypaste-core` directly; they communicate
exclusively through the Unix domain socket owned by `copypaste-daemon`.

`copypaste-relay` is a self-contained HTTP service; it receives only encrypted blobs and
has no access to plaintext or device keys.

## Data Flow

```
System clipboard (NSPasteboard / changeCount polling)
        │  ClipboardMonitor::poll()  [copypaste-daemon/clipboard.rs]
        ▼
Sensitive detection  [copypaste-core/sensitive/detector.rs]
  regex patterns: API keys, tokens, secrets → is_sensitive = true
        │
        ▼
XChaCha20-Poly1305 encrypt  [copypaste-core/crypto/encrypt.rs]
  key  = DeviceKeypair::local_enc_key()  (HKDF-SHA256 from X25519 secret)
  nonce = 24-byte random (OsRng)
        │
        ▼
SQLite (WAL mode)  [copypaste-core/storage/]
  clipboard_items  — encrypted BLOB + nonce
  clipboard_fts    — FTS5 virtual table (plaintext, indexed before key is discarded)
  pending_uploads  — resumable upload state (TUS-style)
  devices / settings
        │
        ▼
Unix socket  [~/.local/share/copypaste/copypaste.sock]
  JSON-RPC-like: { id, method, params } → { id, ok, data?, error? }
  Methods: list | count | delete | search | status | copy | export
        │
  ┌─────┴──────┐
  ▼            ▼
copypaste-cli  copypaste-ui (Tauri)
(terminal)     (menu-bar tray)
```

### Cross-Device Sync (optional, Phase 4+)

```
copypaste-daemon
  └─► POST /upload  (ciphertext_b64, nonce_b64, lamport_ts)
           │
      copypaste-relay  [axum, in-memory + SQLite persistence]
        - bearer token = SHA-256(public_key_bytes)[0..32 hex chars]
        - fan-out: item lands in every OTHER device's inbox
        - quota: 500 items/device inbox; oldest pruned on overflow
           │
      GET /poll  ◄── other device's daemon
```

## Security Architecture

| Layer | Mechanism | Notes |
|-------|-----------|-------|
| Key storage | macOS Keychain (`security-framework`) | Service `com.copypaste.daemon`, account `device-secret-key`. Falls back to ephemeral key on non-macOS. |
| Key material | X25519 `StaticSecret` (`x25519-dalek`, `ZeroizeOnDrop`) | Never written to disk in plaintext |
| Local encryption key | `HKDF-SHA256(secret, info="copypaste-local-storage-v1")` | Derived once at startup; never transmitted |
| Network encryption key | `HKDF-SHA256(ECDH(self, peer), info="copypaste-v1\|sender\|recipient")` | Per-pair, domain-separated |
| Cipher | XChaCha20-Poly1305 (192-bit nonce) | Preferred over AES-GCM: 192-bit nonce eliminates nonce-reuse risk with random generation; no hardware requirement; `chacha20poly1305` crate is pure Rust |
| Sensitive TTL | `expires_at` set for sensitive items | Daemon purges via `delete_expired` on each tick |
| FTS5 plaintext | Indexed before encryption; never stored as plaintext column | Separate virtual table; can be cleared independently |
| Relay auth | `Authorization: Bearer <token>` — constant-time comparison via `subtle` crate | Relay never holds decryption keys |
| Rate limiting | `tower_governor` on relay routes | Prevents inbox flooding |

### Why XChaCha20-Poly1305 over AES-GCM

- 192-bit random nonce: with 24 bytes of randomness, birthday-collision risk is negligible even
  at billions of items, removing the need for a nonce counter.
- No hardware dependency: AES-NI is absent on some targets (older ARM, CI builders); ChaCha20 is
  constant-time in pure software.
- Nonce misuse resistance: XChaCha20 derives an internal subkey from the nonce, providing an
  extra margin against implementation errors.

### Why Unix Socket over HTTP for Daemon IPC

- No port conflicts, no firewall rules, no localhost binding surface.
- File-system permissions (`chmod 600`) restrict access to the owning user without additional
  auth tokens.
- Lower latency than TCP loopback for short-lived CLI invocations.
- Simpler: no TLS, no HTTP framing; newline-delimited JSON is sufficient.

## SQLite Schema (v1)

```sql
clipboard_items  (id PK, item_id, content_type, content BLOB, content_nonce BLOB,
                  blob_ref, is_sensitive, is_synced, lamport_ts, wall_time, expires_at,
                  app_bundle_id)
clipboard_fts    USING fts5(id UNINDEXED, content_text)   -- plaintext search index
devices          (id PK, name, platform, public_key, fingerprint, verified, last_seen)
settings         (key PK, value)
pending_uploads  (item_id PK, tus_url, bytes_uploaded, total_bytes, chunk_format_version,
                  created_at, expires_at)
```

Indexes: `idx_clipboard_wall_time` (DESC), `idx_clipboard_expires` (partial, WHERE NOT NULL).
WAL mode + 8 MB cache. Schema versioned via `PRAGMA user_version`.

## Phase Roadmap

| Phase | Scope | Status |
|-------|-------|--------|
| 1 — Core | `copypaste-core`: crypto, storage, sensitive detection, config | Done |
| 1b — Daemon | `copypaste-daemon`: clipboard polling, IPC server, keychain | Done |
| 2 — CLI | `copypaste-cli`: list, count, delete, search, copy, export | Done |
| 2a — FTS | Full-text search via FTS5, IPC `search` method | Done |
| 3 — UI | `copypaste-ui`: Tauri v2 + React desktop app, menu-bar tray (see ADR-013) | Done |
| 3b — Linux | systemd user service unit + install script | Done |
| 4 — Relay | `copypaste-relay`: device registration, upload/poll, quota | Done |
| 5 — E2E sync | Daemon sync loop: push pending_uploads, poll inbox, decrypt | Planned |
| 6 — Large items | Chunked encryption (`encrypt_chunks`), resumable TUS upload | Partial (core ready) |

## Key Design Decisions

- **Tauri UI (ADR-013)**: `copypaste-ui` uses Tauri v2 + React/TypeScript/Vite for the
  desktop frontend. The Rust Tauri layer bridges IPC calls to the daemon over the existing
  Unix socket; no `copypaste-core` linkage. macOS-only as of v0.4 (Windows support is
  frozen — see ADR-012).
- **Lamport timestamps**: logical clock on `ClipboardItem` enables conflict-free ordering across
  devices without wall-clock trust.
- **Fan-out at relay**: relay writes each uploaded item into every other registered device's
  inbox immediately; no push notifications needed — devices poll on a configurable interval.
- **Rust 1.75 MSRV**: several dependencies pinned below their latest versions to stay compatible
  (`tempfile <3.14`, `home <0.5.10`, `clap <4.5.40`).
