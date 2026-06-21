# CopyPaste — Core Data, Security, Capture & Content Handling

> ⚠️ Snapshot as of 2026-06-04; branch references may be outdated. This inventory was audited
> against branch `v0.6.1-integration`. Gaps listed may have been addressed since.
> Check current code before relying on this inventory.

> Branch: `v0.6.1-integration` | Audit date: 2026-06-04
>
> READ-ONLY survey of `crates/copypaste-core/src/` and
> `crates/copypaste-daemon/src/clipboard.rs`.  Cites file:line for every
> factual claim.  No source was modified.

---

## 1. Encryption

### 1.1 Algorithm

All clipboard content is encrypted with **XChaCha20-Poly1305** (192-bit random nonce, 128-bit auth tag).
`encrypt/decrypt.rs:1-9` — nonce is 24 bytes (`NONCE_SIZE = 24`), tag is 16 bytes (`TAG_SIZE = 16`).
Each call to `encrypt_item_with_aad` generates a fresh nonce via `OsRng.fill_bytes` (`encrypt.rs:116`),
so two encryptions of identical plaintext always produce different ciphertexts.

**User impact**: clipboard contents are unreadable without the device key, even if the SQLite file is extracted from disk.

**Limitation**: a payload exceeding the ChaCha20-Poly1305 per-message limit ((2³²−1)×64 bytes) returns `EncryptError::CipherFailed` — the API never panics on user data (`encrypt.rs:108-126`).

### 1.2 AEAD AAD Binding

Every item ciphertext is bound to its row identity via AEAD authenticated-additional-data (AAD):

| Key version | AAD format | Reference |
|-------------|------------|-----------|
| 1 (legacy)  | `"{item_id}\|{schema_version}"` (schema v3) | `encrypt.rs:76-78` |
| 2 (current) | `"{item_id}\|{schema_version}\|{key_version}"` (schema v4) | `encrypt.rs:95-97` |

`item_id` is the stable cross-device UUID stored in `clipboard_items.item_id`.  If an attacker copies
a ciphertext blob from one row to another the auth tag will reject it — the AAD check fails
(`encrypt.rs:69-97`).  The legacy empty-AAD fallback (`COPYPASTE_ALLOW_LEGACY_AAD`) was permanently
removed in v0.3 (`encrypt.rs:11-13`); v0.2 databases must run `copypaste migrate v3` before upgrading.

Cloud sync items use a distinct `CLOUD_AAD_SCHEMA_VERSION = 5` (`sync_key.rs:28-30`) so cloud ciphertexts
cannot be silently decoded as local ciphertexts.

**User impact**: a database with a copied or replayed row cannot be silently misread — authentication always
fails first.  The schema-version binding means a database opened by a binary expecting schema v10 cannot
successfully decrypt a schema-v3 item using the v4 AAD.

### 1.3 HKDF Key Derivation

Two HKDF families are active simultaneously during the v3→v4 migration window:

**v1 (migration-only):**
- Algorithm: HKDF-SHA256
- Salt: static `b"copypaste-v1-salt"` (`keys.rs:12`)
- Info: `"copypaste-local-storage-v1"` (local) or `"copypaste-v1|{sender}|{recipient}"` (network)
- Exposed via `derive_storage_key_v1` / `DeviceKeypair::local_enc_key` (`keys.rs:132-138`, `255-262`)
- Used ONLY for the v4 ciphertext-migration sweep (decrypt old rows, re-encrypt as v2)

**v2 (current, all new rows):**
- Algorithm: HKDF-SHA512
- Local-storage path: fixed 32-byte salt = SHA-256(`"copypaste/storage-key/v2/hkdf-salt"`) (`keys.rs:100-103`), info `"copypaste-local-storage-v2"`, exposed via `derive_v2` (`keys.rs:119-125`)
- Sync/telemetry path: per-pair salt = SHA-256(`HKDF_SALT_V2_BASE || pair_id`) (`keys.rs:42-51`), info `"copypaste-hkdf-v2|{pair_id}|{purpose}"` where `purpose ∈ {storage, sync, telemetry}` (`keys.rs:60-68`)
- Domain-separated from v1 by both algorithm (SHA-512 vs SHA-256) and different info prefix

**User impact**: each device-pair can rotate its own keys by changing `pair_id` without affecting any
other pair.  Storage, sync, and telemetry keys are independent — a telemetry key leak cannot decrypt
clipboard data.

**Limitation**: `DeviceKeypair::secret_key_bytes()` is `#[deprecated]` (audit MED #3) — it returns a
plain `[u8; 32]` that is not zeroized on stack drop; all new code should use `secret_key_bytes_zeroizing()`
(`keys.rs:179-200`).

### 1.4 Constant-Time Comparisons

The code does not use `==` for secret material comparisons.  The device fingerprint
is a truncated SHA-256 hex string used for user-visible display only (`keychain/mod.rs:101-108`).
Pairing SAS comparisons are handled in `pairing_sm.rs` (not part of this audit scope).
The `subtle` crate is listed in `CLAUDE.md` as the required approach — callers are expected to
use it for token and fingerprint comparisons but specific call sites were not identified in the files
surveyed.

---

## 2. Key Storage

### 2.1 macOS Keychain (Developer-ID-signed builds)

The daemon loads or creates the X25519 device secret via `keychain::load_or_create`
(`keychain/mod.rs:160`).  On a Developer-ID-signed build (`signing::KeyBackend::Keychain`):

- Stored as a `kSecClassGenericPassword` item under service `"com.copypaste.daemon"`, account
  `"device-secret-key"` (`keychain/mod.rs:16-17`).
- Written with `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` and `kSecAttrSynchronizable = false`
  (`keychain/mod.rs:384-468`) so the secret is never included in iCloud Keychain sync or Time Machine.
- ACL is pinned to the three CopyPaste binary paths at creation (`acl::store_with_acl`).
- Only `errSecItemNotFound` (OSStatus -25300) authorises generating a fresh key; locked/denied/timed-out
  reads propagate `KeychainError::Locked` so the daemon degrades cleanly instead of silently minting a
  new key that would make the encrypted DB unreadable (`keychain/mod.rs:241-252`).
- A mid-rotation crash recovery path promotes the `ACCOUNT_ROTATE_BACKUP` slot back to primary
  (`keychain/mod.rs:266-291`).

Additional secrets stored under the same service:
- Cloud sync passphrase-derived key: account `"cloud-sync-key"` (`keychain/mod.rs:21`)
- Supabase GoTrue password: account `"supabase-password"` (`keychain/mod.rs:24`)

### 2.2 File Store (ad-hoc / unsigned builds)

Ad-hoc-signed binaries have a `cdhash` that changes on every rebuild, invalidating the Keychain ACL and
causing a macOS login-password prompt.  These builds use a `0600` file at
`<app_support_dir>/device_secret.key` instead (`keychain/file_store.rs:49`).

Write procedure is atomic: temp file in same directory → `fchmod(0600)` before any secret bytes → write +
flush + sync → `rename` (`keychain/file_store.rs:256-313`).

On first launch after v0.5.1→v0.5.2 upgrade (no key file yet), the store attempts a one-time read of the
legacy Keychain item to preserve the existing encrypted database (`file_store.rs:136-147`).

**User impact**: on signed builds the key never syncs to iCloud.  On unsigned/dev builds the key is a
regular user-readable file — the same security model as "Always Allow" in the Keychain prompt, which
macOS enforces by cdhash.

**Limitation**: a wrong-length key file (corrupted or truncated) is a hard error, never silently treated
as absent — this prevents generating a new key that would orphan the existing encrypted database
(`file_store.rs:232-243`).

---

## 3. SQLCipher Storage

### 3.1 Database File

The local clipboard history is stored in a **SQLCipher** (bundled) database.  The daemon opens it with
`Database::open` / `Database::open_with_cache_mb` (`storage/db.rs:244`).  The `PRAGMA key` is applied
as the very first statement, as required by SQLCipher (`db.rs:270-272`).  An existing plaintext database
is auto-migrated in-place via `sqlcipher_export` atomically unless `COPYPASTE_NO_AUTO_MIGRATE=1`
(`db.rs:318-355`).

**Per-connection pragmas** applied after the key (`db.rs:198-202`):
```
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA temp_store = MEMORY;
```

**WAL mode** is enabled at migration time (`schema.rs:168`).

**Page cache**: default 8 MiB (`SQLITE_CACHE_MB = 8`, `defaults.rs:68`); user-configurable between
1 MiB and 256 MiB, clamped by `AppConfig::clamp_values` (`db.rs:212-219`).

### 3.2 Schema — Tables

Defined in `storage/schema_v1.sql` and extended through ten schema versions:

**`clipboard_items`** — primary history table.  Current columns (schema v10):

| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT PK | per-row UUID (local identity) |
| `item_id` | TEXT UNIQUE | cross-device stable identity; bound in AEAD AAD |
| `content_type` | TEXT | `"text"` / `"image"` / `"file"` |
| `content` | BLOB | XChaCha20-Poly1305 ciphertext (NULL for tombstones) |
| `content_nonce` | BLOB | 24-byte nonce (NULL for image/file items; nonces live per-chunk) |
| `blob_ref` | TEXT | JSON meta for image/file items (width, height, file_id, thumb_file_id, etc.) |
| `is_sensitive` | INTEGER | 1 = sensitive flag (triggers TTL auto-wipe) |
| `is_synced` | INTEGER | 1 = transmitted to relay/peer |
| `lamport_ts` | INTEGER | logical clock for LWW merge |
| `wall_time` | INTEGER | Unix epoch ms at capture |
| `expires_at` | INTEGER | Optional TTL epoch ms (sensitive items) |
| `app_bundle_id` | TEXT | Source app bundle ID (macOS; may be NULL) |
| `content_hash` | TEXT | SHA-256 hex of plaintext; dedup index |
| `origin_device_id` | TEXT | UUID of originating device; LWW tie-break |
| `key_version` | INTEGER | 1 = legacy HKDF-v1; 2 = HKDF-v2 (current) |
| `pinned` | INTEGER | 1 = user-pinned; exempt from all TTL/size pruning |
| `pin_order` | REAL | Fractional sort key for pinned section reorder |
| `thumb` | BLOB | Encrypted capture-time thumbnail (NULL for text / pre-v9 images) |
| `deleted` | INTEGER | 0 = live; 1 = soft-deleted tombstone (op-propagation) |

**`clipboard_fts`** — FTS5 virtual table over `(id UNINDEXED, content_text)`.  Indexes decrypted text for
full-text search.  `id` mirrors `clipboard_items.id`.  (`schema_v1.sql:19-20`)

**`devices`** — paired device registry: `(id, name, platform, public_key, fingerprint, verified, last_seen)`.

**`settings`** — key/value store for daemon and UI settings: `(key TEXT PK, value TEXT)`.

**`pending_uploads`** — tracks in-progress TUS resumable uploads: `(item_id, tus_url, bytes_uploaded, total_bytes, chunk_format_version, created_at, expires_at)`.

**`migration_state`** — tracks the v4 HKDF key-rotation sweep (`db.rs:16-29`).

### 3.3 Schema Versioning & Migrations

Current version: `SCHEMA_VERSION = 10` (`schema.rs:57`).

Migrations run atomically inside a single `BEGIN … COMMIT` transaction (`schema.rs:161-305`).  A
downgrade (on-disk version > binary expectation) returns `SchemaError::Downgrade` rather than silently
proceeding (`schema.rs:179-184`).

Version history:

| Version | Change |
|---------|--------|
| 1 | Baseline schema (clipboard_items, FTS5, devices, settings, pending_uploads) |
| 2 | `content_hash` + index (SHA-256 dedup) |
| 3 | `origin_device_id` (LWW tie-break) |
| 4 | `key_version` (HKDF v1/v2 marker) |
| 5 | UNIQUE index on `content_hash`+minute-bucket (TOCTOU dedup) and `item_id` (sync replay) |
| 6 | `migration_state` table (resumable v4 key-rotation sweep) |
| 7 | `pinned` column (exempt from TTL/size prune) |
| 8 | `pin_order REAL` (drag-to-reorder pinned section) |
| 9 | `thumb BLOB` (capture-time encrypted thumbnail) |
| 10 | `deleted INTEGER` (soft-delete tombstones for LWW op-propagation) |

### 3.4 FTS5 Search

Decrypted text is indexed in `clipboard_fts` at insert time (`items.rs:400-409`).  Queries are
sanitised by `sanitize_fts5_query` (`items.rs:1114-1189`) before being passed as a bound parameter to
`MATCH`:

- Strips all characters except alphanumerics, `_`, `"`, `*`, whitespace.
- Rewrites `-` to space (hyphens are FTS5 NOT-column operators; see `items.rs:1120-1126`).
- Strips FTS5 keywords `NOT`, `OR`, `AND`, `NEAR` case-insensitively.
- Balances double-quote pairs; strips all quotes if count is odd.
- Appends `*` to the last token for prefix search.
- Returns `None` (→ empty results, no error) if no valid tokens remain.

**User impact**: full-text search across clipboard text history, ranked by FTS5 relevance.  Image and file
items are not searchable by content (no FTS entry).

**Limitation**: text previews displayed in the history list are clamped to 1 024 bytes (`MAX_PREVIEW_BYTES`,
`items.rs:1024`) with an ellipsis.  Full content is stored encrypted; the preview comes from the FTS
plaintext index, not re-decryption.

---

## 4. Sensitive-Content Detection

### 4.1 Pattern Matching

`SensitiveDetector` runs every captured text through a `RegexSet` (fast first-pass) then individual
`Regex` matchers (`sensitive/patterns.rs`, `sensitive/detector.rs`).  All input is NFKC-normalised first
(`detector.rs:12-14`) to defeat Unicode bypass tricks (full-width ASCII, ZWJ insertions, etc.).

**37 patterns** are defined in `RAW_PATTERNS` (`patterns.rs:35-186`), grouped into four categories:

| Category | Patterns |
|----------|----------|
| **Credential (0)** | AWS access/temp keys (AKIA/ASIA), GitHub PAT (fine-grained, classic, Actions), OpenAI new/legacy, Anthropic, Stripe live/webhook, npm, PyPI, Slack bot/webhook, Discord bot, Twilio auth token, Google API key, Heroku API key, HashiCorp Vault token, GCP OAuth, SSH private key (PEM, PKCS#8-encrypted, PuTTY), Bearer token, generic `password/secret/api_key` key-value pairs, JWT |
| **Financial (1)** | IBAN, credit card numbers (Luhn algorithm — credit cards bypass the RegexSet; see `detector.rs:241-271`) |
| **PersonalId (2)** | US SSN, email address, US phone number, passport-like number |
| **Infrastructure (3)** | IP:port, database connection string (PostgreSQL, MySQL, MongoDB, Redis, AMQP, MSSQL), AWS ARN, `.env` secret assignments |

**Confidence scores** (from `RAW_PATTERNS` tuples):
- Most credential patterns: 0.99
- JWT: 0.95; HashiCorp Vault: 0.95; OpenAI legacy: 0.95; Heroku: 0.95
- Generic bearer: 0.80; generic password KV: 0.75; Discord bot: 0.85; Twilio: 0.90; IBAN: 0.85
- Infrastructure patterns: 0.70–0.99
- PersonalId low-confidence: email 0.60, phone_us 0.55, passport 0.55

**False-positive suppression** for `generic_password_kv` (`detector.rs:17-67`):
- The captured value must be "strong": length ≥ 10 chars (Unicode scalars, not bytes), OR contain a
  special character `[!@#$%^&*+/=]`, OR mix letters with digits.
- Avoids flagging prose like `password: foo` or `// api_key=demo`.

**Sensitive-app detection** (`detector.rs:278-317`): a hardcoded list of 17 password-manager bundle IDs
(1Password, Bitwarden, KeePass, Dashlane, LastPass, Enpass, NordPass, RoboForm) and their process-name
fragments triggers sensitive treatment regardless of content.

### 4.2 Auto-Wipe Confidence Floor

`is_sensitive_for_autowipe` (`detector.rs:200-229`) applies a **minimum confidence of 0.70** for items
that will receive automatic TTL expiry.  This excludes low-confidence patterns (phone 0.55, passport 0.55,
email 0.60) so routine clipboard content is never silently deleted.  Credit cards (Luhn-validated) are
always included regardless (implicit 0.99).

Items below the 0.70 floor are still marked `is_sensitive = 1` for display purposes but do NOT get an
`expires_at` set by the auto-wipe gate.

**User impact**: an AWS key copied to the clipboard auto-expires in 30 seconds (default
`SENSITIVE_TTL_SECS = 30`, `defaults.rs:51`).  A phone number does not auto-expire.

**Limitation**: the daemon's existing ingest path (`daemon.rs ~line 1177`, noted in a `# FIXWAVE` comment
at `detector.rs:197`) calls `detect(&text).is_some()` for the `is_sensitive` / `expires_at` gate rather
than the correct `is_sensitive_for_autowipe`.  This means some low-confidence patterns (phone, passport,
email) may currently receive an `expires_at`, silently deleting items that the confidence-floor design
intends to keep.

### 4.3 Masking / Redaction

`sensitive::redact::redact` (`redact.rs:14-48`) replaces every matched byte range with `"***REDACTED***"`.
Overlapping ranges are merged so the placeholder is never duplicated.  Range bounds are snapped to UTF-8
character boundaries (`redact.rs:51-67`) to prevent panics on multibyte codepoints.

Redaction is used for logging; it is NOT applied to stored content.  The full ciphertext is always stored.

### 4.4 Sensitive TTL

Sensitive items receive an `expires_at` column value = `wall_time + sensitive_ttl_ms` at capture time.
Default: **30 seconds** (`SENSITIVE_TTL_SECS = 30`, `defaults.rs:51`).  The daemon's cleanup loop calls
`delete_sensitive_expired` (`items.rs:709-741`) periodically.

- `delete_sensitive_expired` filters `is_sensitive = 1 AND wall_time < threshold AND pinned = 0`.
- Pinned items are **exempt** even if marked sensitive (`items.rs:726`).
- FTS entries are pruned in the same transaction to avoid orphaned search-index rows.

**Configurable parameters** (all in `AppConfig`):
- `sensitive_ttl_secs` (default 30): local auto-wipe TTL.  `0` = auto-wipe disabled (sentinel; NOT
  clamped to 1 to avoid silently converting "never wipe" into "wipe after 1s", `config/mod.rs:171-173`).
- `sensitive_ttl_relay_secs` (default 1800): relay-side TTL for sensitive items.
- `sensitive_ttl_local_secs` (default 1800): local-side TTL (separate configurable path).

---

## 5. Retention, Storage Quota & Size Caps

### 5.1 Byte-Based Quota

`prune_to_cap(db, max_bytes)` (`items.rs:910-1010`) enforces a single **byte-only** quota on unpinned
item content:

- Counts `SUM(LENGTH(COALESCE(content,'')))` across all unpinned rows.
- If total > `max_bytes`, evicts oldest unpinned rows (ordered `wall_time ASC, id ASC`) until the excess
  is covered.
- Uses a single-pass SQLite window function (`SUM OVER`) — O(n log n) — to avoid the previous O(n²)
  correlated-subquery.
- The single most-recent unpinned row is protected from eviction even if it alone exceeds the cap
  (prevents a fresh capture from immediately disappearing, `items.rs:934-947`).
- FTS entries for evicted rows are deleted in the same transaction.

Default quota: **10 GiB** (`STORAGE_QUOTA_BYTES = 10 * 1024 * 1024 * 1024`, `defaults.rs:31`).
Minimum floor: **50 MiB** (`MIN_STORAGE_QUOTA_BYTES`, `defaults.rs:40`).
`AppConfig::clamp_values` prevents a sub-floor value from causing self-clearing history (`config/mod.rs:207-208`).

**Pinned items** are **never counted towards quota and never evicted** (`items.rs:887-888`).  There is no
separate cap on pinned content; a user could pin unbounded amounts of data.

### 5.2 Per-Content-Type Size Caps

Enforced at the clipboard-monitor READ gate and again at encode time:

| Content type | Default cap | Min floor | Hard ceiling | Reference |
|---|---|---|---|---|
| Text | 10 MiB (`MAX_TEXT_SIZE_BYTES`) | 64 KiB (`MIN_TEXT_SIZE_BYTES`) | — | `defaults.rs:9,42` |
| Image (raw) | 64 MiB (`MAX_IMAGE_SIZE_BYTES`) | 1 MiB (`MIN_IMAGE_SIZE_BYTES`) | — | `defaults.rs:11,43` |
| File | 100 MiB (`MAX_FILE_SIZE_BYTES`) | 1 MiB (`MIN_FILE_SIZE_BYTES`) | 100 MiB (`MAX_FILE_BYTES`) | `defaults.rs:29,46; file.rs:23` |

Images are also subject to a **decode-bomb cap**: default 50 MiB of decoded pixel memory
(`MAX_DECODED_IMAGE_MB = 50`, `defaults.rs:75`); enforced via `image::Limits` before any large
allocation (`image.rs:79-88`).  A re-encoded PNG that is larger than the raw-input cap (decode
amplification) is also rejected (`image.rs:277-283`).

Sync caps are more restrictive: items over 8 MiB (`SYNC_MAX_BLOB_BYTES` in `sync_orch`) are stored
locally but skipped for P2P/relay sync.  P2P transport frame limit: 16 MiB.

### 5.3 `history_limit` Field

`AppConfig.history_limit` (default 100 000, `defaults.rs:4`) is **deprecated for pruning purposes** —
the schema comment in `config/mod.rs:29` notes it is "Deprecated: no longer used for pruning; retained
for config back-compat."  The byte-only `prune_to_cap` is the active size-control mechanism.

---

## 6. Content Typing

### 6.1 `text_kind` Classifier

`copypaste_core::classify_text(s)` (`text_kind.rs:35-75`) returns one of nine variants in priority order:

| Priority | Kind | Detection rule |
|----------|------|----------------|
| 1 | `Url` | Starts with `http://`, `https://`, or `ftp://` (case-insensitive); no whitespace. `mailto:` excluded here so email catches it. |
| 2 | `Email` | Exactly one `@`; non-empty local and domain; domain has TLD; no whitespace. Handles `mailto:` prefix. |
| 3 | `ColorHex` | Starts with `#`; 3, 4, 6, or 8 hex digits. |
| 4 | `Phone` | Optional `+` prefix; digits/spaces/dashes/parens only; ≥ 7 digits. |
| 5 | `Number` | Optional sign; digits + optional single `.` + optional commas (thousands separator). |
| 6 | `Json` | Starts with `{…}` or `[…]` and parses as valid JSON via `serde_json`. |
| 7 | `FilePath` | Starts with `/`, `~/`, or `C:\` (Windows drive letter); single line; length > 1. |
| 8 | `Code` | Multiline + code signal (`{`, `;`, `=>`, `fn `, `def `, `import `, `class `, `#include`, `</`), OR single-line `=>` or `{ … ; }`. |
| 9 | `PlainText` | Fallback. |

This classification is a **display hint** only — it does not change `content_type` (which is always
`"text"` for text items) and is not persisted to the database.

### 6.2 Image Thumbnails (Variant B)

Introduced in schema v9.  At capture time `encode_image_full` (`image.rs:465-518`) decodes the raw
clipboard bytes once and produces:

1. Full-resolution encrypted chunks (keyed by `file_id`)
2. A downscaled encrypted thumbnail stored in `clipboard_items.thumb` (keyed by `thumb_file_id`)

**Thumbnail parameters** (`image.rs:38`):
- Maximum dimension: **192 px** on the longest side (`THUMBNAIL_MAX_DIM = 192`)
- Format: **PNG** (lossless; WebP encoder not enabled in the `image` crate feature set, `image.rs:337-342`)
- Aspect ratio is preserved; small images are not upscaled (`image.rs:366-370`)

`thumb_file_id` is derived deterministically from `file_id` as SHA-256(`"copypaste-thumb-v1"` || `file_id`)
(`clipboard.rs:88-97`), giving each thumbnail a distinct AEAD context (a wrong `thumb_file_id` fails the
auth tag — `image.rs:384-392`).

Thumbnails are encrypted with the same content key as the full image (`image.rs:351`).
Stored thumbnails whose longest side exceeds the current 192 px cap are flagged for lazy regeneration
(`thumb_dims_exceed_cap`, `image.rs:437-442`).

**User impact**: the UI renders list-row previews from the 192 px thumbnail rather than the full image,
avoiding 200–400 MB WebView memory spikes from decode-bombing the full-res data URI.

**Limitation**: thumbnail backfill for legacy items (captured before schema v9) is on-demand / lazy.
Items without a thumbnail fall back to a placeholder in the UI until backfilled.

### 6.3 File Handling

`encode_file` (`file.rs:65-80`) chunks raw bytes verbatim — NO decode/re-encode step (unlike images).
Files are encrypted with 512 KB chunks (`FILE_CHUNK_SIZE`, `file.rs:21`) identical to the image chunk
substrate.  `blob_ref` JSON carries `filename`, `mime`, `original_size`, `chunk_count`, `file_id`.
MIME type is derived from the file extension at capture time (`clipboard.rs:219-250`); unknown extensions
fall back to `application/octet-stream`.

Files over 8 MiB are stored locally but **not synced** (sync cap `SYNC_MAX_BLOB_BYTES` in `sync_orch`).
Files over 100 MiB are rejected at encode time.

---

## 7. Clipboard Capture Mechanics

### 7.1 Polling

The daemon polls `NSPasteboard.generalPasteboard().changeCount()` on macOS.  Default interval:
**500 ms** (`POLL_INTERVAL_MS = 500`, `defaults.rs:5`); user-configurable between 100 ms and 5 000 ms.

The `changeCount` is read inside an `objc2::rc::autoreleasepool` closure (`clipboard.rs:332`) to prevent
memory leaks from autoreleased Cocoa objects accumulating on the Tokio thread.

Priority: **text > image (PNG/TIFF) > file (`public.file-url`)** (`clipboard.rs:310-313`).  When both
text and image are present on the same clipboard change, image bytes are never materialised — only the
text is returned (`clipboard.rs:389-403`).

### 7.2 Supported Content Types

| Type | NSPasteboard query | Notes |
|------|--------------------|-------|
| Text | `NSPasteboardTypeString` | UTF-8 string |
| Image | `public.png` then `public.tiff` | PNG preferred; TIFF fallback |
| File | `public.file-url` then `NSFilenamesPboardType` | Percent-decoded to filesystem path; file is read at capture |
| Unsupported | `public.rtf`, `public.rtfd`, `public.html`, `public.url`, `com.apple.pasteboard.promised-file-url` | Logged once per kind; never captured |

### 7.3 Self-Write Guard (Echo Suppression)

When the daemon itself writes to `NSPasteboard` (via the `copy_item` / "copy" IPC handler), it stores
the resulting `changeCount` in a shared `Arc<AtomicI64>` (`clipboard.rs:271-272`).  The next poll that
sees this exact `changeCount` suppresses the capture and clears the sentinel (`clipboard.rs:519-535`).

This prevents a "copy an item → daemon writes to pasteboard → daemon immediately re-captures the same
item as a new entry" duplicate loop.

### 7.4 org.nspasteboard Skip Markers

Before reading any content, the monitor checks for three `org.nspasteboard` UTIs
(`org.nspasteboard.TransientType`, `org.nspasteboard.ConcealedType`, `org.nspasteboard.AutoGeneratedType`)
(`clipboard.rs:352-373`).  If any is present, the changeCount is advanced but no content is stored.  This
is the standard protocol used by password managers and other privacy-aware apps to signal "skip this".

**Limitation**: this relies on the source app correctly annotating its copies.  Apps that do not use the
`org.nspasteboard` protocol are not filtered (unless their bundle ID is in `excluded_app_bundle_ids`).

### 7.5 Rapid-Change / Burst Handling

If the changeCount delta since the last poll is ≥ `SKIPPED_BATCH_THRESHOLD = 3` (`clipboard.rs:16`),
intermediate clipboard values were lost.  The monitor logs a warning with the miss count and **falls
through** to capture the most-recent value, rather than returning `SkippedBatch` and losing it
(`clipboard.rs:548-564`).

### 7.6 Deduplication

At insert time `find_recent_by_hash` (`items.rs:470-491`) checks whether a row with the same SHA-256
`content_hash` exists within the last 60 seconds.  A duplicate triggers `bump_item_recency` (promote
existing row to top of history) rather than a new insert (`items.rs:342-409`).

For images, `image_content_hash` is the first 16 bytes of SHA-256(raw) (`clipboard.rs:76-81`).

---

## 8. Notable Gaps Observed

1. **No per-app capture rules** — `excluded_app_bundle_ids` (`AppConfig`) can block apps entirely, but
   there is no mechanism to capture from an app and apply a different TTL, quota tier, or sensitivity
   classification based on the source app's bundle ID.

2. **No manual sensitive toggle** — users cannot mark an individual item as sensitive or non-sensitive
   after capture.  The `is_sensitive` flag is set entirely by the auto-detector at capture time and
   cannot be overridden via IPC.

3. **Fixed thumbnail size** — `THUMBNAIL_MAX_DIM = 192` is a compile-time constant.  There is no user
   knob or per-device configuration.  The format is hardcoded as PNG (lossless); a future lossy codec
   is noted in comments (`image.rs:337-342`) but not implemented.

4. **`history_limit` deprecated but not removed** — The `history_limit` field is retained in
   `AppConfig` for back-compat but is no longer enforced.  Only the byte quota (`storage_quota_bytes`)
   actually limits history length.  A user who sets `history_limit` in their config file will see it
   accepted but silently ignored for pruning.

5. **Low-confidence auto-wipe bug** — A `# FIXWAVE` comment at `detector.rs:197` notes that the daemon's
   `is_sensitive` / `expires_at` gate still calls `detect(&text).is_some()` instead of
   `is_sensitive_for_autowipe`.  This means email addresses (confidence 0.60), phone numbers (0.55), and
   passport-like codes (0.55) may currently receive a 30-second expiry TTL, silently deleting items that
   should be retained per the confidence-floor design.

6. **Pinned-item quota exemption has no ceiling** — Pinned items are excluded from every prune path
   (TTL, sensitive TTL, byte-cap eviction).  There is no maximum on the number of pinned items or total
   pinned bytes, so a user could exhaust disk space by pinning many large images.

7. **No rich-text or RTF capture** — `public.rtf` and `public.html` clipboard types are logged and
   discarded.  Plain text wins when both are present.

8. **File captures are local-only above 8 MiB** — Files between 8 MiB and 100 MiB are stored in the
   local database but silently skipped by the sync engine.  There is no user notification.

9. **Sensitive detection is text-only** — Images and files are never pattern-scanned.  A credit card
   number in a screenshot or a private key in a PDF are stored without the `is_sensitive` flag.

10. **`secret_key_bytes` still present as deprecated** — The deprecated `DeviceKeypair::secret_key_bytes`
    method (audit MED #3, `keys.rs:178-191`) still exists and is called from `copypaste-daemon::platform::macos`
    per the inline comment.  A migration to `secret_key_bytes_zeroizing` is in progress but not complete.
