# CopyPaste IPC Protocol

Newline-delimited JSON over a Unix domain socket.
One request per line in, one response per line out. UTF-8 only.

## Envelope

### Request

```json
{ "id": "client-supplied-id", "method": "list", "params": { "limit": 100 } }
```

| Field    | Type   | Required | Notes                                                              |
| -------- | ------ | -------- | ------------------------------------------------------------------ |
| `id`     | string | yes      | Echoed verbatim in the response. Used by clients to match replies. |
| `method` | string | yes      | Method name (see daemon source for the current set).               |
| `params` | object | no       | Method-specific arguments. Defaults to `{}`.                       |

### Response

```json
{ "id": "...", "ok": true,  "data": { ... } }
{ "id": "...", "ok": false, "error": "human message", "error_code": "not_found" }
```

| Field        | Type           | Present when            | Notes                                                                                                   |
| ------------ | -------------- | ----------------------- | ------------------------------------------------------------------------------------------------------- |
| `id`         | string         | always                  | Mirrors the request id (or `"?"` if the request itself failed to parse).                                |
| `ok`         | bool           | always                  | `true` on success, `false` on error.                                                                    |
| `data`       | any            | success only            | Method-specific payload.                                                                                |
| `error`      | string         | error only              | Human-readable, English. Suitable for logs; do not surface to end users without translation.            |
| `error_code` | string \| null | error only (when known) | Stable machine-readable code from the table below. Clients should branch on this, not on `error` text. |

## Error codes

Stable identifiers — once shipped, codes are never repurposed.
Adding new codes is allowed; renaming or repurposing an existing code is a breaking change.

| Code               | Meaning                                                              | Typical cause                                                                       |
| ------------------ | -------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `not_found`        | Requested resource (item id, peer, etc.) does not exist.             | `delete` on an already-deleted id; `paste` on a missing id.                         |
| `auth_failed`      | Authentication failed.                                               | Wrong credentials, expired token, missing keychain entry.                           |
| `invalid_argument` | Request was structurally valid JSON but violated the param contract. | Missing required field, wrong type, malformed fingerprint.                          |
| `not_implemented`  | Method is recognised but not yet implemented.                        | Cloud-sync stubs (`cloud_sign_in`, `cloud_sign_out`) until Supabase wiring lands.   |
| `ipc_not_ready`           | Daemon is booting — DB or cloud not yet ready.                                              | Client raced the daemon's startup; clients should back off and retry, not give up.                      |
| `internal_error`          | Catch-all for unexpected daemon-side failures.                                              | I/O error, db error, unhandled panic in a handler.                                                      |
| `version_mismatch`        | Client sent a `protocol_version` outside the daemon's supported range.                      | CLI/UI is too old or too new for this daemon; surface an upgrade prompt — do NOT retry the request.     |
| `migration_in_progress`   | The v4 key-rotation sweep is still running; ingest writes are temporarily refused.          | Client should back off and retry after a short delay.                                                   |
| `rate_limited`            | A conflicting single-active operation is already in flight (e.g. a second active pairing). | Wait for the current operation to finish, then retry.                                                    |

## Methods

All methods are defined in [`crates/copypaste-ipc/src/methods.rs`](../crates/copypaste-ipc/src/methods.rs).

### Core clipboard

| Method | Description |
|---|---|
| `list` | Fetch a paginated list of clipboard items |
| `history_page` | Fetch one paginated page of history (`{limit, offset}`) → `{items, total, own_device_id}` |
| `search` | Full-text search over clipboard items |
| `copy` | **Deprecated.** Returns `error_code: "not_implemented"`. Use `copy_item`. |
| `paste` | **Deprecated.** Returns `error_code: "not_implemented"`. Use `copy_item`. |
| `copy_item` | Copy a clipboard item back to the system clipboard by stable UUID (`{id}`) |
| `delete` | **Deprecated.** Returns `error_code: "not_implemented"`. Use `delete_item`. |
| `pin` | **Deprecated.** Returns `error_code: "not_implemented"`. Use `pin_item`. |
| `delete_item` | Delete a single clipboard item by stable UUID (`{id}`) |
| `delete_all` | Delete all clipboard items (clear history) |
| `count` | Return the total count of stored clipboard items |
| `stats` | Return aggregate statistics about the clipboard database |
| `pin_item` | Pin or unpin a clipboard item (`{id, pinned: bool}`) |
| `reorder_pinned` | Reorder pinned items (`{ids: [String]}` — full ordered list of pinned UUIDs) |
| `add_file_item` | Ingest a file directly into clipboard history from the UI (`{filename, mime, data_b64}`) → `{id}` |

### Daemon health

| Method | Description |
|---|---|
| `status` | Query the running daemon's health / readiness state |

### Import / export

| Method | Description |
|---|---|
| `export` | Export clipboard items as a JSON blob |
| `import` | Bulk-import clipboard items from a JSON blob |

### Private mode

| Method | Description |
|---|---|
| `set_private_mode` | Enable or disable clipboard recording pause mode |
| `get_private_mode` | Query the current private-mode state |

### Configuration and sync

| Method | Description |
|---|---|
| `get_config` | Read the current daemon configuration object |
| `set_config` | Write / merge a partial daemon configuration object |
| `set_sync_passphrase` | Store the shared sync passphrase and derive the sync key (`{passphrase}`) |
| `rotate_sync_key` | Rotate the sync key to a new passphrase (`{passphrase}`) → `{ok, rotated}` |
| `revoke_and_rotate` | Revoke a peer and rotate the sync key atomically (`{fingerprint, passphrase}`) → `{revoked_at, rotated}` |
| `store_cloud_password` | Store the Supabase GoTrue password directly in the Keychain (`{password}`) → `{persisted}` |
| `get_sync_status` | Query the current cloud-sync state → `{passphrase_set, supabase_configured, signed_in, last_sync_ms, badge_state, …}` |
| `cloud_test_connection` | Run a live connection diagnostic against the configured cloud backend |
| `cloud_sign_in` | Sign in to the cloud sync account (requires `cloud-sync` feature; returns `not_implemented` otherwise) |
| `cloud_sign_out` | Sign out of the cloud sync account (requires `cloud-sync` feature; returns `not_implemented` otherwise) |

### Own device identity

| Method | Description |
|---|---|
| `get_own_fingerprint` | Return this device's mTLS certificate fingerprint (hex SHA-256) → `{fingerprint}` |
| `get_own_device_info` | Return rich identity for this device: name, model, OS, version, IPs, fingerprint |

### Pairing (QR)

| Method | Description |
|---|---|
| `pair_generate_qr` | Generate a short-lived QR pairing payload |

### Pairing (LAN discovery / SAS)

| Method | Description |
|---|---|
| `list_discovered` | List peers visible via mDNS-SD, each tagged `paired` or not |
| `rescan_discovered` | Force an mDNS-SD rescan and return the fresh discovered device list |
| `pair_with_discovered` | Begin SAS pairing as initiator with a discovered peer (`{device_id}`) |
| `pair_get_sas` | Poll the SAS pairing state machine → `{state, sas?, role?}` |
| `pair_confirm_sas` | Deliver the local user's SAS accept/reject decision (`{accept: bool}`) |
| `pair_abort` | Abort in-flight discovery pairing and reset to `idle` |
| `pair_peer_with_password` | Pair with a peer using a shared password (`{peer_fingerprint, password}`) |
| `poll_peer_events` | Drain pending peer connect/disconnect events → `{events: [{kind, fingerprint}]}` |

### Peer management

| Method | Description |
|---|---|
| `list_peers` | List all paired devices → `{peers: [PairedDevice]}` with online status, last-seen, latency |
| `unpair_peer` | Remove a paired peer from the local trust store (`{fingerprint}`) |
| `revoke_peer` | Revoke a paired peer with a logged revocation timestamp (`{fingerprint}`) → `{revoked_at}` |
| `revoke_all_peers` | Revoke ALL paired peers in one call → `{revoked: u32}` |

### Item media access

| Method | Description |
|---|---|
| `get_item_image` | Fetch full image bytes for a `content_type == "image"` item (`{id}`) → `{data_uri}` |
| `get_item_file` | Fetch full binary payload for a `content_type == "file"` item (`{id}`) → `{filename, mime, data_b64}` |
| `get_item_thumbnail` | Fetch pre-computed thumbnail for a clipboard image (`{id}`) → `{thumbnail: String \| null}` |
| `get_app_icon` | Resolve a macOS app bundle ID to a 32×32 PNG icon (`{bundle_id}`) → `{png_b64: String \| null}` |

### Database maintenance

| Method | Description |
|---|---|
| `db_stats` | Return lightweight storage statistics → `{item_count, size_bytes}` |
| `vacuum` | Run `VACUUM` (and optionally `REINDEX`) on the encrypted database (`{reindex_only?, dry_run?}`) → `{ok, size_before, size_after, reclaimed}` |
| `reset_database` | Wipe and recreate the database (requires `{confirm: true}`) — usable in degraded mode → `{reset, ready}` |
| `db_backup` | Create an encrypted SQLCipher backup (`{dest_path}`) → `{ok, dest_path, size_bytes}` |
| `db_restore` | Restore the database from an encrypted backup (requires `{confirm: true, src_path, force?}`) → `{ok, ready}` |

## Client guidance

* Always check `ok` first.
* On `ok: false`, prefer `error_code` for control flow. Only fall back to substring matches on `error` when `error_code` is absent (legacy responses, or pre-W3.6 daemons).
* `ipc_not_ready` is transient — retry with backoff. All other codes should be surfaced to the user (translated as appropriate).

## Backwards compatibility

Older responses may omit `error_code` entirely. The field is **optional on the wire** and serialized via `#[serde(skip_serializing_if = "Option::is_none")]`. Clients MUST tolerate its absence and treat it as "code unknown".

### Deprecated methods

The following legacy method verbs are **still recognised** by the daemon but always respond with `ok: false, error_code: "not_implemented"`. Clients MUST migrate to the replacement before the next major release.

| Deprecated verb | Replacement |
|---|---|
| `copy` | `copy_item` |
| `paste` | `copy_item` |
| `delete` | `delete_item` |
| `pin` | `pin_item` |

## Source of truth

Codes are declared in [`crates/copypaste-daemon/src/protocol.rs`](../crates/copypaste-daemon/src/protocol.rs) as `ERR_CODE_*` constants. Helpers `Response::err_with_code` and `Response::not_implemented` produce tagged responses.
