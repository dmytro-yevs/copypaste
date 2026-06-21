# CopyPaste — Sync & Connectivity: Complete Product Inventory

> ⚠️ Snapshot as of 2026-06-04; branch references may be outdated. This inventory was audited
> against branch `v0.6.1-integration`. Gaps listed may have been addressed since.
> Check current code before relying on this inventory.

Branch: `v0.6.1-integration`  
Audit date: 2026-06-04  
Status: READ-ONLY. This document describes behavior as found in code; it does not prescribe future changes.

---

## Table of Contents

1. [Overview — Three Independent Sync Paths](#1-overview--three-independent-sync-paths)
2. [P2P Sync (LAN / mTLS)](#2-p2p-sync-lan--mtls)
3. [Relay Sync (Store-and-Forward HTTP)](#3-relay-sync-store-and-forward-http)
4. [Supabase Cloud Sync](#4-supabase-cloud-sync)
5. [Pairing Flows](#5-pairing-flows)
6. [Op Propagation — Delete / Pin / Reorder / Clear](#6-op-propagation--delete--pin--reorder--clear)
7. [CRDT / LWW Merge Protocol](#7-crdt--lww-merge-protocol)
8. [Cross-Device Encryption (Shared Sync Key)](#8-cross-device-encryption-shared-sync-key)
9. [Notable Gaps Observed](#9-notable-gaps-observed)

---

## 1. Overview — Three Independent Sync Paths

CopyPaste ships three sync transports. They operate in parallel and are fully independent: enabling one does not disable or replace another. LWW merge deduplicates items that arrive on multiple paths simultaneously.

| Path | Transport | Pairing required | Encrypted in transit | Works without internet |
|------|-----------|-----------------|---------------------|----------------------|
| P2P | mTLS TCP over LAN | Yes (PAKE + mDNS) | Yes (TLS 1.3) | Yes |
| Relay | HTTPS store-and-forward | Yes (shares sync key) | Yes (HTTPS + AEAD payload) | No |
| Supabase cloud | HTTPS REST + WebSocket | Supabase account | Yes (AEAD payload) | No |

---

## 2. P2P Sync (LAN / mTLS)

### 2.1 What it does

Each daemon listens on a TCP port and advertises itself over mDNS-SD under the service type `_copypaste._tcp.local.` (`crates/copypaste-p2p/src/discovery.rs:33`). When a known paired peer is discovered, the daemon dials it and performs a full HELLO/HAVE/WANT/ITEMS/DONE item-exchange session over mutual TLS (`crates/copypaste-sync/src/engine.rs`). Both sides present and verify ECDSA P-256 self-signed certificates; the cert fingerprint (SHA-256(DER), lowercase hex) is the stable device identity.

### 2.2 Discovery

The mDNS TXT record advertises:
- `v` = protocol version (`"2"` current; `"1"` legacy, accepted for backward compat)
- `did` = device cert fingerprint (hex)
- `name` = human-readable device name
- `bport` = TCP port of the PAKE bootstrap listener (v2 only; absent on v1 peers disables the Pair button in UI)

The discovery service enforces a hard cap of 256 simultaneous known peers and a per-peer-ID rate limiter (`crates/copypaste-p2p/src/discovery.rs:66,118`). Addresses are sorted IPv4-first, deduplicated, and filtered of `0.0.0.0` / link-scoped unknowns (`discovery.rs:574-590`).

### 2.3 mTLS handshake

Both ends use `ClientAuth::Required`. A custom `PeerCertVerifier` (not the system CA store) checks the presented cert fingerprint against the `PairedPeers` in-memory allowlist (`crates/copypaste-p2p/src/transport.rs:362-464`). An unknown fingerprint causes an immediate `UnknownPeer` error; no data is exchanged.

Timeouts:
- TCP connect: 5 s (`TCP_CONNECT_TIMEOUT`), classified as transient → retried
- TLS handshake: 10 s (`TLS_HANDSHAKE_TIMEOUT`), classified as permanent → not retried
- Up to 4 total attempts with ±50 ms jittered 100 ms backoff between transient failures (`transport.rs:62-69`)

TCP keepalive is enabled: idle→probe after 20 s, probe interval 10 s (`transport.rs:92-95`). This detects a peer that disappears without sending FIN (Wi-Fi drop, kill signal).

### 2.4 Cert / identity persistence

The self-signed cert is generated once and persisted to `p2p_identity.json` in the app data directory (`crates/copypaste-p2p/src/cert.rs:106-153`). File mode is `0600`. On each daemon launch `load_or_create` reloads the cert; a fingerprint mismatch (tamper/corruption) is a hard error. This means the identity is stable across restarts — paired devices do not need to re-pair after a daemon restart.

Cert rotation (e.g. manual key refresh) uses a 60-second grace window (`CERT_ROTATION_GRACE = 60 s`, `transport.rs:128`) during which both the old and new fingerprint are accepted, preventing in-flight handshake failures.

### 2.5 Sync session protocol

The sync engine (`crates/copypaste-sync/src/engine.rs`) drives a symmetric 5-step handshake:

```
A --HELLO(device_id, clock, item_count)----> B
A <---HELLO(device_id, clock, item_count)--- B
A --HAVE[(item_id, lamport_ts)...]---------> B
A <---HAVE[(item_id, lamport_ts)...]-------- B
A --WANT[item_ids where peer is newer]-----> B
A <---WANT[item_ids where peer is newer]---- B
A --ITEMS[WireItem...]--------------------> B
A <---ITEMS[WireItem...]------------------- B
A --DONE----------------------------------> B
A <---DONE--------------------------------- B
```

Frames are length-prefixed (4-byte LE `u32`) JSON. Maximum frame size: 16 MiB (`engine.rs:38`). HAVE/WANT keying uses the stable cross-device `item_id` (not the per-row `id` which is a fresh UUID on each device).

On connect, a catch-up replay sends the full local history to the newly connected peer (`crates/copypaste-daemon/src/sync_orch.rs:671-725`). This is paginated at 500 rows per page. Incoming items are merged via LWW (see Section 7).

### 2.6 Online indicator

A peer is "online" iff its cert fingerprint has a live entry in the `LivePeerSinks` map (`crates/copypaste-daemon/src/p2p.rs:71`). The IPC `list_peers` handler reads this map as the authoritative online flag; `last_sync_at` is used as a fallback only when P2P is disabled.

### 2.7 User experience

- P2P sync fires automatically once two paired devices are on the same LAN.
- No user action is required after initial pairing.
- The daemon must be running (`COPYPASTE_P2P=1` environment var or equivalent config).
- Items copied on device A appear on device B typically within 1-2 seconds on a local network.

### 2.8 Limitations / failure modes

- **Re-pair required if `p2p_identity.json` is deleted.** The fingerprint changes; old peers reject the new cert. The 60 s grace window does not help for a full file deletion.
- **mDNS is LAN-only.** Devices on separate network segments (different subnets, VPNs) will not discover each other. The relay path serves that case.
- **Single active pairing at a time** (`pairing_sm.rs:32`). A second `pair_with_discovered` while one is already in flight is rejected.
- **v1 peers (no `bport`)** appear in the discovered list but the Pair button is disabled because the bootstrap port is missing (`discovery.rs:50-55,84`).
- **Multi-homed hosts**: the discovery service filters to usable LAN interfaces and skips loopback/virtual/down NICs (`discovery.rs:386-407`). Address selection may still pick the wrong interface on hosts with multiple active LAN interfaces (known HW-B2 root cause — documented in memory).

---

## 3. Relay Sync (Store-and-Forward HTTP)

### 3.1 What it does

The relay is a standalone Axum HTTP server (`crates/copypaste-relay/`) that acts as a shared encrypted inbox. All paired devices share one inbox ID derived deterministically from the shared sync key via HKDF (`derive_relay_inbox_id`). The relay never sees plaintext; it stores and forwards opaque ciphertext blobs.

Daemon-side relay client: `crates/copypaste-daemon/src/relay.rs`.

### 3.2 Pipeline

**Registration**: `POST {relay_url}/devices` with `{device_id, device_name, public_key_b64}` → `201 {auth_token}`. The token is cached in a `0600` file (`relay_token`). On 401, the token is dropped and re-registration is attempted on the next tick (`relay.rs:806`).

**Push loop**: subscribes to the `new_item_tx` broadcast channel. For each local item:
1. Decrypt the at-rest ciphertext with the local key
2. Re-encrypt under the shared sync key (`encrypt_for_cloud`)
3. Wrap in a `RelayEnvelope` (`{item_id, lamport_ts, ct_b64}`) → base64 → `POST /devices/{inbox_id}/items`

**Receive loop**: polls `GET /devices/{inbox_id}/items?since=&since_id=` every 5 seconds (`POLL_INTERVAL`, `relay.rs:72`). When a full batch of 50 items (`PULL_LIMIT`) comes back, re-polls immediately without waiting (burst drain). Items are decrypted, LWW-merged, and inserted via `sync_common::build_local_item` + `copypaste_core::insert_item`.

**Self-echo dedup**: because every device pushes to and pulls from the same inbox, a device's own push comes back on the next pull. The LWW guard on `item_id` with equal `lamport_ts` makes that a no-op (`relay.rs:33-35`).

### 3.3 Relay server defaults

| Parameter | Default | Env var override |
|-----------|---------|-----------------|
| Port | 8080 | `RELAY_PORT` |
| Item TTL | 86400 s (24 h) | `RELAY_SYNC_TTL_SECS` |
| Max item size | 10 MiB | `RELAY_MAX_ITEM_BYTES` |
| Max items per device | 500 | `RELAY_MAX_ITEMS_PER_DEVICE` |
| Persistence | in-memory (`:memory:`) | `RELAY_DB_PATH` (SQLite path) |

The relay also supports SSE for push-style delivery (tested but polling is the primary production path). A background TTL evictor runs every 60 s.

### 3.4 Inbox ID security

The inbox ID is HKDF-derived from the sync key — it is considered a secret and is never logged. The auth token is also never logged and is stored `0600`. If the sync key is rotated (`rotate_sync_key` IPC), the inbox ID diverges; the old inbox becomes unreachable and a new one starts fresh (`ipc.rs:4107-4138`).

### 3.5 User experience

- Relay sync is active whenever `config.relay_url` is set (via UI or `copypaste cloud setup`).
- Cross-device latency is bounded by the 5 s poll interval (≤5 s typical; longer only if the relay is unavailable).
- Works over the internet (not LAN-constrained).
- Requires the relay server to be reachable.

### 3.6 Limitations / failure modes

- **Poll-based (not push on iOS/Android background)**: Android uses the same 5 s poll interval but there is no WebSocket to the relay; background polling depends on WorkManager scheduling.
- **Inbox diverges after sync-key rotation.** Devices that have not yet fetched the new key will not find items in the new inbox until they re-pair or re-fetch the key.
- **Relay server availability**: if the relay goes down, items queue in the push retry queue (capped at 1024 entries, `relay.rs` imports `sync_common`) and are delivered when the relay comes back. Items older than the TTL (default 24 h) are evicted server-side and lost.
- **File sync ceiling**: blobs larger than 8 MiB (`SYNC_MAX_BLOB_BYTES`, `sync_orch.rs:763`) are stored locally but dropped from sync (warned in logs). The relay's own per-item cap is 10 MiB of encoded content.

---

## 4. Supabase Cloud Sync

### 4.1 What it does

Cloud sync uses Supabase (PostgreSQL + PostgREST + Realtime WebSocket) as a shared cloud database. Items are encrypted client-side; Supabase stores only ciphertext. This path requires a Supabase account and row-level security policies enforced by GoTrue JWT.

Daemon side: `crates/copypaste-daemon/src/cloud.rs`. Supabase SDK: `crates/copypaste-supabase/`.

### 4.2 Auth model

- **SUPABASE_URL** and **SUPABASE_ANON_KEY** must be set (env vars or persisted `config.json`).
- **SUPABASE_EMAIL** / **SUPABASE_PASSWORD** enable GoTrue email+password sign-in for the `authenticated` RLS scope. Without these, anon-key requests are rejected by the project's RLS policies (`cloud.rs:21-22`).
- Auth fail-closed: if email/password sign-in fails, cloud sync aborts entirely rather than falling back to the anon key (`CloudError::AuthFailed`).
- HTTPS-only: `SUPABASE_URL` must use `https://` or cloud sync is refused (`CloudError::InsecureUrl`).

### 4.3 Push loop

Subscribes to `new_item_tx` broadcast. Each item is:
1. Decrypted with the local key.
2. Re-encrypted under the shared sync key using `encrypt_for_cloud` (XChaCha20-Poly1305, AAD bound to `item_id`).
3. POSTed to `POST /rest/v1/clipboard_items`.

Push retries with exponential backoff: 1 s initial, 30 s max (`PUSH_INITIAL_BACKOFF`, `PUSH_MAX_BACKOFF`). The retry queue holds up to 1024 items (`PUSH_RETRY_QUEUE_CAP`); older entries are dropped when full.

### 4.4 Realtime WebSocket + HTTP poll fallback

A `RealtimeClient` (Phoenix Channel protocol over WebSocket) subscribes to `INSERT` events on the `clipboard_items` table. When the WebSocket is connected and the channel join is confirmed, the HTTP fallback poll runs at 60 s (`POLL_INTERVAL_WS_CONNECTED`). When disconnected, HTTP polling runs at 10 s (`POLL_INTERVAL_WS_FALLBACK`). Batch size: 20 rows per tick; a full batch triggers an immediate re-poll (burst drain).

### 4.5 User experience

- Configured via `copypaste cloud setup` CLI or UI settings.
- Syncs across all devices sharing the same Supabase account.
- Real-time push delivery via WebSocket when connected; 10 s poll fallback.
- Items are deleted server-side only when the daemon explicitly removes them (not by TTL).

### 4.6 Limitations / failure modes

- **Requires a Supabase project.** Self-hosted Supabase or the managed cloud is acceptable; project setup is manual.
- **Account-level isolation via RLS.** Multiple users cannot share the same Supabase account without seeing each other's (encrypted) rows. The encryption means data is not readable, but metadata (row count, timing) is visible at the DB level.
- **No WebSocket on Android background** (known from memory: `project_v06_sync_findings.md`). Android falls back to the 60/10 s HTTP poll; there is no reactive push while the app is backgrounded.
- **Storage cap**: `prune_to_cap` is called after each merge to enforce `storage_quota_bytes`. Supabase itself has no automatic row TTL unless configured separately.

---

## 5. Pairing Flows

### 5.1 Discovery Pair (LAN / SAS, "Pair" button)

**How it works:**

The initiator device sees a discovered peer in the UI (sourced from `DiscoveryService::peers()`). The user taps "Pair". The initiator calls `pair_with_discovered` IPC, which:

1. Looks up the peer's `bport` from the mDNS-discovered `PeerInfo`.
2. Dials the peer's TCP bootstrap port over an ephemeral TLS connection.
3. Runs a 3-message OPAQUE PAKE handshake (`crates/copypaste-p2p/src/pake.rs`). On the discovery path, the PAKE password is an **ephemeral random string** sent in-clear inside the bootstrap TLS channel — authentication is NOT provided by the PAKE password here. Authentication is entirely provided by the human SAS comparison.
4. Derives the channel-bound SAS: `HKDF(bound_key, "copypaste/p2p/sas/v1")` → 4 bytes → `% 1_000_000` → 6 decimal digits (`pake.rs:254-260`).
5. The daemon advances to state `AwaitingSas` and presents the 6-digit SAS via `pair_get_sas` IPC.

**User experience:**

Both devices show the same 6-digit number. The user must verify they match on both screens, then tap "Confirm" on both devices within 60 seconds (`SAS_CONFIRM_TIMEOUT`, `pairing_sm.rs:76`). If either side rejects or times out, the keys are zeroized and nothing is persisted.

**On successful confirmation:**

- Both sides exchange a metadata frame carrying `device_model`, `os_version`, and `app_version` AFTER the SAS confirmation step (`ipc.rs:2022`).
- Peer is written to `peers.json` with the cert fingerprint, address, and shared sync key.
- The peer's fingerprint is added to the `PairedPeers` live allowlist.
- P2P sync starts immediately for the new peer.

**MitM resistance:**

The SAS derives from the post-PAKE, post-TLS-channel-binding `bound_key`. An active MitM that intercepts and relays the PAKE messages over separate TLS sessions gets different channel binders per leg → different bound keys per leg → different SAS per leg → the human sees a mismatch (`pake.rs:130-176`).

**Limitations:**

- Only one pairing can be in flight at a time (`pairing_sm.rs:32`).
- `peer_model` and `peer_os` are NOT available at `pair_get_sas` time; they arrive only in the final `pair_with_discovered` response post-confirm (`ipc.rs:2022`). The SAS UI cannot show the peer's device model while awaiting confirmation.
- The responder path shows empty `ip_addrs` and `fingerprint = None` at SAS time because the inbound connection arrives without prior mDNS context (`pairing_sm.rs:43-64`).
- v1 peers (no `bport`) cannot be paired via this flow.

### 5.2 QR Pair (non-SAS token path)

**How it works:**

The displaying device calls `pair_generate_qr` IPC. The daemon:
1. Generates a `PairingToken` — 32 bytes from the OS CSPRNG (256 bits entropy, `pairing_qr.rs:157-163`).
2. Derives the PAKE password from the token: `base64url(token_bytes)`.
3. Builds a `PairingPayload` and encodes it as a CPPAIR2 string (current) or CPPAIR1 (legacy, still accepted):
   ```
   CPPAIR2.<fp_b64url43>.<token_b64url43>.<device_id_b64url22>.<name_b64url>.<host:port>
   ```
4. Wraps it in a deep-link URI: `cppair://pair?p=<percent-encoded CPPAIR2...>`.
5. Returns the QR string + expiry. The token is stored in `pending_qr_token` with a TTL.

**The QR carries:**

- The displaying device's **cert fingerprint** (32 bytes base64url-encoded in v2, or hex in v1) — the scanner pins this peer.
- A **single-use, high-entropy pairing token** — fed into the PAKE handshake as the shared secret. This replaces the 6-character typed password with 256 bits of entropy.
- The displaying device's **UUID** (`device_id`).
- The displaying device's **human-readable name**.
- An optional **discovery hint** (`host:port`) — used when mDNS discovery is unreliable.

The QR does NOT carry: any private key, the sync key, a PasswordFile, or any long-term secret (`pairing_qr.rs:7-35`).

**On scan:**

The scanning device calls `pair_accept_qr` IPC with the scanned string + the first PAKE message. The daemon on the displaying side:
1. Decodes the QR payload, verifies the token matches the stored `pending_qr_token`.
2. Runs `PakeResponder::respond` (step 2) + `PakeResponder::finish` (step 4) using the token as the PAKE password.
3. Returns the PAKE message chain.

No SAS confirmation step. The high-entropy token provides authentication without human comparison.

**Anti-downgrade:**

Unknown version strings (`CPPAIR0`, etc.) are hard-rejected at decode time (`pairing_qr.rs:310-314`). This prevents a tampered "no-token" QR from bypassing the PAKE.

**Limitations:**

- The QR token has a TTL (the daemon evicts stale sessions). If the QR expires before scanning, re-generate.
- The `addr_hint` (`host:port`) is optional; if absent and mDNS discovery fails, the scanner cannot locate the displayer (`discovery.rs:338-351`).
- The QR must be scanned by the intended device only. Anyone who reads the QR can initiate pairing as the scanning side.

### 5.3 Mutual Unpair

`unpair_peer` IPC:
1. Removes the peer from `peers.json`.
2. Evicts the peer's fingerprint from the live `PairedPeers` allowlist — future mTLS handshakes from that peer are rejected immediately without a daemon restart.
3. If the peer is currently connected (entry in `LivePeerSinks`), sends a `ControlMsg::Unpair` frame over the live mTLS channel (`ipc.rs:812-856`).

The peer receiving the `ControlMsg::Unpair` frame also removes the sender using the **mTLS-authenticated fingerprint** of the connection, never a field inside the message (to prevent spoofed evictions, `protocol.rs:179-184`). Old peers that predate mutual unpair will log a warning and ignore the frame; the local eviction still takes effect.

### 5.4 Revoke

`revoke_peer` IPC performs the same P2P allowlist eviction as unpair, plus writes an audit log entry to the `revoked_devices` table (`ipc.rs:5074`). `revoke_all_peers` evicts all entries atomically. The `remove` call on `PairedPeers` evicts both the active fingerprint and any still-graced superseded fingerprints (`transport.rs:301-306`).

---

## 6. Op Propagation — Delete / Pin / Reorder / Clear

### 6.1 Delete (single item)

`delete_item` IPC calls `soft_delete_and_broadcast` (`ipc.rs:2711-2743`):
1. Wipes `content`, `content_nonce`, and `thumb` in the DB row.
2. Sets `deleted = 1` on the row.
3. Bumps `lamport_ts` (new local tick) and `wall_time` (now_ms) so the tombstone is causally after any prior version.
4. Broadcasts the tombstone row via `new_item_tx`.

The sync orchestrator's outbound path forwards the tombstone `WireItem` (`deleted = true`, `content = None`) to all connected peers. On the receiving peer, if the tombstone's `lamport_ts` is higher than the local copy (LWW `TakeRemote`), `soft_delete_item` is called on the local row (`sync_orch.rs:379-404`).

**Propagation converges**: a tombstone wins over any live version with a lower Lamport timestamp. A live item with a higher Lamport timestamp keeps its version (LWW keeps local).

### 6.2 Pin / Unpin

`pin_item` and `unpin_item` IPC update the row's `pinned` flag (and optionally `pin_order`), bump `lamport_ts`, and broadcast via `new_item_tx`. The `WireItem` now carries `pinned` and `pin_order` fields with `#[serde(default)]` for backward compatibility (`protocol.rs:128-135`). On `TakeRemote`, `wire_to_local` propagates these fields directly (`merge.rs:99-101`). A remote unpin on a locally-pinned row converges correctly: the remote wins LWW when its `lamport_ts` is higher.

### 6.3 Reorder (pin_order)

`pin_order` is a `f64` sort key among pinned items. When the user reorders pins on one device, the new `pin_order` values are stamped + broadcast. On the receiving peer, LWW `TakeRemote` replaces the local `pin_order` with the wire value.

### 6.4 Clear-All

`delete_all` IPC (`ipc.rs:3005-3037`) executes a `DELETE FROM clipboard_items WHERE pinned = 0` SQL statement locally. **It does not broadcast tombstones**. Pinned items survive. This operation does NOT propagate to other devices via sync; other devices retain their non-pinned items until their own `delete_all` is issued.

### 6.5 What converges and what does not

| Operation | Propagates via sync | Convergent |
|-----------|--------------------|-----------:|
| Copy (new item) | Yes — P2P + relay + Supabase | Yes |
| Delete single item | Yes — soft-delete tombstone | Yes (LWW) |
| Pin / unpin | Yes — `pinned` field on WireItem | Yes (LWW) |
| Reorder pins | Yes — `pin_order` field on WireItem | Yes (LWW) |
| Clear-All (delete_all) | **No** — local SQL only, no broadcast | No |
| Image thumbnail (`thumb`) | **No** — thumb is local-only, not on WireItem | N/A (local) |

---

## 7. CRDT / LWW Merge Protocol

### 7.1 Identity

Items use a stable cross-device `item_id` (UUID) as the CRDT identity. The per-row `id` is a fresh `Uuid::new_v4()` on every device and is never used for identity matching. HAVE/WANT exchange keying on `item_id` ensures two devices holding the same logical item converge to one row rather than accumulating duplicates (`engine.rs:209-217`).

### 7.2 Conflict resolution (Last-Write-Wins)

Tiebreak order (decreasing priority):
1. Higher `lamport_ts` wins (causal ordering).
2. Equal Lamport: higher `wall_time` (Unix ms) wins.
3. Equal wall time: lexicographically larger `origin_device_id` wins (deterministic, both sides agree).

(`merge.rs:34-63`)

### 7.3 Clock safety

- Negative `lamport_ts` / `wall_time` from a hostile peer are clamped to 0 before any processing (`engine.rs:389`, `sync_orch.rs:341`).
- Values more than 10^12 ticks ahead of the local clock are clamped to the ceiling, preventing clock-jamming attacks that would make a hostile peer win every future LWW forever (`engine.rs:51-52, 404-425`).
- The post-session peer clock is stored as the local clock's value after all `observe()` calls, not the stale HELLO clock (`engine.rs:471-474`).

### 7.4 Frame limits

- Maximum P2P frame: 16 MiB (`engine.rs:38`, `transport.rs:80`).
- Binary fields (`content`, `content_nonce`) are base64-encoded (not JSON number arrays) to stay within the frame limit (`protocol.rs:27-45`).

---

## 8. Cross-Device Encryption (Shared Sync Key)

### 8.1 The problem

Items are stored at rest encrypted under each device's private local key. Naively forwarding the at-rest ciphertext to a peer would produce an undecryptable row on the peer (different key).

### 8.2 Solution: re-keying through a shared sync key

At pairing time both devices derive a 32-byte shared sync key from the PAKE session key. This key is persisted in `peers.json` as `sync_key_b64`.

**Outbound (send)**: `rekey_outbound` in `sync_orch.rs` (`line 891`):
1. Decrypt the at-rest ciphertext with the local key.
2. Re-encrypt the plaintext under the shared sync key (`encrypt_for_cloud`, XChaCha20-Poly1305, AAD = `item_id`).
3. Place the self-framed blob in `wire.content`; set `wire.content_nonce = None` (the unwrap marker).

If re-keying fails and a shared key is present, the item is **dropped** (not forwarded as undecryptable raw ciphertext) — `RekeyOutcome::Failed` causes a `continue` in the send loop (`sync_orch.rs:194-200`).

**Inbound (receive)**: `rekey_inbound` (`sync_orch.rs:955`):
1. Detect sync-key-wrapped payload: `content_nonce == None`.
2. Decrypt with the shared sync key.
3. Re-encrypt under THIS device's local v2 key + v4 AAD.
4. Index the plaintext into FTS for search/preview.

**Image and file items** go through `rekey_blob_outbound` / `rewrap_inbound_blob`: the chunk-encrypted blob is reassembled to plaintext, then re-wrapped as a single shared-key blob. The `file_name` and `mime` fields are stamped on the wire before `blob_ref` is cleared (`sync_orch.rs:849-858`).

**Sync ceiling**: blobs larger than 8 MiB are dropped with a warning and not forwarded (`SYNC_MAX_BLOB_BYTES = 8 MiB`, `sync_orch.rs:763`).

### 8.3 Key version correctness

`wire_to_local` preserves the sender's `key_version` so the receiver's `decrypt_item_by_version` selects the matching key and AAD. Hard-coding `key_version = 1` on inbound items (the prior bug) caused AuthFailed on every synced item (`merge.rs:86-93`).

---

## 9. Notable Gaps Observed

The following limitations or missing behaviors were observed directly in code. They are facts, not recommendations.

### 9.1 No reactive push on Android (P2P)

Android has no persistent WebSocket or TCP listener for P2P sync. The Android background capture model uses a WorkManager fallback poll. When the Android app is backgrounded, it does not actively discover or connect to peers. The macOS daemon connects to Android once mDNS advertises it, but the reverse (Android→macOS) requires Android's background activity to be alive.

### 9.2 Clear-all does not sync

`delete_all` is a local SQL `DELETE` with no broadcast. Clearing history on one device does not propagate to other devices. Each device must issue its own `delete_all`. (References: `ipc.rs:3005-3037`.)

### 9.3 Image thumbnails are local-only

`thumb` (the precomputed WebP thumbnail blob) is explicitly omitted from `WireItem` (`merge.rs:104-106`). Synced image items arrive without a thumbnail on the receiving device; thumbnails are rebuilt locally on demand. There is no thumbnail backfill mechanism over sync.

### 9.4 File items: auto-apply to NSPasteboard is deferred

When a synced file item arrives and auto-apply is enabled, the `apply_to_pasteboard_if_fresh` function skips files with a `debug!` log: "auto-apply skipped for file item (not yet supported)" (`sync_orch.rs:1268-1271`). Text and images are auto-applied; files are not.

### 9.5 Device model / OS only exchanged post-SAS

On the discovery (SAS) pairing path, `peer_model` and `peer_os` are NOT available during the SAS confirmation step. They arrive only in the final `pair_with_discovered` response after both sides confirm (`ipc.rs:2022,2128-2129`). The UI cannot show the peer's device model on the SAS confirm screen.

### 9.6 Responder has no fingerprint at SAS time

On the responder path (inbound pairing connection), the `PeerSnapshot.fingerprint` is `None` during SAS presentation because the inbound connection arrives before the mTLS fingerprint is surfaced (`pairing_sm.rs:43-64`). The UI renders whatever is populated and silently omits the rest.

### 9.7 Re-pair required if cert file is deleted or fingerprint corrupted

`load_or_create` treats a fingerprint mismatch as a hard error and refuses to load the corrupt identity (`cert.rs:131-143`). If `p2p_identity.json` is lost or corrupted, a new cert is generated, the fingerprint changes, and all existing peers reject connections. Every paired device must re-pair.

### 9.8 Relay inbox diverges on sync-key rotation

After `rotate_sync_key` IPC, the HKDF-derived inbox ID changes. The daemon logs "relay inbox id will diverge and the old inbox is unreachable" (`ipc.rs:4138`). Items in the old inbox that have not yet been polled are permanently lost. Devices that have not yet refreshed their sync key will be unable to find the new inbox.

### 9.9 No conflict resolution UI

There is no UI surface for merge conflicts. LWW is fully automatic and silent. Users cannot see when a remote version wins over a local one, or inspect the merge history.

### 9.10 WorkManager fallback poll path (Android)

The Android side uses WorkManager as a fallback for background sync when the app is not in the foreground. The exact polling cadence is subject to Android OS battery and scheduling constraints; the 5 s relay poll interval is a target, not a guarantee on backgrounded Android.

### 9.11 Single shared sync key for the group

`SyncCrypto::shared_sync_key` returns the first peer record with a valid `sync_key_b64` (`sync_orch.rs:117-133`). With more than two paired devices a common group key would be required; the comment notes "deferred". In practice, pairing device A to device B and device A to device C creates two distinct sync keys; B and C do not share a key with each other and will not sync items between them unless directly paired.

---

*Document generated by static code audit. All file:line citations refer to the `v0.6.1-integration` branch at the time of this audit (2026-06-04).*
