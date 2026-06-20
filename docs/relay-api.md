# CopyPaste Relay API

Base URL: `http://localhost:8080` (default; operator-configurable via `RELAY_PORT` and `RELAY_BIND_ADDR`)

**Authentication:** `Authorization: Bearer <auth_token>` where `auth_token` is
the **opaque random 32-hex-character token** returned by `POST /devices` at
registration. The token is 16 bytes of `OsRng` entropy encoded as hex — it is
**not** derived from the public key or any other secret. Tokens are compared
constant-time via `subtle::ConstantTimeEq` (no timing oracle). The `Bearer`
scheme prefix is matched case-insensitively (RFC 6750 §2.1).

**Token lifetime:** tokens expire 1 year after issuance. A single device record
may hold up to 16 independently-issued tokens concurrently; expired tokens are
pruned automatically before new ones are added, and the oldest live token is
evicted when the 16-token cap is reached (FIFO).

**Rate limiting:**
- Per-IP: 3 req/s steady-state, burst 60 (≥200 req/min). Applied to all
  non-exempt routes. By default the key is the TCP peer IP (spoofing-resistant).
  Set `RELAY_TRUST_PROXY_HEADERS=1` to honor `X-Forwarded-For`/`X-Real-IP`/
  `Forwarded` headers instead — only safe when a trusted reverse proxy is in
  front.
- Per-device: 1 req/s steady-state, burst 20 (60 req/min). Applied to
  device-scoped item routes only, keyed by the `:device_id` URL segment.
- Registration: 5 attempts per (IP, device_id) per 60 s, enforced inside the
  handler (separate from the governor layer).

Exceeding either limit returns `429 Too Many Requests` with a `Retry-After`
header (in seconds).

**Concurrency cap:** `RELAY_MAX_CONNECTIONS` (default 1024) limits simultaneous
in-flight requests via a `tower` concurrency layer; excess requests queue
(back-pressure) rather than being dropped.

**Exempt routes** (`/health`, `/stats`, `/metrics`) — no rate limiting applied.

---

## POST /devices

Register (or co-register) a device and obtain a bearer token.

**Unauthenticated.**

**Request body:**
```json
{
  "device_id": "<uuid-v4>",
  "device_name": "<1-64 character human-readable name>",
  "public_key_b64": "<base64-standard-encoded X25519 public key (must decode to exactly 32 bytes)>",
  "pop_b64": "<base64-standard-encoded HMAC-SHA256(sync_key, 'relay-registration-pop-v1:' || device_id)>"
}
```

`public_key_b64` also accepts the legacy alias `public_key`.

`pop_b64` is **required**. The relay verifies it to confirm the registrant holds
the sync key the `device_id` was derived from. Registration is rejected when
this field is absent, malformed, or (on co-registration) does not match the
stored PoP.

**Response 201:**
```json
{
  "device_id": "<uuid-v4>",
  "auth_token": "<32 hex chars — random, opaque>",
  "expires_at": "<RFC-3339 UTC timestamp — 1 year from registration>"
}
```

**Co-registration (shared-account inbox):** if the `device_id` is already
registered, the relay mints a fresh independent token and keeps all existing
tokens valid. This lets all devices on one account co-register the same
account-inbox `device_id` (derived via HKDF of the shared sync key, never sent
in cleartext) so they can all push to and read from a single shared inbox.

**Per-source device quota:** on first registration of a `device_id`, the relay
counts the number of unique device records registered from the same source IP.
Free tier: 5 new device records per source IP. Co-registrations (same
`device_id`, different token) do **not** count against this limit.

**Errors:**
- `400` — invalid UUID, invalid base64, key not 32 bytes, blank name, `device_name` > 64 chars, missing `pop_b64`
- `403` — per-source-IP new-device quota exhausted (5 unique devices per IP, free tier)
- `429` — registration rate limit (5 attempts per (IP, device_id) per 60 s)

---

## GET /devices

List registered device IDs.

**Requires bearer authentication.** The token is verified against all registered
devices; any valid token from any registered device is accepted. Returns only
opaque device IDs — no tokens, keys, or names are included (tokens would allow
hijacking; other fields are accessible via `GET /devices/{device_id}`).

**Response 200:**
```json
{
  "devices": ["<uuid>", "<uuid>", ...]
}
```

**Errors:** `401` — missing or invalid bearer token.

---

## GET /devices/{device_id}

Retrieve public information about a registered device.

**Unauthenticated.**

**Response 200:**
```json
{
  "device_id": "<uuid-v4>",
  "device_name": "<string>",
  "public_key_b64": "<base64>",
  "registered_at": "<RFC-3339 UTC>",
  "expires_at": "<RFC-3339 UTC — latest expiry across all co-registered tokens>"
}
```

**Errors:** `404` — device not found.

---

## POST /devices/{device_id}/items

Push an encrypted clipboard item into the device's inbox.

**Requires bearer authentication.** The token must belong to `device_id`.
Authentication is verified first (before base64 decoding the payload) to avoid
pre-auth CPU amplification.

**Request body:**
```json
{
  "content_type": "text" | "image" | "file",
  "content_b64": "<base64-standard-encoded ciphertext>",
  "wall_time": 1718000000000
}
```

| Field | Type | Notes |
|---|---|---|
| `content_type` | string | MIME-style type: `"text"`, `"image"`, or `"file"` |
| `content_b64` | string | Base64-standard-encoded XChaCha20-Poly1305 ciphertext (opaque to the relay) |
| `wall_time` | u64 | Sender wall-clock time, Unix epoch **milliseconds** (untrusted; used for cursor-based pull, not TTL) |

**Body size limit:** `RELAY_MAX_ITEM_BYTES` × 4/3 + 1 KiB (default ≈ 13.3 MiB
encoded), enforced by a global body-limit layer before the handler runs.

**Per-tier item-size quotas** (checked after auth, before storing):
- `"text"`: decoded ciphertext ≤ 8 MiB
- `"image"`, `"file"`: decoded ciphertext ≤ 10 MiB

**Inbox overflow:** when a device inbox exceeds the per-device cap (default 500;
configurable via `RELAY_MAX_ITEMS_PER_DEVICE`), the **oldest items are silently
pruned** to make room. The sender is not notified. This is a silent prune, not a
rejection.

**Response 201:**
```json
{ "id": 42 }
```

`id` is the auto-assigned relay-internal integer for the stored item (monotonically
increasing per device); used as the `since_id` cursor in pull requests.

**Errors:**
- `401` — missing or invalid bearer token (also returned if the device was evicted between auth and store)
- `413` — decoded payload exceeds per-tier item-size limit (`ITEM_SIZE_EXCEEDED`) or encoded body exceeds the global body cap (`PAYLOAD_TOO_LARGE`)
- `429` — per-device or per-IP rate limit

---

## GET /devices/{device_id}/items

Poll for new items in a device's inbox. Supports cursor-based pagination.

**Requires bearer authentication.** The token must belong to `device_id`.

**Query parameters:**

| Param | Type | Default | Notes |
|---|---|---|---|
| `since` | u64 | `0` | Return items with `wall_time > since` (milliseconds), or with `(wall_time, id) > (since, since_id)` when `since_id` is also provided |
| `since_id` | i64 | absent | Composite-cursor companion to `since`. When provided, items qualify iff `(wall_time, id) > (since, since_id)` — a strictly monotonic order that avoids duplicate-or-drop when multiple items share the same `wall_time`. Absent → legacy `wall_time`-only floor (backward-compatible). |
| `limit` | usize | 200 | Maximum items to return; capped at 500 server-side regardless of the supplied value. |

**Response 200** — a JSON array of items:
```json
[
  {
    "id": 42,
    "content_type": "text",
    "content_b64": "<base64 ciphertext>",
    "wall_time": 1718000000000
  }
]
```

Paginate by passing the last returned `(wall_time, id)` back as `since` and
`since_id`. An empty array means the inbox is fully consumed up to the given
cursor.

**Response byte budget:** the relay caps total `content_b64` bytes cloned per
response at 128 MiB (across all items in the page) to bound store-mutex hold
time. This limits individual items returned, not their count.

**Errors:** `401`, `404`.

---

## DELETE /devices/{device_id}/items/{item_id}

Remove a specific item from a device's inbox.

**Requires bearer authentication.** The token must belong to `device_id`.
`item_id` is the integer `id` returned by push or pull.

**Response 204:** No content.

**Errors:** `401`, `404` (device or item not found).

---

## GET /devices/{device_id}/subscribe

Server-Sent Events (SSE) stream of new inbox items in real time.

**Requires bearer authentication.** The token must belong to `device_id`.
Auth is verified once at connection time before the stream is opened.

**Query parameters:** same `since` / `since_id` as `GET .../items` — used to
set the initial resume cursor.

**Stream behavior:**

1. **Backfill:** on connect the producer flushes every item already in the inbox
   past the `(since, since_id)` cursor, paging at most 500 items per store-lock
   acquisition until the inbox is drained.
2. **Real-time delivery:** a per-device broadcast wake channel (fired after each
   `POST .../items` write commits) wakes the producer, which re-reads from its
   advancing cursor and emits each new item.

Inbox semantics mirror `GET .../items`: cursor-based, at-least-once, **no
delete-on-read**. An item can therefore arrive over both SSE and a concurrent
poll; clients must dedup by `id`.

**SSE event format** (each SSE event):
```
event: item
id: <item id>
data: {"id":42,"content_type":"text","content_b64":"<base64>","wall_time":1718000000000}
```

- `event: item` — the event name.
- `id: <item id>` — the per-device integer item id; clients may use it as
  `Last-Event-ID`, though the authoritative resume mechanism is the
  `?since=&since_id=` cursor on reconnect.
- `data:` — JSON object identical to one element of the `GET .../items` array.

**Keepalive:** SSE comment frames (`:` lines) are sent every 25 s to survive
idle-timeout proxies and load balancers. The relay monitors for client disconnect
(`tx.closed()`) and tears down the producer task immediately on disconnect,
releasing the broadcast receiver and store reference.

**Errors:** `401`, `404`.

---

## GET /health

Liveness check. No authentication required. Rate-limit exempt.

**Response 200:**
```json
{ "status": "ok" }
```

Device and item counts are intentionally omitted to prevent operational data
leaking to unauthenticated callers.

---

## GET /stats

Protocol version probe. No authentication required. Rate-limit exempt.

**Response 200:**
```json
{ "version": "2" }
```

Device and item counts are intentionally omitted (same rationale as `/health`).

---

## GET /metrics

Prometheus text-format exposition endpoint (`text/plain; version=0.0.4`). No
authentication required. Rate-limit exempt so Prometheus scrapers do not share
the per-IP budget.

**Response 200** (Prometheus text format):
```
# HELP copypaste_relay_up Whether the relay is up (1 = yes)
# TYPE copypaste_relay_up gauge
copypaste_relay_up 1
```

Only the liveness gauge is emitted. Device count, item count, and eviction
counters are not exposed to unauthenticated scrapers.

---

## Operator Configuration

All settings are loaded from environment variables at startup; missing or
unparseable values fall back to defaults.

| Variable | Default | Description |
|---|---|---|
| `RELAY_PORT` | `8080` | TCP port to listen on |
| `RELAY_BIND_ADDR` | `0.0.0.0` | Interface to bind (e.g. `127.0.0.1` for loopback-only behind a proxy) |
| `RELAY_SYNC_TTL_SECS` | `86400` | Item TTL in seconds (24 h); items older than this are pruned by the background evictor |
| `RELAY_MAX_ITEM_BYTES` | `10485760` (10 MiB) | Maximum decoded ciphertext size per item; capped at 100 MiB |
| `RELAY_MAX_ITEMS_PER_DEVICE` | `500` | Per-device inbox cap; must be ≥ 1 |
| `RELAY_TRUST_PROXY_HEADERS` | `false` | Set to `1`/`true`/`on` to key per-IP rate limits on `X-Forwarded-For`/`X-Real-IP`/`Forwarded` |
| `RELAY_DB_PATH` | `:memory:` | SQLite file path for persistence across restarts; defaults to in-memory (ephemeral) |
| `RELAY_MAX_CONNECTIONS` | `1024` | Max concurrent in-flight requests; excess requests queue (back-pressure); must be ≥ 1 |

---

## Notes

- The relay stores **only ciphertext** — it has no decryption keys and cannot
  read clipboard content.
- Items are end-to-end encrypted (XChaCha20-Poly1305); nonces are embedded in
  `content_b64` (opaque to the relay).
- **TTL eviction:** items expire after `RELAY_SYNC_TTL_SECS` (default 86 400 s =
  24 h), measured by server-side insert time — not the (untrusted) sender
  `wall_time`. Expired items are pruned by a background task.
- **Inbox overflow:** when a device inbox exceeds `RELAY_MAX_ITEMS_PER_DEVICE`,
  the oldest items (lowest `wall_time`) are silently pruned on each push.
- **Sensitivity:** sensitive items do sync through the relay (encrypted). The
  receiving daemon re-evaluates sensitivity from decrypted plaintext.
- **Persistence:** the relay stores device records, token sets, and inbox items
  in a plain SQLite database (not SQLCipher — the relay never holds keys). Set
  `RELAY_DB_PATH` to a file path to survive restarts; default `:memory:` is
  ephemeral.
- **Token security:** `Authorization: Bearer` tokens are compared constant-time
  (`subtle::ConstantTimeEq`). The `Bearer` scheme is matched case-insensitively.
  Token expiry is clock-fail-closed (a clock error results in no valid tokens).
