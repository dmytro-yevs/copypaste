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
| `ipc_not_ready`    | Daemon is booting — DB or cloud not yet ready.                       | Client raced the daemon's startup; clients should back off and retry, not give up. |
| `internal_error`   | Catch-all for unexpected daemon-side failures.                       | I/O error, db error, unhandled panic in a handler.                                  |

## Client guidance

* Always check `ok` first.
* On `ok: false`, prefer `error_code` for control flow. Only fall back to substring matches on `error` when `error_code` is absent (legacy responses, or pre-W3.6 daemons).
* `ipc_not_ready` is transient — retry with backoff. All other codes should be surfaced to the user (translated as appropriate).

## Backwards compatibility

Older responses may omit `error_code` entirely. The field is **optional on the wire** and serialized via `#[serde(skip_serializing_if = "Option::is_none")]`. Clients MUST tolerate its absence and treat it as "code unknown".

## Source of truth

Codes are declared in [`crates/copypaste-daemon/src/protocol.rs`](../crates/copypaste-daemon/src/protocol.rs) as `ERR_CODE_*` constants. Helpers `Response::err_with_code` and `Response::not_implemented` produce tagged responses.
