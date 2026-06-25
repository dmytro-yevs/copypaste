# CopyPaste — Architecture

## Crate Dependency Graph

```
copypaste-core          (library — crypto, SQLCipher storage, sensitive detection, config)
  ├── copypaste-daemon  (long-running background process — clipboard, IPC, sync orchestration)
  │     ├── copypaste-ipc       (IPC request/response types shared by daemon, CLI, and UI)
  │     ├── copypaste-p2p       (mTLS P2P transport + mDNS-SD discovery)
  │     ├── copypaste-sync      (sync engine — CRDT, Lamport timestamps, protocol types)
  │     │     └── copypaste-core  (direct dep: crypto + storage used by sync engine)
  │     └── copypaste-supabase  (Supabase cloud sync — opt-in via cloud-sync feature)
  ├── copypaste-cli     (user-facing CLI — no core dep, speaks IPC only)
  └── copypaste-ui      (Tauri v2 + React desktop UI — no core dep, speaks IPC only)

copypaste-relay         (Axum HTTP relay server — standalone, no core dep)
copypaste-android       (UniFFI FFI crate — cdylib for Android, links copypaste-core + copypaste-p2p + copypaste-sync)
copypaste-telemetry     (telemetry, opt-out, PII scrubbing — standalone library)
copypaste-bench         (Criterion benchmarks — dev tool, links copypaste-core)
```

`copypaste-cli` and `copypaste-ui` never link `copypaste-core` directly; they communicate
exclusively through the Unix domain socket owned by `copypaste-daemon`.

`copypaste-relay` is a self-contained HTTP service; it receives only encrypted blobs and
has no access to plaintext or device keys.

`copypaste-android` links `copypaste-core`, `copypaste-p2p`, and `copypaste-sync` directly
(UniFFI cdylib) and exposes a Kotlin API for the Android app — p2p for PAKE pairing and
mTLS cert generation, sync for the transport-agnostic `SyncEngine::run_session`. `copypaste-sync`
also depends directly on `copypaste-core` (crypto and storage types used by the sync engine).
`copypaste-telemetry` and `copypaste-bench` have zero internal deps (relay, P2P, Supabase are
dep-isolated from each other).

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
Unix socket  [macOS: ~/Library/Application Support/CopyPaste/daemon.sock
              Linux: ~/.local/share/copypaste/daemon.sock]
  JSON-RPC-like: { id, method, params } → { id, ok, data?, error?, error_code? }
  Core methods: list | history_page | count | search | status | export
                copy_item | delete_item | delete_all | pin_item | reorder_pinned
  (Legacy verbs "copy", "paste", "delete", "pin" are recognised but return
   error_code="not_implemented" — use the *_item forms above.)
        │
  ┌─────┴──────┐
  ▼            ▼
copypaste-cli  copypaste-ui (Tauri)
(terminal)     (menu-bar tray)
```

### Cross-Device Sync (optional, Phase 4+)

```
copypaste-daemon
  └─► POST /devices/:id/items  (content_type, content_b64, wall_time)
           │
      copypaste-relay  [axum, in-memory + SQLite persistence]
        - bearer token = random 16-byte OsRng token (32 hex chars), NOT derived from pubkey
        - each device registers its shared-inbox device_id with a PoP (HMAC-SHA256 of sync key)
        - quota: 500 items/device inbox; oldest pruned on overflow; TTL default 24 h
           │
      GET /devices/:id/items?since=<wall_time>&since_id=<id>&limit=<n>  ◄── other device's daemon
      GET /devices/:id/subscribe  (SSE, real-time push)  ◄── alternative to polling
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
| Relay auth | `Authorization: Bearer <token>` — random 16-byte `OsRng` token (32 hex chars), constant-time compare via `subtle::ct_eq` | Token is NOT derived from the public key; relay never holds decryption keys |
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

## SQLite Schema (v14)

```sql
-- All columns present on a fully-migrated (v14) database:
clipboard_items  (id PK, item_id, content_type, content BLOB, content_nonce BLOB,
                  blob_ref, is_sensitive, is_synced, lamport_ts, wall_time, expires_at,
                  app_bundle_id,
                  content_hash TEXT,               -- v2: SHA-256 dedup hash
                  origin_device_id TEXT NOT NULL,  -- v3: LWW merge tiebreak
                  key_version INTEGER NOT NULL,    -- v4: HKDF key generation (1 or 2)
                  pinned INTEGER NOT NULL,         -- v7: explicit pin flag (0/1)
                  pin_order REAL,                  -- v8: drag-to-reorder sort key (pinned only)
                  thumb BLOB,                      -- v9: encrypted image thumbnail
                  deleted INTEGER NOT NULL)        -- v10: soft-delete tombstone (0/1)
clipboard_fts    USING fts5(id UNINDEXED, content_text)   -- plaintext search index (non-sensitive only)
devices          (id PK, name, platform, public_key, fingerprint, verified, last_seen)
settings         (key PK, value)
pending_uploads  (item_id PK, tus_url, bytes_uploaded, total_bytes, chunk_format_version,
                  created_at, expires_at)
migration_state  (key PK, key_version_in_progress, last_processed_id, started_at, completed_at)
                  -- v6: resumable v4 key-rotation sweep tracking
revoked_devices  (fingerprint PK, name TEXT, revoked_at INTEGER)
                  -- v12: device-revocation audit log
```

Indexes: `idx_clipboard_wall_time` (DESC), `idx_clipboard_expires` (partial, WHERE NOT NULL),
`idx_clipboard_content_hash` (partial, WHERE NOT NULL), `idx_clipboard_key_version` (partial,
WHERE key_version < 2), `idx_dedup_hash_minute` (UNIQUE, content_hash + wall_time/60 bucket),
`idx_clipboard_item_id` (UNIQUE), `idx_clipboard_pinned` (partial, WHERE pinned = 1),
`idx_clipboard_unpinned_len` (covering, LENGTH(content) WHERE pinned = 0),
`idx_clipboard_deleted` (partial, WHERE deleted = 1),
`idx_clipboard_history_page` (partial on deleted=0, covers pinned DESC + pin_order + wall_time DESC — v14),
`idx_revoked_devices_revoked_at` (DESC).
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
| 5 — E2E sync | Daemon sync loop: push pending_uploads, poll inbox, decrypt | Done |
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
- **Rust 1.96 MSRV**: the following workspace-level pins remain in root `Cargo.toml` (as of
  2026-06-14): `rustls = "0.23"` / `tokio-rustls = "0.26"` / `rcgen = "0.12"` (pinned together
  for ring crypto-provider consistency) and `subtle >= 2.5` (lower-bound for constant-time
  comparison fixes). The upper-bound pins for `uuid`, `clap`, and `home`, and the
  `tempfile <3.14` dev-dep ceiling, were all removed on 2026-06-14 once MSRV was confirmed
  at 1.96 (Rust 1.85+ safe).
