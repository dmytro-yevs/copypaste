# Backend Performance Audit — CopyPaste Rust Backend

**Branch:** `v0.6.1-integration`  
**Date:** 2026-06-04  
**Scope:** `copypaste-daemon/src/`, `copypaste-core/src/`, `copypaste-sync/`, `copypaste-p2p/`, `copypaste-relay/`, `copypaste-supabase/`  
**Rules:** Read-only. No code changes.

---

## 1. Clipboard Polling

### CLIP-1 — NSString UTI objects allocated on every poll tick (idle case)
**File:** `crates/copypaste-daemon/src/clipboard.rs:352-387`  
**Impact:** Medium | **Effort:** S

Every call to `poll()` unconditionally allocates 5+ `NSString` objects via
`NSString::from_str(...)` (3 for the org.nspasteboard probe, 2 for
`public.png`/`public.tiff`) **before** the change-count guard can short-circuit.
When no paste has occurred this work is pure waste at the default 500 ms cadence
(120 allocs/min even at rest). The `changeCount` guard at line 339 fires first and
returns `None`, but the three nspasteboard `NSString` + `NSArray` objects are
created inside the `autoreleasepool` closure unconditionally before the check.

**Recommendation:** Move the `count == self.last_change_count` early-return check
*outside* the `autoreleasepool` closure (fetch only `changeCount` first, then open
the pool only when there is a change). All UTI `NSString` objects and the
`NSArray` probes would then be allocated only when content actually changed.

---

### CLIP-2 — Unsupported-kind probe allocates 5 `NSString`+`dataForType` round-trips on every changed event
**File:** `crates/copypaste-daemon/src/clipboard.rs:467-485`  
**Impact:** Low | **Effort:** S

The unsupported-kind walk (`public.rtf`, `public.rtfd`, …) probes 5 UTIs with
both `dataForType` and `stringForType` (10 ObjC calls) on every changed event
when no text/image/file was found. The same branch creates a fresh heap-allocated
`Vec<String>` each time. Because the result is logged once per kind (SEEN set),
the IPC cost keeps recurring but produces no new log after the first time.

**Recommendation:** Use `availableTypeFromArray` over the full probe list (one ObjC
call) rather than 10 individual probes. The SEEN gate already ensures only the
first occurrence logs; the probe cost is the larger issue.

---

### CLIP-3 — 500 ms default poll interval with no adaptive back-off on idle
**File:** `crates/copypaste-core/src/config/defaults.rs:5`  
**Impact:** Medium | **Effort:** M

`POLL_INTERVAL_MS = 500` is fixed. The daemon wakes the Tokio reactor, calls
`NSPasteboard.changeCount`, and touches the DB (cleanup counters) 120 times/minute
even when the machine is idle for hours. macOS provides `NSPasteboardDidChangeNotification`
(Cocoa) and `CGEventTap` (lower-level) as event-driven alternatives that would
reduce idle wakeups to near zero.

**Recommendation:** Implement a fallback "idle stretch" mode: after N consecutive
no-change polls (e.g. 30 s of unchanged count) back off to 2–5 s ticks until the
count changes; resume 500 ms after the first change. Alternatively, register a
`NSPasteboardDidChangeNotification` observer on the main thread and only spin the
capture path when notified.

---

## 2. SQLCipher / Storage

### DB-1 — `get_page`, `get_page_pinned_first`, `get_page_meta`: `prepare()` on every call
**File:** `crates/copypaste-core/src/storage/items.rs:502, 532, 596`  
**Impact:** High | **Effort:** S

`get_page`, `get_page_pinned_first`, and `get_page_meta` all call
`db.conn().prepare(...)` on every invocation, recompiling the SQL to a bytecode
plan on every IPC `history_page` request. `search_items` already uses
`prepare_cached` (line 1218) and the comment at line 1197 calls out the win.
These three hot queries hit the DB on every UI list refresh and popup open, and
each re-compilation requires locking the schema cache inside SQLite.

**Recommendation:** Switch all three to `prepare_cached`. The statement is
parameterized only with `LIMIT`/`OFFSET`, so the cache key is stable across calls.

---

### DB-2 — Per-row FTS DELETE in prune loops: O(n) round-trips inside one transaction
**File:** `crates/copypaste-core/src/storage/items.rs:696-700, 735-739, 1004-1009`  
**Impact:** High | **Effort:** M

`delete_expired`, `delete_sensitive_expired`, and `prune_to_cap` all:
1. Collect evicted IDs into a `Vec<String>`.
2. Loop over the Vec, issuing one `DELETE FROM clipboard_fts WHERE id = ?1` per row.

During a cloud backfill that evicts hundreds of rows, this is O(n) individual SQL
executions inside one transaction. FTS5 supports a bulk delete via a `DELETE …
WHERE id IN (SELECT …)` correlated sub-query, or by building a single
`DELETE FROM clipboard_fts WHERE id IN (?, ?, …)` with a dynamically-constructed
IN list (safe for reasonable batch sizes).

**Recommendation:** Replace the per-row FTS loop with a single `DELETE FROM
clipboard_fts WHERE id IN (SELECT id FROM clipboard_items WHERE <same predicate>)`
executed *before* the main item delete, inside the same transaction. This collapses
N round-trips to 1 and avoids materialising the ID vec at all.

---

### DB-3 — `prune_to_cap` runs the window-function CTE twice
**File:** `crates/copypaste-core/src/storage/items.rs:962-1008`  
**Impact:** Medium | **Effort:** S

`prune_to_cap` computes the same running-cumulative-sum window function CTE twice:
once to collect the eviction ID list (for the FTS loop), then again inside the
`DELETE … WHERE id IN (SELECT id FROM ranked …)`. With a large DB and a large
eviction batch this scans the unpinned rows twice.

**Recommendation:** Collect the IDs once. Then use a single
`DELETE FROM clipboard_items WHERE id IN (?,…)` with the collected IDs (or a
temp table), eliminating the second full CTE execution.

---

### DB-4 — `exists_item_by_item_id` uses `COUNT(1)` when `LIMIT 1` is cheaper
**File:** `crates/copypaste-core/src/storage/items.rs:671-677`  
**Impact:** Low | **Effort:** S

```sql
SELECT COUNT(1) FROM clipboard_items WHERE item_id = ?1
```
scans all matching rows. Because `item_id` carries a UNIQUE index, there is at
most one match, but SQLite still evaluates the aggregate. `SELECT 1 FROM
clipboard_items WHERE item_id = ?1 LIMIT 1` stops at the first row.
Used in the relay ingest hot-path (line 659 of `relay.rs`).

**Recommendation:** Replace with `query_row("SELECT 1 FROM … LIMIT 1", …).optional()`.

---

### DB-5 — Missing composite index on `(wall_time, pinned)` for the main page query
**File:** `crates/copypaste-core/src/storage/schema_v1.sql:16`  
**Impact:** Medium | **Effort:** S

`get_page` and `get_page_meta` filter `WHERE deleted = 0` and sort by
`wall_time DESC`. The existing `idx_clipboard_wall_time` index covers only
`wall_time DESC`. Because `deleted` was added in schema v10 after the index was
created, the query now applies a post-scan filter over the full index range.

With a large history the planner might opt out of the index entirely when the
selectivity of `deleted = 0` is very high (almost all rows are not deleted). A
partial index `ON clipboard_items(wall_time DESC) WHERE deleted = 0` would cover
both conditions in one tight index.

**Recommendation:** Add a new migration (v11) that creates:
```sql
CREATE INDEX IF NOT EXISTS idx_clipboard_live_wall_time
    ON clipboard_items(wall_time DESC) WHERE deleted = 0;
```

---

### DB-6 — Single `tokio::sync::Mutex<Database>` serialises ALL DB access
**File:** `crates/copypaste-daemon/src/daemon.rs:178`, `crates/copypaste-core/src/storage/pool.rs`  
**Impact:** High | **Effort:** L

The entire daemon — clipboard capture, IPC handlers (every `history_page`,
`copy_item`, `search`), sync_orch merge, cloud/relay ingest, TTL cleanup — all
queue behind one `tokio::sync::Mutex<Database>`. Read-only IPC verbs (history,
search, preview) block behind exclusive write operations (insert, prune). An
`r2d2` connection pool exists in `pool.rs` with full WAL + busy-timeout setup
but is NOT wired into the daemon (the module comment at `pool.rs:7-8` acknowledges
"Phase 1 … Daemon wiring will migrate in Wave 3.1").

**Recommendation:** Complete the `pool.rs` integration: wire the pool for read-only
IPC verbs. WAL mode already serialises writer-vs-reader at the SQLite level, so
reads and the single writer can proceed concurrently. This eliminates IPC latency
spikes caused by a slow cloud ingest holding the mutex.

---

### DB-7 — `sensitive_ttl` cleanup runs every 5 s even when sensitive items are rare
**File:** `crates/copypaste-daemon/src/daemon.rs:1000-1019`  
**Impact:** Low | **Effort:** S

`SENSITIVE_CLEANUP_INTERVAL_MS = 5_000` means a `spawn_blocking` + DB acquisition
every 5 seconds unconditionally. The query touches the DB with `WHERE is_sensitive = 1
AND wall_time < ?1 AND pinned = 0` even when there are no sensitive items. There
is no index on `is_sensitive`.

**Recommendation:** Add a partial index `ON clipboard_items(wall_time) WHERE
is_sensitive = 1 AND pinned = 0` so the cleanup query is O(sensitive items), not
O(all items). Also consider tracking a flag in memory: if no sensitive item was
captured since the last wipe, skip the DB query entirely.

---

## 3. Cryptography

### CRYPTO-1 — `derive_v2` called on every text item decrypt in the hot sync path
**File:** `crates/copypaste-daemon/src/sync_common.rs:105`, `sync_common.rs:311`, `sync_orch.rs:988`  
**Impact:** Medium | **Effort:** S

`decrypt_item_plaintext` calls `derive_v2(&v1_key)` on every invocation:
```rust
let v2_key = derive_v2(&v1_key);  // HKDF-SHA512 each call
decrypt_item_by_version(item.key_version, &v1_key, &v2_key, …)
```
For the cloud/relay receive path this is called once per ingested item. During a
backfill of 500 items, that is 500 × HKDF-SHA512 derivations, each producing a
key that is identical across all calls (same `v1_key` for the session).

`SyncCrypto` already caches `v2_key` at construction. The `sync_common` module
does not have access to a `SyncCrypto` instance; it takes only
`&zeroize::Zeroizing<[u8; 32]>`.

**Recommendation:** Cache the derived `v2_key` in `build_local_item`'s caller
scope (cloud/relay ingest loops) so it is derived once per ingest session rather
than once per item. Alternatively thread `SyncCrypto` into `sync_common` functions.

---

### CRYPTO-2 — `encrypt_item_with_aad` / `decrypt_item_with_aad` construct a new cipher instance per call
**File:** `crates/copypaste-core/src/crypto/encrypt.rs:109-126, 138-153`  
**Impact:** Low | **Effort:** M

Both functions call `XChaCha20Poly1305::new(key.into())` on every call, which
initialises the ChaCha20 key schedule (32 bytes → expanded 40-word state) and
allocates a new cipher object. For text items this is called once per capture,
which is fine. For a cloud backfill of 500 items it is called 500 times with the
same key.

**Recommendation:** For batch operations (migration sweep, cloud ingest) pass a
pre-constructed `XChaCha20Poly1305` instance rather than a raw key, letting the
caller cache the cipher across multiple calls. Add `encrypt_batch_with_aad` /
`decrypt_batch_with_aad` that take `&XChaCha20Poly1305`.

---

### CRYPTO-3 — `shared_sync_key()` reads `peers.json` from disk on EVERY outbound item
**File:** `crates/copypaste-daemon/src/sync_orch.rs:117-132`  
**Impact:** High | **Effort:** S

`SyncCrypto::shared_sync_key()` calls `crate::peers::load_peers(&self.peers_path)`
on every call. `rekey_outbound` (called for every local clipboard item broadcast)
calls `shared_sync_key()` which re-reads the JSON file every time:

```
local item broadcast → rekey_outbound → shared_sync_key → load_peers (disk I/O)
```

At the default 500 ms poll interval with frequent clipboard use this is a
filesystem read per captured item. `catchup_items` (called on every P2P
connect) also reads `peers.json` once per page, but the pre-flight read is
separate — so it reads it `(N_pages + 1)` times for a full history.

**Recommendation:** Cache the parsed `Vec<PeerDevice>` in `SyncCrypto` (wrapped in
an `Arc<RwLock<_>>` if live update is needed) and invalidate/reload only when the
`peers.json` mtime changes, or on an explicit post-pairing notification.

---

## 4. Sync

### SYNC-1 — `rekey_outbound` called for every broadcast item even when no peer is connected
**File:** `crates/copypaste-daemon/src/sync_orch.rs:189-200`  
**Impact:** Medium | **Effort:** S

The sync orchestrator's main loop calls `rekey_outbound` (decrypt + re-encrypt
under shared key) for every locally-captured item regardless of whether any P2P
peer is currently connected. When P2P is enabled but no peer is online, every
clipboard capture pays:
- `shared_sync_key()` → `load_peers` (disk I/O)
- `decrypt_item_by_version` (HKDF + XChaCha20 decrypt)
- `encrypt_for_cloud` (XChaCha20 encrypt)

then `outbound_tx.send(wire)` fails because the receiver (P2P outbound loop)
has no connected sink.

**Recommendation:** Before calling `rekey_outbound`, check whether the outbound
channel has capacity/receivers (e.g. use `outbound_tx.receiver_count() > 0` or a
companion `Arc<AtomicBool> has_peers` toggled by the P2P accept/disconnect loop).
When no peer is connected, skip the re-key entirely.

---

### SYNC-2 — Cloud poll at 60 s (WS connected) still spins `spawn_blocking` every tick
**File:** `crates/copypaste-daemon/src/cloud.rs:97, 1721-1800`  
**Impact:** Low | **Effort:** S

`POLL_INTERVAL_WS_CONNECTED = 60 s`. Each tick triggers a `spawn_blocking` to
read the watermark from the settings table even when the WebSocket is delivering
items in real-time and the `since` filter will return an empty batch. A WebSocket
event should be sufficient to trigger ingest; the polling loop primarily serves as
a fallback. When `page.is_empty()` the poll wasted a blocking thread acquisition
and a DB read.

**Recommendation:** When WebSocket is connected and no items arrive within the
interval, skip the HTTP poll (the WS subscription covers it). Retain the 10 s
fallback polling for WS-disconnected case.

---

### SYNC-3 — Relay receive loop polls every 5 s unconditionally; no SSE path
**File:** `crates/copypaste-daemon/src/relay.rs:73, 767`  
**Impact:** Medium | **Effort:** M

`POLL_INTERVAL = Duration::from_secs(5)` drives the relay receive loop with no
adaptive back-off. The relay server exposes an SSE endpoint (see
`crates/copypaste-relay/src/routes/items.rs`) but the daemon client never
subscribes to it — the module comment notes "polling is the portable backstop."
Each 5 s tick issues an HTTP GET with a bearer token, a `spawn_blocking` + mutex
acquisition, and a `prune_to_cap` scan when items arrive.

**Recommendation:** Implement SSE subscription in the relay daemon client.
Received SSE events trigger immediate ingest. The polling loop remains as a
fallback (backoff to 30 s when SSE is healthy), matching the cloud path's
two-mode design.

---

### SYNC-4 — Inbound blob items re-encode (decode → re-chunk) on every receive
**File:** `crates/copypaste-daemon/src/sync_orch.rs:1044-1123`, `sync_common.rs:411-450`  
**Impact:** Medium | **Effort:** M

Every received image/file item from P2P or cloud undergoes:
1. `decrypt_from_cloud` → PNG/raw bytes (1× decrypt)
2. `encode_image_with_limit` → PNG decode (image crate) → WebP/PNG re-encode + chunk (1× codec round-trip)
3. `chunks_to_blob` + DB write

This is a full image codec round-trip on the receiving device even if the sender
already stored the same PNG at the same resolution. Items arriving from cloud
backfill (e.g. 50 images from a long-offline device) will trigger 50 full
decode+encode operations sequentially inside `spawn_blocking`.

**Recommendation:** If the remote image is already PNG and within size limits,
skip the codec round-trip and store the raw PNG bytes directly as a single chunk
(no `encode_image_with_limit`). Use the decode+re-encode path only for TIFF or
oversized images that need downscaling. The `file_id` can still be derived from
SHA-256 of the raw bytes.

---

### SYNC-5 — `merge_incoming_with_crypto` issues one `spawn_blocking` per incoming item
**File:** `crates/copypaste-daemon/src/sync_orch.rs:223-228`  
**Impact:** Medium | **Effort:** M

The sync orchestrator's `incoming` arm calls:
```rust
merge_incoming_with_crypto(&db, vec![wire], crypto.as_ref(), …).await
```
with a single-item `vec![]` on every `incoming_rx.recv()`. Each call spawns a
blocking task, acquires the DB mutex, runs the merge, prunes, and releases.
For a P2P burst of 100 items (catch-up on reconnect), this is 100 sequential
`spawn_blocking` → mutex cycles instead of one batch.

**Recommendation:** Drain `incoming_rx` in a short `try_recv` loop to collect a
batch (up to e.g. 64 items), then call `merge_incoming_with_crypto` once with the
full batch. The function already accepts `Vec<WireItem>` and processes the batch
in a single blocking closure.

---

## 5. Concurrency

### CONC-1 — `spawn_blocking` per IPC handler call; no connection pool
**File:** `crates/copypaste-daemon/src/ipc.rs`, `crates/copypaste-daemon/src/daemon.rs:178`  
**Impact:** High | **Effort:** L

Every IPC method that touches the DB does:
```rust
tokio::task::spawn_blocking(move || {
    let guard = db.blocking_lock();
    …
})
```
All IPC calls — even read-only ones like `history_page`, `search`, `get_item` —
contend on the same global `Mutex<Database>` from a spawned blocking thread.
Tokio's blocking thread pool grows unboundedly under pressure. With 10 concurrent
IPC clients each doing `history_page`, there are 10 blocked threads each holding a
blocking-thread slot waiting for the mutex.

This is the same root problem as DB-6. The fix is the same: wire the `SqlitePool`
so reads use a pooled read connection and the write mutex is only contested by actual
writes.

---

### CONC-2 — `std::sync::RwLock<AppConfig>` acquired on every poll tick
**File:** `crates/copypaste-daemon/src/daemon.rs:971-974`  
**Impact:** Low | **Effort:** S

```rust
let live_config = core_config_arc
    .read()
    .map(|g| g.clone())
    .unwrap_or_else(|_| config.clone());
```
`AppConfig` is cloned on every 500 ms tick. `AppConfig` is a non-trivial struct
(~10 fields including `String` and `Option<String>` heap allocations). Under the
default config `set_config` is rare; the lock is almost never contended, but the
clone allocates at 120 calls/min.

**Recommendation:** Use `Arc::clone(&core_config_arc)` inside the tick and compare
a generation counter rather than cloning the full struct. Or hold the `RwLockReadGuard`
only for the specific field reads needed per tick rather than cloning the whole config.

---

### CONC-3 — Broadcast channel size 256; lagged subscribers silently lose items
**File:** `crates/copypaste-daemon/src/daemon.rs:499`  
**Impact:** Low | **Effort:** S

`broadcast::channel::<ClipboardItem>(256)`. There are currently 4 subscribers
(sync_orch, cloud, relay, P2P outbound). Each subscriber advances its own cursor;
if any one subscriber falls behind by 256 items, `RecvError::Lagged` is returned
and those items are silently dropped from that subscriber's stream. Under a rapid
clipboard burst (e.g. automated paste loop) the sync and relay paths would lose
items.

The comment notes this was bumped from 64 for this reason. 256 is still a fixed
ceiling. Each `ClipboardItem` carries a full encrypted `content: Option<Vec<u8>>`
blob (potentially hundreds of KB for image items), so the raw memory cost at 256
capacity can reach ~25 MB resident.

**Recommendation:** Use lightweight `item_id + lamport_ts` tokens on the broadcast
channel and re-fetch the full item from DB in subscribers that need the content.
This removes the blob from the broadcast, reduces memory pressure, and makes the
channel capacity far less sensitive to content size.

---

## 6. Memory / Allocations

### MEM-1 — Image bytes copied to `Vec<u8>` via `.to_vec()` in NSPasteboard read
**File:** `crates/copypaste-daemon/src/clipboard.rs:396-399`  
**Impact:** Medium | **Effort:** S

```rust
let png_data = unsafe { pb.dataForType(&png_type) };
if let Some(ref d) = png_data {
    Some(d.bytes().to_vec())   // full copy of potentially multi-MB image
}
```
The `NSData.bytes()` slice is valid only until `autoreleasepool` drains. The
`.to_vec()` allocates a fresh heap buffer and copies the entire raw image (up to
64 MiB by default) into it. For TIFF this happens twice (PNG fallback on miss).
The full bytes then travel through `handle_image` → `encode_image_full` which
re-encodes them, so the original raw copy is live alongside the encoded copy.

**Recommendation:** Avoid holding both raw and encoded copies simultaneously by
passing the raw slice directly into the encode pipeline rather than materialising
an owned `Vec` first. This requires changing the encode entry point to accept
`&[u8]` (it already does), and restructuring `handle_image` to not store the raw
bytes after the encode starts.

---

### MEM-2 — `ClipboardItem` cloned into broadcast channel carries full `content` blob
**File:** `crates/copypaste-daemon/src/daemon.rs:499` (see CONC-3 above)  
**Impact:** Medium | **Effort:** S

Same issue as CONC-3. Every `ClipboardItem` broadcast to sync subscribers carries
`content: Option<Vec<u8>>` — the full encrypted ciphertext blob (identical to the
DB content). For a 4 MB image item this means a second live copy of those 4 MB in
memory while the broadcast channel holds a slot.

**Recommendation:** Broadcast only the primary key (`id: String`) and let
subscribers fetch content on demand. For sync/relay paths that need content, one
DB read per consumed item replaces the multi-MB in-flight copy.

---

### MEM-3 — Base64 encode/decode round-trip on every relay envelope
**File:** `crates/copypaste-daemon/src/relay.rs:349-365`  
**Impact:** Medium | **Effort:** S

The relay push path encodes ciphertext to base64, wraps it in a JSON struct
(`RelayEnvelope`), serialises the JSON to bytes, then base64-encodes the entire
JSON again for the outer `content_b64`. This is double-base64 with two intermediate
heap allocations per pushed item:

```
blob (binary) → base64(blob) → JSON(RelayEnvelope{ct_b64}) → base64(JSON)
```

The relay server presumably stores `content_b64` verbatim and returns it on pull.
The receiver undoes both layers on ingest.

**Recommendation:** Eliminate the inner `ct_b64` by storing the blob bytes directly
in the outer JSON as a base64 field. The envelope becomes:
```json
{"item_id":"…","lamport_ts":1,"ct":"<base64(blob)>"}
```
This halves the base64 work and removes one intermediate `Vec<u8>` per item
on both push and pull.

---

### MEM-4 — `sanitize_fts5_query` allocates multiple intermediate `String`s per search
**File:** `crates/copypaste-core/src/storage/items.rs:1114-1188`  
**Impact:** Low | **Effort:** S

`sanitize_fts5_query` creates:
1. `cleaned: String` (char-map + filter collect)
2. Conditionally a second `balanced: String` (odd-quote strip collect)
3. `parts: Vec<String>` (per-token `format!("{tok}*")`)
4. `parts.join(" AND ")` → final `String`

For a typical 1–3 word search query this is 4+ allocations before the SQL is
executed. At interactive typing rates (keypress per search), the allocation
pressure is measurable.

**Recommendation:** Use a single `String::with_capacity(raw.len() * 2)` and build
the output in-place rather than collecting intermediate iterators. The token-prefix
step can push directly into the output buffer.

---

## 7. P2P Transport

### P2P-1 — `WireItem` JSON serialisation on every outbound item (no binary framing)
**File:** `crates/copypaste-sync/src/protocol.rs` (via `serde_json`)  
**Impact:** Medium | **Effort:** M

`WireItem` is serialised as JSON with `serde_json` before being length-prefixed on
the P2P wire. For an image `WireItem` carrying a multi-MB `content` blob, the JSON
encoding base64-expands the binary content field by ~33%, inflating a 4 MB blob to
~5.4 MB on the wire. All fields (including string UUIDs, metadata, booleans) are
re-serialised on every forward.

**Recommendation:** Consider switching the P2P data plane to `bincode` or
`postcard` (already a workspace dependency in some projects) or MessagePack for
wire encoding. A binary format eliminates the base64 overhead for blob fields and
is faster to encode/decode.

---

### P2P-2 — P2P connector sleeps 100 ms between retries; 4 attempts = 400 ms latency on first connect
**File:** `crates/copypaste-p2p/src/transport.rs:68`  
**Impact:** Low | **Effort:** S

`CONNECT_RETRY_DELAY = 100 ms`, `MAX_CONNECT_ATTEMPTS = 4`. When a peer is
available but the mDNS-announce arrives before the listener is bound (common on
startup), the connector stalls for up to 400 ms before succeeding. The delay is
not exponential — all 4 attempts fire at 100 ms spacing.

**Recommendation:** Use an immediate first attempt, then exponential back-off
starting at 50 ms (50, 100, 200 ms) to recover quickly from the brief race window
while not wasting attempts on genuinely offline peers.

---

## 8. Relay Server

### RELAY-1 — `governor` rate-limiter uses a DashMap-keyed by IP; no TTL eviction during steady state
**File:** `crates/copypaste-relay/src/governor_cleanup.rs`  
**Impact:** Low | **Effort:** S

The governor cleanup task periodically evicts stale rate-limit entries. The
eviction interval and the governor's in-memory footprint grow linearly with the
number of distinct IPs that ever contacted the relay. Under a burst from many
sources the DashMap holds entries indefinitely until the next cleanup tick.

**Recommendation:** Confirm the cleanup interval is tuned to the expected IPs/min
rate. The current implementation is noted as existing already; this is a reminder
to verify the eviction cadence matches the burst pattern documented in
`relay-v2-quotas-plan.md`.

---

---

## Top 10 Performance Wins (Ranked)

| Rank | ID | Title | Impact | Effort |
|------|----|-------|--------|--------|
| 1 | DB-6 / CONC-1 | Wire `SqlitePool` for read-only IPC; eliminate global write mutex serialisation of reads | High | L |
| 2 | CRYPTO-3 | Cache `peers.json` parse in `SyncCrypto`; stop one disk-read per clipboard item | High | S |
| 3 | DB-1 | Switch `get_page`, `get_page_pinned_first`, `get_page_meta` to `prepare_cached` | High | S |
| 4 | DB-2 | Replace per-row FTS DELETE loops with single-query bulk DELETE in prune ops | High | M |
| 5 | SYNC-1 | Skip `rekey_outbound` when no P2P peer is connected | Medium | S |
| 6 | SYNC-5 | Batch incoming WireItems before calling `merge_incoming_with_crypto` | Medium | M |
| 7 | CLIP-1 | Move changeCount check outside `autoreleasepool`; avoid UTI NSString allocs on idle | Medium | S |
| 8 | CRYPTO-1 | Cache `derive_v2` per ingest session; avoid 500× HKDF-SHA512 during cloud backfill | Medium | S |
| 9 | DB-5 | Add partial index `ON clipboard_items(wall_time DESC) WHERE deleted = 0` for live-page query | Medium | S |
| 10 | CONC-3 / MEM-2 | Broadcast only `item_id` token; subscribers fetch content on demand; remove blob from channel | Medium | S |

---

*This document covers only performance opportunities. Correctness and security findings are tracked separately in `docs/audit/`.*
