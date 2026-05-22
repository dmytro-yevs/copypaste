# CopyPaste Relay API

Base URL: `http://localhost:8080` (default)

Authentication: `Authorization: Bearer <token>` where token = first 32 hex chars of SHA-256(public_key_bytes).

Rate limiting: 60 req/min per device (burst: 20).

---

## POST /devices

Register a device.

**Request:**
```json
{
  "device_id": "uuid-v4",
  "public_key": "base64-encoded-32-bytes"
}
```

**Response 200:**
```json
{
  "device_id": "uuid-v4",
  "token": "hex32chars"
}
```

---

## POST /devices/:device_id/items

Upload encrypted clipboard item.

**Headers:** `Authorization: Bearer <token>`

**Request:**
```json
{
  "item_id": "uuid",
  "ciphertext": "base64",
  "nonce": "base64-24bytes",
  "sender_device_id": "uuid",
  "content_type": "text",
  "lamport_ts": 42
}
```

**Response 201:** `{}`

**Errors:** 401 (bad token), 404 (device not found), 429 (rate limit)

---

## GET /devices/:device_id/items?since_lamport=N

Poll for new items in this device's inbox.

**Headers:** `Authorization: Bearer <token>`

**Query:** `since_lamport=0` (returns items with lamport_ts > N)

**Response 200:**
```json
{
  "items": [
    {
      "item_id": "uuid",
      "ciphertext": "base64",
      "nonce": "base64",
      "sender_device_id": "uuid",
      "content_type": "text",
      "lamport_ts": 42
    }
  ]
}
```

---

## DELETE /devices/:device_id/items/:item_id

Delete a synced item from inbox.

**Response 200:** `{}`

---

## GET /health

Health check.

**Response 200:** `{"status": "ok"}`

---

## Notes

- Relay stores **only ciphertext** — no decryption keys
- Items are end-to-end encrypted (X25519 ECDH + XChaCha20-Poly1305)
- Per-device inbox quota: 500 items (oldest pruned on overflow)
- TTL: items expire per sender's `sync_ttl_secs` config (default 86400s)
- Sensitive items are **never uploaded** (detected by daemon before upload)
