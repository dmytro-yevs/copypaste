# CopyPaste Relay API

Base URL: `http://localhost:8080` (default; operator-configurable via `RELAY_ADDR`)

**Authentication:** `Authorization: Bearer <auth_token>` where `auth_token` is
the **opaque random 32-hex-character token** returned by `POST /devices` at
registration. The token is 16 bytes of `OsRng` entropy encoded as hex — it is
**not** derived from the public key or any other secret. Tokens are compared
constant-time via `subtle::ct_eq` (no timing oracle).

**Rate limiting:**
- Per-IP: 200 req/min (burst 60). Applies to all non-exempt routes.
- Per-device: 60 req/min (burst 20). Applies to device-scoped item routes.
- Registration: 5 attempts per (IP, device_id) per 60 s.

Exceeding either limit returns `429 Too Many Requests` with a `Retry-After` header.

---

## POST /devices

Register (or co-register) a device and obtain a bearer token.

**Unauthenticated.**

**Request body:**
```json
{
  "device_id": "<uuid-v4>",
  "device_name": "<1-64 character human-readable name>",
  "public_key_b64": "<base64-standard-encoded X25519 public key (must decode to 32 bytes)>",
  "pop_b64": "<base64-standard-encoded HMAC-SHA256(sync_key, 'relay-registration-pop-v1:' || device_id)>"
}
```

`pop_b64` is required. The relay verifies it to confirm the registrant holds
the sync key the `device_id` was derived from.

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
tokens valid. This allows multiple devices on one account to share a single
inbox derived from the shared sync key.

**Errors:**
- `400` — invalid UUID, invalid base64, key not 32 bytes, blank name, missing `pop_b64`
- `403` — free-tier per-IP device quota (5 new device records) exhausted
- `429` — registration rate limit (5 attempts / 60 s per IP+device pair)

---

## GET /devices

List registered device IDs.

**Requires bearer authentication.** Returns only opaque device IDs — no tokens,
keys, or names. A valid `Authorization: Bearer <auth_token>` for any registered
device is required; an invalid/absent token returns `401 Unauthorized`. This
closes the unauthenticated inbox-enumeration vector (finding P2-relay / `7185`).

**Response 200:**
```json
{
  "devices": ["<uuid>", "<uuid>", ...]
}
```

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
  "registered_at": "<RFC-3339>",
  "expires_at": "<RFC-3339>"
}
```

**Errors:** `404` — device not found.

---

## POST /devices/{device_id}/items

Push an encrypted clipboard item into the device's inbox.

**Requires bearer authentication.**

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
| `content_b64` | string | Base64-standard-encoded encrypted payload (XChaCha20-Poly1305 ciphertext + nonce, opaque to the relay) |
| `wall_time` | u64 | Sender wall-clock time, Unix epoch **milliseconds** |

Body size limit: `RELAY_MAX_ITEM_BYTES` × 4/3 + 1 KiB (default ≈ 13.3 MiB encoded).

**Response 201:**
```json
{ "id": 42 }
```

`id` is the auto-assigned relay-internal integer for the stored item; used as
the `since_id` cursor in pull requests.

**Errors:**
- `401` — missing or invalid bearer token
- `403` — target device inbox quota exceeded (500 items hard cap; oldest are
  pruned silently — the sender is not notified)
- `404` — target device not found
- `413` — payload too large
- `429` — per-device or per-IP rate limit

---

## GET /devices/{device_id}/items

Poll for new items in a device's inbox. Supports cursor-based pagination.

**Requires bearer authentication.**

**Query parameters:**

| Param | Type | Default | Notes |
|---|---|---|---|
| `since` | u64 | `0` | Return items with `wall_time > since` (milliseconds) |
| `since_id` | i64 | absent | Composite-cursor companion to `since`; when provided, returns items where `(wall_time, id) > (since, since_id)`. Use to paginate without duplicates when multiple items share the same `wall_time`. |
| `limit` | usize | 200 | Maximum items to return; capped at 500. |

**Response 200:**
```json
{
  "items": [
    {
      "id": 42,
      "content_type": "text",
      "content_b64": "<base64 ciphertext>",
      "wall_time": 1718000000000
    }
  ]
}
```

Paginate by passing the last returned `(wall_time, id)` back as `since` and
`since_id`. An empty `items` array means the inbox is fully consumed up to the
given cursor.

**Response budget:** the relay caps total `content_b64` bytes cloned per
response at 128 MiB (across all items in the page) to bound lock-hold time.

**Errors:** `401`, `404`.

---

## DELETE /devices/{device_id}/items/{item_id}

Remove a specific item from a device's inbox.

**Requires bearer authentication.** `item_id` here is the integer `id` returned
by push or pull.

**Response 204:** No content.

**Errors:** `401`, `404` (device or item not found).

---

## GET /devices/{device_id}/subscribe

Server-Sent Events (SSE) stream of new inbox items in real time.

**Requires bearer authentication.**

Opens a persistent HTTP connection. The relay emits an SSE event for each new
item pushed to the device's inbox. Also drains the existing inbox from the last
known cursor at connection time. Keepalive comment frames are sent every 25 s.

**Event format (each SSE `data` field):**
```json
{
  "id": 42,
  "content_type": "text",
  "content_b64": "<base64 ciphertext>",
  "wall_time": 1718000000000
}
```

**Errors:** `401`, `404`.

---

## GET /health

Liveness check. No authentication required. No device or item counts are
included (operational data should not be exposed to unauthenticated callers).

**Response 200:**
```json
{ "status": "ok" }
```

---

## GET /stats

Protocol version probe. No authentication required.

**Response 200:**
```json
{ "version": "2" }
```

---

## GET /metrics

Prometheus-compatible metrics endpoint. No authentication required. Exempt from
rate limiting so Prometheus scrapers do not share the per-IP budget.

---

## Notes

- The relay stores **only ciphertext** — it has no decryption keys and cannot
  read clipboard content.
- Items are end-to-end encrypted (XChaCha20-Poly1305); nonces are embedded in
  `content_b64` (opaque to the relay).
- Per-device inbox quota: 500 items hard cap; oldest pruned silently on overflow.
- Item TTL: items expire after `sync_ttl_secs` (default 86 400 s = 24 h).
- Sensitive items **do** sync through the relay (encrypted). Sensitivity is
  re-evaluated by the receiving daemon from decrypted plaintext.
- The relay persists device records, tokens, and inbox items to a local SQLite
  database (plain SQLite — no SQLCipher, ciphertext-only) so state survives
  restarts.
