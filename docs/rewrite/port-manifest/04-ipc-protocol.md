# Port Manifest 04 — Daemon IPC Protocol (external wire contract)

> **v2 scope:** The method inventory, error taxonomy, readiness semantics and
> path-redaction rule remain binding. Exact envelopes, version negotiation,
> legacy verbs and platform transports below describe v1 only; v2 deliberately
> has no wire compatibility with v0.4.x. See [the manifest scope](README.md).

Harvested from the legacy tree at `crates/copypaste-ipc/**`,
`crates/copypaste-daemon/src/protocol.rs`, `crates/copypaste-daemon/src/ipc/**`,
`crates/copypaste-cli/src/ipc.rs`, `crates/copypaste-ui/src-tauri/src/ipc.rs`,
and `docs/adr/ADR-007-ipc-protocol-versioning.md`.

---

## 1. Purpose & scope

This is the recovered specification of the v1 daemon's local IPC protocol. The
following table records the consumers that depended on that historical contract:

| Consumer | Location | Nature |
|---|---|---|
| `copypaste` CLI | `crates/copypaste-cli/src/ipc.rs` + `src/commands/**` | Untyped `serde_json::Value` client, 138 `.as_*()` field pokes across 20 files |
| Tauri desktop UI (Rust bridge) | `crates/copypaste-ui/src-tauri/src/ipc.rs` | Short-lived connection per call, fixed request id `"ui-1"` |
| Tauri desktop UI (TS) | `crates/copypaste-ui/src/lib/ipc.ts` | `IpcReply` with optional `protocol_version`, `CURRENT_PROTOCOL_VERSION = 1` |
| Daemon self-probe | `crates/copypaste-daemon/src/ipc/socket.rs:53` `probe_listening_daemon` | Blocking pre-bind `status` call for stale-daemon takeover |
| Android client | via relay/P2P, mirrors the same `SyncBadgeState` / `SYNC_BADGE_RECENT_MS` | Indirect |
| Frozen Windows skeleton | `crates/copypaste-daemon/src/ipc_win.rs` | Named-pipe transport, same envelope + same 16 MiB cap (ADR-012) |

Scope: transport, framing, envelope, protocol versioning, the complete method
catalogue, error codes, readiness gating, the single streaming method, and legacy
verbs. **Out of scope:** storage/sync/pairing-crypto semantics beyond what is
visible on the wire.

---

## 2. Invariants (MUST hold)

These were v1 wire-compatibility requirements. They are reference material for
v2 except where the manifest scope keeps their behaviour binding.

**I1 — `id` is a JSON string, always.**
Both `Request.id` and `Response.id` are `String` on the wire (`"1"`, `"req-42"`,
`"ui-1"`, `"takeover-probe"`). The daemon echoes it verbatim.
`crates/copypaste-ipc/src/lib.rs:22` records CopyPaste-crol: an `id: u64` typed
schema was a planning artefact that never matched the live wire and was reverted.
Pinned by `crates/copypaste-ipc/tests/wire_roundtrip.rs:19` and
`tests/snapshot.rs:38`.

**I2 — The CLI enforces id echo.** `crates/copypaste-cli/src/ipc.rs:234-241`
compares the response `id` **as a `serde_json::Value`** against the request id and
hard-errors on mismatch ("response id mismatch: sent X, got Y"). Therefore **every**
error path in the daemon — including JSON parse failures and oversized-request
rejections — must best-effort recover and echo the client's id. See CopyPaste-cbfl
(`ipc/mod.rs:220`) and `echo_id_from_prefix` (`ipc/connection.rs:51`).

**I3 — Field order is part of the observable contract.**
`serde_json` serialises in declaration order and
`crates/copypaste-ipc/tests/snapshot.rs` asserts byte-exact strings:
`Request` = `id, method, params, protocol_version`;
`Response` = `id, ok, data, error, error_code, protocol_version`.

**I4 — `Response.protocol_version` is always present**, including on every error
response (ADR-007 §Decision; `protocol.rs:126-128` — no `skip_serializing_if`).
`data`, `error`, `error_code` are omitted from the wire when `None`.

**I5 — Missing `Request.protocol_version` means `1`, not `0`.**
The daemon uses `#[serde(default = "default_protocol_version")]` → `1`
(`protocol.rs:40-42, 77`). The shared `copypaste_ipc::Request` uses plain
`#[serde(default)]` → `0`, which the gate would reject as below
`MIN_SUPPORTED_PROTOCOL_VERSION`. **This divergence is load-bearing**
(CopyPaste-c4q2.11, `protocol.rs:44-64`). The rewrite MUST default to `1`.

**I6 — One request = one line = one response line.** Newline-delimited JSON
(NDJSON/LDJSON). No length prefix, no framing header. `\r` is tolerated and
stripped (`connection.rs:413`). Empty lines are silently skipped as keep-alives
(`connection.rs:418`).

**I7 — Connections are persistent and support pipelining.** The daemon loops
reading lines from the same connection until EOF/timeout/error. The CLI opens one
connection per command (and reconnects on retry); the Tauri bridge opens one per call.

**I8 — Error codes are append-only.** Once shipped, a code string is never
repurposed or renamed (`response.rs:10-12`, `error.rs:24-26`). Unknown codes must
be forwarded gracefully — `ErrorCode::parse` returns `Option` (`error.rs:97`) and
the CLI keeps `raw_error_code` verbatim (CopyPaste-FEACLI-8, `cli/src/ipc.rs:254`).

**I9 — Clients branch on `error_code`, never on the `error` string.**
Stated in ADR-007, `response.rs:64-67`, and enforced across the CLI.

**I10 — `watch_subscribe` event frames are NOT `Response` envelopes.** They are a
distinct `{ok, event, id, …}` shape with no `protocol_version` and no `data`
wrapper. See §7 of the catalogue.

**I11 — `list_peers` MUST strip `password_file_enc` and `password_file_b64`.**
CopyPaste-5lm, `handlers_peers.rs:315-317`; regression-tested by
`list_peers_strips_password_file_fields` (`ipc/tests.rs:215`).

**I12 — `get_config` MUST NOT return `supabase_email` / `supabase_password`.**
It returns `AppConfigResponse`, a *structurally different type* with only
`supabase_email_set` / `supabase_password_set` booleans, built by exhaustive
destructuring so a new secret field fails to compile (CopyPaste-c4q2.18,
`ipc/config.rs:28-45`, `copypaste-ipc/src/methods/config.rs:171`).

**I13 — `set_config` MERGES, never overwrites.** Because `get_config` redacts
secrets, a naive read-modify-write would null them. `merge_config(read_config(),
incoming)` preserves any field the caller omitted (`handlers_config.rs:68-81`).

**I14 — Socket permissions.** Socket file mode `0600`; parent directory mode
`0700`. Set immediately after bind, before the accept loop
(`connection.rs:163-198`). The socket is the *only* authentication boundary —
there is no in-band auth.

**I15 — Unknown methods return an UNTAGGED error** (`Response::err`, no
`error_code`) with message `"unknown method: {other}"`
(`handlers_items.rs:37`). Deliberately distinct from `not_implemented`.

---

## 3. Transport & envelope (exact)

### 3.1 Socket path

Single canonical resolver: `copypaste_ipc::paths::socket_path()`
(`crates/copypaste-ipc/src/paths.rs:87`).

| Step | Rule |
|---|---|
| 1 | `COPYPASTE_SOCKET` env var, if set (even if empty string → used verbatim) |
| 2 | `app_support_dir()/daemon.sock`, on every platform |

v2 has no Windows arm here. The resolver stays platform-neutral and the Windows
transport derives a pipe name from whatever it returns
(`copypaste_ipc::transport::pipe::name_for`), because v1's fixed
`\\.\pipe\copypaste-daemon` is machine-global: two accounts on one machine would
contend for one endpoint, and the second daemon would refuse to start.

`app_support_dir()` (`paths.rs:41`):

| Platform | Directory |
|---|---|
| macOS | `~/Library/Application Support/CopyPaste` |
| Windows | `%APPDATA%\CopyPaste`, else `~/AppData/Roaming/CopyPaste` |
| Linux/other | `$XDG_DATA_HOME/copypaste`, else `~/.local/share/copypaste` |
| Last resort | `$TMPDIR/CopyPaste` (never panics) |

### 3.2 Bind, permissions, stale-socket self-healing

`IpcServer::bind` (`ipc/connection.rs:163`):

1. `create_dir_all(parent)`; `chmod 0700` on the parent (warn-only on failure).
2. `bind_with_stale_cleanup(socket_path)` (`ipc/socket.rs:263`).
3. `chmod 0600` on the socket (hard error on failure).

`bind_with_stale_cleanup` policy (CopyPaste-ah1m atomicity fix):

* Acquires an **exclusive `flock(2)`** on `<socket_path>.lock` for the whole
  probe→remove→bind critical section, so two concurrently-starting daemons cannot
  both conclude "stale". The lockfile is created but never deleted.
* No file present → bind.
* File present, no live listener → stale; `remove_file` then bind.
* File present, **live listener with the SAME `build_version`** → refuse to steal;
  return `Err` so the caller exits cleanly (dual-daemon prevention).
* File present, live listener with a **different (or absent) `build_version`** →
  attempt eviction: `SIGTERM` the reported `pid`, poll until the socket stops
  answering, then rebind. Newest binary wins on upgrade.
* Live listener that reports `degraded: true` at the same version → **may** be
  replaced (a degraded peer does not get the same-version protection).

Eviction TOCTOU guard (CopyPaste-dl1e, `socket.rs:109`): never signal pid 0, 1, or
self; validate `/proc/<pid>/exe` (Linux) or `proc_pidpath` (macOS) contains
`"copypaste"`; re-probe after SIGTERM to confirm the socket actually freed. Fail-safe
— if identity cannot be confirmed, do NOT signal.

The takeover probe (`probe_listening_daemon`, `socket.rs:53`) sends
`{"id":"takeover-probe","method":"status","params":{}}\n` with a **3 s** read/write
timeout and reads `data.build_version`, `data.pid`, `data.degraded`.
**This means `status` must be answerable synchronously, before readiness, and its
`build_version`/`pid`/`degraded` fields are load-bearing for daemon lifecycle.**

### 3.3 Framing & size caps

Newline-delimited UTF-8 JSON, one object per line. Two-pass, **method-aware**
size cap (CopyPaste-c4q2.28, `connection.rs:304-410`):

| Constant | Value | Source | Applies to |
|---|---|---|---|
| `SMALL_REQUEST_BYTES` | `64 * 1024` = 65 536 B | `ipc/consts.rs:303` | Every method not on the large allow-list |
| `MAX_REQUEST_BYTES` | `16 * 1024 * 1024` = 16 777 216 B | `ipc/consts.rs:290` | Only `import`, `add_file_item` |
| `copypaste_ipc::MAX_IPC_REQUEST_BYTES` | `16 * 1024 * 1024` | `copypaste-ipc/src/lib.rs:110` | Shared SoT for Unix + Windows pipe + CLI import pre-flight |
| `MAX_RESPONSE_BYTES` (CLI) | `16 * 1024 * 1024` | `cli/src/ipc.rs:64` | Client-side response ceiling |
| `MAX_PAGE` | `1000` | `ipc/consts.rs:337` | Server-side clamp on `limit` for `search` / `history_page` |
| `MAX_IMPORT_ITEM_BYTES` | `4 * 1024 * 1024` | `ipc/consts.rs:344` | Per-item decoded `content_bytes_b64` on `import` |
| `MAX_CONCURRENT_CONNECTIONS` | `64` | `ipc/consts.rs:357` | Semaphore permits |
| `MAX_PAKE_SESSIONS` / `PAKE_SESSION_TTL` | see `ipc/pairing.rs` | | In-flight PAKE session bound |
| `PEER_EVENT_QUEUE_CAP` | `64` | `ipc/consts.rs:371` | `poll_peer_events` buffer |
| `QR_PAIRING_TTL_SECS` | `120` | `copypaste-ipc/src/lib.rs:98` | QR validity; must equal `BOOTSTRAP_ACCEPT_TIMEOUT` |

Algorithm:

1. **Phase 1** — read at most `SMALL_REQUEST_BYTES + 1` bytes up to `\n`.
2. If the buffer did **not** end in `\n`: scan the buffered prefix with a
   hand-rolled byte scanner `extract_json_string_field(prefix, "method")`
   (`connection.rs:15`) — `serde_json` cannot help because the object is cut
   mid-stream. This requires well-behaved clients to serialise `id` and `method`
   **before** `params` (which `copypaste_ipc::Request` field order guarantees).
3. If the method is **not** `import`/`add_file_item` → send `request_too_large`
   (limit rendered as `"64 KiB"`) with detail
   `"Only bulk methods (import, add_file_item) may exceed it."` and **close the
   connection**.
4. **Phase 2** — for allow-listed methods, read the remainder up to
   `MAX_REQUEST_BYTES + 1`. Still no `\n` → `request_too_large` (limit rendered as
   `"16 MiB"`) with detail `"For large imports split the payload into smaller
   batches."` and close.
5. Trim trailing `\n`/`\r`; skip empty lines; reject invalid UTF-8 with an
   **untagged** `Response::err("0", "invalid UTF-8: {e}")` (note: id is the literal
   `"0"`, not echoed — this violates I2 and will trip the CLI's id guard; a known
   wart worth fixing in the rewrite).

Oversized-request error message template (verbatim):
`request too large: IPC request exceeds the {N MiB|N KiB} limit. {detail}`
Its id is recovered by `echo_id_from_prefix` → `extract_json_string_field(prefix,
"id")`, falling back to the literal `"0"`.

### 3.4 Timeouts

| Timeout | Value | Side | Source |
|---|---|---|---|
| `IPC_READ_TIMEOUT` | 30 s per read (applied to both phases) | Daemon | `ipc/consts.rs:315` (CopyPaste-cce1) |
| `IPC_WRITE_TIMEOUT` | 10 s per write | Daemon | `ipc/consts.rs:331` (CopyPaste-c4q2.24) |
| CLI ordinary request | 5 s end to end | CLI | `cli/src/client.rs` |
| Tauri `DEFAULT_BUDGET` | 10 s end to end | UI backend | `backend/daemon.rs` |
| Tauri / CLI long budget | 180 s end to end | Both clients | `Method::is_long_running` |
| Takeover probe | 3 s | Daemon startup | `ipc/socket.rs:60` |
| `list_peers` bootstrap join | 5 s | Daemon-internal | `handlers_peers.rs:81` (CopyPaste-7mf) |

**v2 amendment.** v1 applied the long budget to exactly `vacuum`, `db_backup`,
`db_restore`, `import`, `pair_with_discovered`, `pair_peer_with_password`
(CopyPaste-8ebg.4). v2 keeps the reason and widens the set, because the same
argument covers work v1 never drove from a client: `sync_now`, `cloud_sync_now`
and `cloud_sign_in` are round trips over a network, and five or ten seconds
cancels one the daemon was going to finish. `rescan` is **not** in the set — it
republishes an mDNS record and returns a cache, blocking on nothing, and the
long budget left the Refresh button disabled for three minutes. The set is
`Method::is_long_running` in `copypaste-ipc` — one list both clients read, since
v1 kept three models of this contract and they disagreed.

The budget is **end to end** rather than per read: connect, write and reply
share one deadline, because the number a caller can reason about is how long the
request can take, not how long one of its three steps can.

The daemon writes nothing until the handler completes, so a client budget
shorter than the work does not protect the client — it cancels an operation that
was going to succeed. Any rewrite keeping synchronous request/response must
preserve these budgets or introduce progress frames.

**A blown budget is not "unreachable".** The connection was accepted, so a
process is there. Both clients report it as its own condition — `EXIT_TIMEOUT`
(4) in the CLI, `BackendError::Timeout` in the app, which `Supervisor::state`
reads as `Unhealthy` rather than `Stopped`. Reporting it as unreachable told a
user to start a daemon that was already holding the endpoint.

On read timeout the daemon **drops the connection without sending a response** —
the client sees EOF, which its retry logic must handle.

### 3.5 Concurrency

`tokio::sync::Semaphore` with 64 permits, `try_acquire_owned()` (never blocks the
accept loop). Over-cap connections are **immediately dropped** at the OS level with
no error frame (CopyPaste-6ot5, `connection.rs:255-283`). Connection tasks live in a
`JoinSet` and are `abort_all()`-ed on shutdown (daemon-core L2).

### 3.6 Request envelope

```json
{"id":"1","method":"history_page","params":{"limit":50,"offset":0},"protocol_version":1}
```

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | string | **yes** | — | Echoed verbatim. No uniqueness enforced by the daemon. |
| `method` | string | **yes** | — | Bare verb, dispatched by string match. |
| `params` | any JSON | no | `null` (`#[serde(default)]`) | Handlers use `.get(k).and_then(as_*)`, so `null` behaves like `{}` for all optional params. |
| `protocol_version` | u32 | no | **`1`** (daemon) / `0` (shared crate — see I5) | |

Any parse failure returns
`{"id":<recovered>,"ok":false,"error":"parse error: {serde msg}","error_code":"invalid_argument","protocol_version":1}`.
The id is recovered from `v["id"]` as string, then `as_i64`, then `as_u64`,
finally the literal `"?"` (`ipc/mod.rs:216-241`; CopyPaste-cbfl). Tested by
`parse_error_echoes_id_from_raw_json` / `parse_error_uses_fallback_id_when_not_valid_json`
(`ipc/tests.rs:7100, 7121`).

### 3.7 Response envelope

```json
{"id":"1","ok":true,"data":{…},"protocol_version":1}
{"id":"1","ok":false,"error":"item not found: …","error_code":"not_found","protocol_version":1}
```

| Field | Type | Present | Notes |
|---|---|---|---|
| `id` | string | always | |
| `ok` | bool | always | |
| `data` | any JSON | success only (`skip_serializing_if = Option::is_none`) | May legitimately be `null` (e.g. legacy `delete`) |
| `error` | string | error only | Human text, **never** to be pattern-matched |
| `error_code` | string | error only, and only via `err_with_code` | Legacy `Response::err` omits it entirely |
| `protocol_version` | u32 | **always** | |

### 3.8 Protocol version negotiation (ADR-007)

* `PROTOCOL_VERSION = 1` — single source of truth at
  `crates/copypaste-ipc/src/lib.rs:78`. `protocol.rs:31` re-exports it as
  `CURRENT_PROTOCOL_VERSION` (CopyPaste-c4q2.19: no separate literal).
* `MIN_SUPPORTED_PROTOCOL_VERSION = 1` (`protocol.rs:35`).
* Gate (`check_request_gates`, `connection.rs:685-709`), applied **before** any
  handler and also on the `watch_subscribe` interception path
  (CopyPaste-crh3.105):

  ```
  if pv < MIN || pv > CURRENT:
      → ok=false, error_code="version_mismatch",
        error="unsupported protocol version {pv} (daemon supports {MIN}..={CURRENT})"
  ```
* **Clients MUST NOT retry on `version_mismatch`** — it is a hard mismatch. The
  CLI converts it into a fatal `anyhow!("version mismatch: {msg} — upgrade CLI or
  restart daemon")` at `cli/src/ipc.rs:259-267`, **before** any retry logic runs.
* The TS UI warns (`console.warn`) when `reply.protocol_version >
  CURRENT_PROTOCOL_VERSION` but does not block; the Tauri Rust bridge does not
  currently forward the field, so TS sees `undefined` and treats it as "no
  mismatch" (ADR-007 §TypeScript client implementation).
* Bump rules: rename/remove a field, remove a method, change a field's type.
  Additive changes (new method, new optional field, new error code, new `data`
  field) do NOT bump.

### 3.9 serde defaults that matter

| Site | Attribute | Effect |
|---|---|---|
| `protocol.rs:77` `Request::protocol_version` | `default = "default_protocol_version"` → `1` | Alpha clients keep working |
| `copypaste-ipc/src/request.rs:32` | `default` → `0` | **Diverges**; would be rejected by the gate |
| `Request::params` | `default` → `Value::Null` | Missing `params` is legal |
| `Response::{data,error,error_code}` | `skip_serializing_if = Option::is_none` | Absent, not `null`, on the wire |
| `Response::protocol_version` | none (daemon) / `default` (shared crate) | Always serialised by the daemon |
| `AppConfig::*` | all `Option<T>`, `default`, most `skip_serializing_if` | `None` on `set_config` = "leave unchanged" |
| `PairedDevice::*` (list_peers rows) | nearly all `#[serde(default)]` | Old `peers.json` records parse |
| `SyncBadgeState` / `PeerTransport` / `ErrorCode` | `rename_all = "snake_case"` / `"lowercase"` | Wire strings |

---

## 4. Method catalogue

### 4.0 Dispatch chain

`dispatch()` (`ipc/mod.rs:216`) parses, records tracing, runs `check_request_gates`,
then enters a chain-of-responsibility. Each `dispatch_*` matches its verbs and falls
through via `_ =>`:

```
dispatch_items      (handlers_items.rs:11)
  └→ dispatch_config    (handlers_config.rs:5)
      └→ dispatch_sync      (handlers_sync.rs:10)
          └→ dispatch_status    (handlers_status.rs:5)
              └→ dispatch_db        (handlers_db.rs:5)
                  └→ dispatch_peers     (handlers_peers.rs:5)
                      └→ dispatch_pairing   (handlers_pairing.rs:10)
                          └→ dispatch_transfer  (handlers_transfer.rs:5)
                              └→ dispatch_items_extra (handlers_items.rs:33)
                                  └→ Response::err("unknown method: {m}")   [untagged]
```

`watch_subscribe` never reaches `dispatch()` — it is intercepted in
`handle_connection` (`connection.rs:446-468`).

**Count:** 59 distinct method strings + 2 `pair_peer_with_password` sub-steps
(`initiate`/`finish`) = **61 addressable operations**. `pair_accept_qr` additionally
has two behavioural modes selected by which params are present.

Notation below: `S` = string, `B` = bool, `U` = unsigned int, `I` = signed int,
`A` = array, `O` = object. Every handler also inherits the envelope-level errors
(`invalid_argument` on parse failure, `version_mismatch`, `request_too_large`) and,
where marked **DB**, the readiness gate `ipc_not_ready`.

---

### 4.1 Clipboard items — read (`handlers_items_read.rs`)

| # | Method | Params | Success `data` | Errors | DB gate | Source |
|---|---|---|---|---|---|---|
| 1 | `list` | *(ignored)* | — | **always** `not_implemented`: `"list is deprecated: use history_page with {limit, offset} — the response shape is identical but pinned items appear first (c4q2.17)"` | no (c4q2.17 removed it from `requires_db`) | `handlers_items_read.rs:10` |
| 2 | `count` | none | `{count: I}` | `internal_error` (join failure); untagged `err` (DB error) | **yes** | `:19` |
| 3 | `search` | `query: S` **req**; `limit: U` (def 20, clamped `MAX_PAGE`); `kind: S` (`"text"`\|`"image"`\|`"file"`, unknown ⇒ empty result) | `{items:[{id,content_type,is_sensitive,wall_time,lamport_ts,preview,pinned,kind,too_large_to_sync}]}` | `invalid_argument` (`"missing param: query"`); `internal_error` | **yes** | `:27` |
| 4 | `stats` | none | `{total_items:I, sensitive_items:I, version:"1", build_version:S}` | `internal_error` | **yes** | `:137` |
| 5 | `history_page` | `limit: U` (def 50, clamp `MAX_PAGE`); `offset: U` (def 0); `cursor: O{wall_time:I, id:S, pinned?:B, pin_order?:F\|null}` (optional keyset mode) | `{items:[…], total:I, own_device_id:S, next_cursor:O\|null}` | `invalid_argument` on malformed cursor: `"invalid cursor: expected {wall_time: number, id: string, pinned?: bool, pin_order?: number\|null}"`; `internal_error`; untagged `err` | **yes** | `:170` |

`history_page` item object (`handlers_items_read.rs:331-345`):
`id, content_type, is_sensitive, wall_time, lamport_ts, preview, pinned, pin_order,
sensitive_spans, too_large_to_sync, origin_device_id, origin_device_name, kind`.

* `preview` rules: sensitive → `"[sensitive — id:{first 8 of id}]"`; text → FTS
  preview or `"[text — id:{8}]"`; file → `"[file: {filename}]"`; image →
  `"[image — id:{8}]"`.
* For non-sensitive text the preview is **NFKC-normalised** and `sensitive_spans`
  is an array of `[start_char, end_char]` pairs indexed into the **returned**
  (normalised) preview, not the raw text (CopyPaste-mnte;
  `history_page_spans_index_into_returned_preview_not_raw`, `tests.rs:1750`).
* `kind` ∈ classified text label (`copypaste_core::text_kind`) \| `"FILE"` \| `"IMAGE"`.
* `next_cursor` is `null` unless `items.len() == limit`; when present it is
  `{wall_time, id, pinned, pin_order}` of the last item — a *flat structured*
  cursor, deliberately not an opaque token (CopyPaste-a3nu).
* `search` returns a **subset** of the item fields (no `pin_order`,
  `sensitive_spans`, `origin_*`) and no `total`.

---

### 4.2 Clipboard items — mutate (`handlers_items_mutate.rs`)

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 6 | `delete` *(legacy)* | `id: S` (must be a valid UUID) | `null` | `invalid_argument` (`"missing param: id"`, `"invalid param: id must be a valid UUID"`); `internal_error` | yes | `:6` |
| 7 | `delete_all` | none | `{deleted: U}` | untagged `err`; `internal_error` | yes | `:27` |
| 8 | `pin` *(legacy, pin-only)* | `id: S` UUID | `{pinned: true, id: S}` | `invalid_argument`; `internal_error` | yes | `:117` |
| 9 | `pin_item` | `id: S` UUID **req**; `pinned: B` **req** | `{pinned: B, id: S}` | `invalid_argument` (`"missing param: pinned (bool)"` / bad UUID); untagged `err`; `internal_error` | yes | `:166` |
| 10 | `reorder_pinned` | `ids: A<S>` **req** | `{ok: true}` | `invalid_argument` (`"missing param: ids (array of item id strings)"`, `"ids must be an array of strings"`); untagged `err`; `internal_error` | yes | `:218` |
| 11 | `delete_item` | `id: S` UUID **req** | `{deleted: B, id: S}` | `invalid_argument`; `internal_error` | yes | `:284` |

`delete_all` only tombstones **non-pinned, non-deleted** rows, in a single
transaction (CopyPaste-cb7u / jvzm.3). All mutations re-read the row and broadcast
it on `new_item_tx` so P2P/cloud LWW converges.

---

### 4.3 Clipboard items — paste-back (`handlers_items_clipboard.rs`)

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 12 | `copy` *(legacy)* | `id: S` UUID | `{id, content_type, written: true}` | `invalid_argument`; `not_found` (`"item not found: {id}"`); `auth_failed` (`"paste decrypt failed: {msg}"`); `internal_error` (`"pasteboard write failed: {msg}"`) | yes | `:7` |
| 13 | `paste` *(legacy, alias of `copy`)* | same | same | same | yes | `handlers_items.rs:17` |
| 14 | `copy_item` | `id: S` UUID **req** | `{id, content_type, preview: S\|null, written: true}` | `invalid_argument`; `not_found`; `auth_failed`; untagged `err` (`"pasteboard write failed: …"` — note: **untagged** here, unlike `copy`) | yes | `:137` |

`preview`: plaintext for non-sensitive text; `"[file: {name}]"` for files; `null`
for images. All three promote-on-copy (bump `wall_time`/`lamport_ts`, recompute
`expires_at` from the live `sensitive_ttl_secs`; CopyPaste-8ebg.2, ojhe) —
best-effort, failures are logged not surfaced.

---

### 4.4 Clipboard items — media (`handlers_items_media.rs`)

> SECURITY note in-source: these handlers dispatch decrypt on the row's
> `key_version` (v1 raw seed vs `derive_v2`). That dispatch must move verbatim.

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 15 | `get_item_image` | `id: S` UUID **req** | `{data_uri: "data:image/png;base64,…"}` (always PNG regardless of stored MIME) | `invalid_argument` (bad/missing id; `"item {id} is not an image (content_type: X)"`); `not_found`; `internal_error` (no content blob / missing `blob_ref` / parse / `chunks_from_blob`); `auth_failed` (`"image item {id} decode failed: …"`) | yes | `:20` |
| 16 | `get_item_thumbnail` | `id: S` UUID **req** | `{thumbnail: "data:image/png;base64,…"}` **or** `{thumbnail: null}` | `invalid_argument`; `not_found`; untagged `err` (non-image, missing `blob_ref`, thumb decode); `internal_error` | yes | `:159` |
| 17 | `get_item_file` | `id: S` UUID **req** | `{filename: S, mime: S, data_b64: S}` (standard base64) | `invalid_argument` (`"item {id} is not a file (content_type: X)"`); `not_found`; `internal_error`; `auth_failed` | yes | `:346` |

`get_item_thumbnail` performs a **lazy Phase-4 backfill**: if `thumb` is NULL, or
the stored thumbnail's recorded dims exceed the current `THUMBNAIL_MAX_DIM` cap
(HB-10 350 MB image-memory regression), it regenerates and persists a new thumbnail
in-place, then decodes it. Any backfill failure yields `{thumbnail: null}` — never
an error — and callers fall back to `get_item_image`.

---

### 4.5 Item ingest & icons (`handlers_items_ingest.rs`)

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 18 | `get_app_icon` | `bundle_id: S` **req** | `{png_b64: S\|null}` (32×32 PNG; `null` when not installed / not extractable) | untagged `err` (`"missing param: bundle_id"`); `internal_error` | no | `:270` |
| 19 | `add_file_item` | `filename: S` **req, non-empty**; `mime: S` (def `"application/octet-stream"`); `data_b64: S` **req** | `{id: S}` | `invalid_argument` (`"missing or empty param: filename"`, `"missing param: data_b64"`, `"data_b64 decode error: {e}"`); `internal_error` (`"add_file_item failed: …"`) | yes | `:294` |

`add_file_item` is on the **large-payload allow-list** (up to 16 MiB request line).
Its `file_id` is a content hash so identical files dedup across captures; `item_id`
is `Uuid::from_bytes(file_id)`.

---

### 4.6 Config (`handlers_config.rs`)

> **Amended for v2 (2026-07-30).** Implemented as `Method::{GetConfig,
> SetConfig}` over `copypaste_ipc::config`, which is the single model of the
> settings — bounds, validation and the per-field "does this take effect without
> a restart" answer all live there, so the daemon, the CLI and the Settings UI
> cannot disagree about them. Ten fields rather than v1's 21; the type's doc
> comment names every one that was dropped and why, including one that was
> **considered and refused** (a user-settable sensitive-content confidence
> threshold, which would let a slider authorise auto-deletion).
>
> Three behaviours are binding and are implemented: `set_config` takes a *patch*
> rather than a whole record, so two open Settings tabs cannot lose each other's
> writes; a value out of range is rejected whole and the daemon keeps running on
> the last good configuration; and no error names the config store. The record
> lives in `sync_device_state` inside the SQLCipher database rather than in a
> `config.toml`, which is what makes the last of those structural — there is no
> path to leak — and is why a restore deliberately leaves that table alone.
>
> A fourth is binding and was **added in v2**: an unreadable record must fail
> *closed*. v1's rule was "a bad value never bricks the daemon", satisfied by
> falling back to the defaults — and the defaults have private mode off, LAN
> visibility on and sync on, so one bad byte turned a user's privacy settings
> back on and said nothing. v2 reads the record field by field, keeps every
> field that decodes, and for one that does not takes the closed value rather
> than the default: private mode on, sync and LAN visibility off. An unreadable
> `excluded_app_bundle_ids` cannot be recovered, so it turns private mode on —
> an empty list is the fail-open answer and means a password manager's copies
> start being recorded. A field simply *absent* still takes its default, or
> every downgrade would read as a privacy failure.
>
> What fell back is reported on `status` as `SettingsHealth` — field names only,
> never a value — and the app announces it. Running on settings the user did not
> choose while rendering them as if they had is the failure this closes.
> Implemented in `crates/copypaste-ipc/src/config.rs`,
> `crates/copypaste-daemon/src/settings/` and `server/config.rs`.

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 20 | `get_config` | none | `AppConfigResponse` (see below) | untagged `err` (serialise failure); `internal_error` (`"get_config blocking task failed: {e}"`) | no | `:7` |
| 21 | `set_config` | an `AppConfig` object (all fields optional) | `{saved: true}` | untagged `err` (`"invalid config: {e}"`, write failure); `internal_error` (`"set_config blocking task failed: {e}"`) | no | `:39` |

`AppConfig` fields (`copypaste-ipc/src/methods/config.rs:36`), all `Option<T>`:
`p2p_enabled: B`, `supabase_url: S`, `supabase_anon_key: S`, `relay_url: S`,
`supabase_email: S`, `supabase_password: S`, `max_text_size_bytes: U`,
`max_image_size_bytes: U`, `max_file_size_bytes: U`, `storage_quota_bytes: U`,
`sensitive_ttl_secs: U`, `sync_on_wifi_only: B`, `sound_on_copy: B`,
`notify_on_copy: B`, `collect_public_ip: B`, `paste_as_plain_text: B`,
`excluded_app_bundle_ids: A<S>`, `lan_visibility: B`, `sync_enabled: B`,
`auto_apply_synced_clip: B`.

`AppConfigResponse` (`:171`) is the same **minus** `supabase_email` /
`supabase_password`, **plus** `supabase_email_set: bool` and
`supabase_password_set: bool` (both non-optional).

`set_config` side effects that are part of the observable contract:

* `supabase_password` is migrated to the macOS Keychain and **stripped** from
  `config.json` only after a successful read-back (the ephemeral-key bypass returns
  `Ok(())` as a no-op; a blind strip would lose the secret).
* `lan_visibility` is **hot-applied**: `Some(true)` restarts mDNS-SD, `Some(false)`
  stops it, `None` = no change.
* `p2p_enabled` is persisted but **requires a daemon restart** to take effect
  (CopyPaste-bjh); the daemon only logs a notice.
* `relay_url: Some("")` (or whitespace) is the **explicit-clear sentinel** and
  shuts down the running relay orchestrator at runtime. `None` is *not* a clear
  (CopyPaste-44rq.67).

---

### 4.7 Sync — auth (`handlers_sync_auth.rs`)

| # | Method | Params | Success `data` | Errors | Feature | Source |
|---|---|---|---|---|---|---|
| 22 | `store_cloud_password` | `password: S` **req** (typed deserialize) | `{persisted: B}` — `true` when the Keychain round-trip confirmed; `false` on non-macOS / bypass (password held in memory only, lost on restart) | `invalid_argument` (`"invalid store_cloud_password params: {e}"`, `"password must not be empty"`); `internal_error` | always compiled | `:20` |
| 23 | `cloud_sign_in` | none (creds resolved from `CloudConfig::from_env`) | `{signed_in: true}` | `invalid_argument` (`"cloud-sync not configured: set supabase_url and supabase_anon_key …"`); `auth_failed` (`"sign-in failed: {e}"`) | `cloud-sync`; else `not_implemented("cloud-sync")` | `:113` |
| 24 | `cloud_sign_out` | none | `{signed_in: false}` | — (always succeeds; Keychain/config clear failures are logged only) | `cloud-sync`; else `not_implemented("cloud-sync")` | `:160` |

`cloud_sign_out` is **persistent** (CopyPaste-crh3.100): it deletes the Keychain
Supabase password and clears `supabase_email` / `supabase_password` from
`config.json`, but deliberately **keeps** the project URL + anon key.

---

### 4.8 Sync — keys (`handlers_sync_keys.rs`)

| # | Method | Params | Success `data` | Errors | Feature | Source |
|---|---|---|---|---|---|---|
| 25 | `set_sync_passphrase` | `passphrase: S` **req, non-empty** | `{ok: true}` | `invalid_argument` (`"missing or empty param: passphrase"`); `auth_failed` (no Supabase account id); untagged `err` (`"key derivation failed: {e}"`) | `cloud-sync`; else `not_implemented("cloud-sync")` | `:18` |
| 26 | `rotate_sync_key` | `passphrase: S` **req, non-empty** | `{ok: true, rotated: true}` | same as above | `cloud-sync` **or** `relay-sync`; else `not_implemented("cloud-sync or relay-sync")` | `:87` |
| 27 | `revoke_and_rotate` | `fingerprint: S` **req** (validated); `passphrase: S` **req, non-empty** | `{ok: true, removed: B, revoked_at: I, fingerprint: S, rotated: true}` | `invalid_argument` (`"missing param: fingerprint"`, `"invalid fingerprint format: {fp}"`, `"missing or empty param: passphrase"`); `auth_failed`; untagged `err`; `internal_error` | `cloud-sync` **or** `relay-sync` | `:217` |

Key derivation is Argon2id over `(passphrase, supabase_account_id)`. The account id
is **required** — without it, other devices of the same account could not reproduce
the key. Absence yields `auth_failed`. `revoke_and_rotate` derives the new key
**first**, so a bad passphrase fails before any revocation state is mutated.
`revoke_and_rotate` requires a ready DB.

---

### 4.9 Sync — status (`handlers_sync_status.rs`)

| # | Method | Params | Success `data` | Errors | Feature | Source |
|---|---|---|---|---|---|---|
| 28 | `get_sync_status` | none | `{passphrase_set:B, supabase_configured:B, signed_in:B, last_sync_ms:I\|null, supabase_url:S\|null, email:S\|null, badge_state:S\|null, supabase_account_id:S\|null}` | `internal_error` (`"get_sync_status blocking task failed: {e}"`) | `cloud-sync`; else `not_implemented("cloud-sync")` | `:7` |
| 29 | `cloud_test_connection` | none | `{ok:B, configured:B, stage:S, message:S}` — **always `ok=true` at the envelope level**; the diagnostic verdict is in `data.ok` | none (envelope always succeeds) | `cloud-sync`; else `not_implemented("cloud-sync")` | `:128` |

`get_sync_status.email` is **masked** (`a***@example.com`, or `*@domain` for
1-char locals, or `"<redacted>"` for non-addresses) — M3 fix, so same-UID processes
cannot harvest the full GoTrue address. `supabase_anon_key`, the password, and the
passphrase are never returned.

`badge_state` ∈ `"synced" | "syncing" | "idle" | "offline" | "error" |
"misconfigured"` — computed daemon-side by
`compute_sync_badge_state_with_inflight` so macOS, Android, and CLI all render the
same value (CopyPaste-merc / 1jms.22). `SYNC_BADGE_RECENT_MS = 300_000`
(`copypaste-ipc/src/methods/badge.rs:64`).

`cloud_test_connection.stage` ∈ `"config" | "url" | "auth" | "network" | "done" |
"key" | "table" | "rls" | "http"` with an actionable `message` per stage
(`handlers_sync_status.rs:185-350`). The probe is a single
`GET {url}/rest/v1/clipboard_items?limit=0` with `SYNC_HTTP_TIMEOUT` (30 s,
CopyPaste-16vr).

---

### 4.10 Status & private mode (`handlers_status.rs`)

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 30 | `set_private_mode` | `enabled: B` **req** | `{private_mode: B, private_mode_epoch: U}` | `invalid_argument` (`"missing param: enabled (bool)"`) | no | `:7` |
| 31 | `get_private_mode` | none | `{private_mode: B, private_mode_epoch: U}` | — | no | `:40` |
| 32 | `status` | none | see below | — (never errors) | **no — must answer while degraded** | `:50` |

`status` healthy branch:
```json
{"status":"running","private_mode":B,"private_mode_epoch":U,"ready":B,
 "degraded":false,"build_version":S,"pid":U,"device_key_fingerprint":S}
```
`status` degraded branch:
```json
{"status":"degraded","private_mode":B,"private_mode_epoch":U,"ready":false,
 "degraded":true,"degraded_reason":S,"build_version":S,"pid":U,
 "device_key_fingerprint":S}
```

`degraded_reason` values are a **closed set** the UI keys its recovery banner off
(`ipc/consts.rs:376,386`):

| Value | Meaning |
|---|---|
| `"keychain_locked"` | SQLCipher key unreachable (post-reinstall regression) |
| `"db_key_mismatch"` | Key obtained but does not match the DB (`SQLITE_NOTADB`); "re-grant the Keychain prompt" would NOT help |

`private_mode_epoch` is a monotonically-increasing counter bumped on every
`set_private_mode`, exposed in `status` and `get_private_mode` so a poller can
detect a change without a subscription (CopyPaste-48k0).
`device_key_fingerprint` = lowercase hex `SHA-256(X25519 device public key)` —
**informational only**, distinct from the mTLS cert fingerprint used for pairing
(CopyPaste-ruep).

---

### 4.11 Database admin (`handlers_db.rs`)

| # | Method | Params | Success `data` | Errors | DB gate | Source |
|---|---|---|---|---|---|---|
| 33 | `reset_database` | `confirm: B` **must be `true`** | `{reset: true, ready: true}` | `invalid_argument` (`"reset_database requires confirm=true"`); `internal_error` | **NO — deliberately excluded from `requires_db`; it is the escape hatch out of degraded mode** | `:18` |
| 34 | `vacuum` | `reindex_only: B` (def false); `dry_run: B` (def false) | `{ok: true, size_before: U, size_after: U, reclaimed: I}` | `internal_error` (`"dry-run DB probe failed"`, `"VACUUM failed"`, `"REINDEX failed"`, `"vacuum blocking task failed"`) | **yes** (CopyPaste-crh3.7) | `:191` |
| 35 | `db_stats` | none | `{item_count: U, size_bytes: U}` | `internal_error` | **yes** | `:302` |
| 36 | `db_backup` | `dest_path: S` **req, non-empty** | `{ok: true, dest_path: S, size_bytes: U}` | `invalid_argument` (`"db_backup requires a non-empty dest_path"`, `"db_backup: dest_path already exists: {p}"`); `internal_error` (parent dir missing, `VACUUM INTO failed`) | **yes** (CopyPaste-crh3.7) | `:344` |
| 37 | `db_restore` | `confirm: B` **must be `true`**; `src_path: S` **req, non-empty, must be an existing file**; `force: B` (def false) | `{ok: true, ready: true}` | `invalid_argument` (`"db_restore requires confirm=true"`, `"db_restore requires a non-empty src_path"`, `"db_restore: backup file not found: {p}"`); `ipc_not_ready` (degraded **and** Keychain unreachable — no filesystem change made); `internal_error` | **NO — recovery escape hatch, must work while degraded** | `:468` |

`vacuum`/`db_backup` are gated in degraded mode specifically because otherwise
`db_backup` would `VACUUM INTO` the empty in-memory placeholder and return
`{ok:true}` for an EMPTY backup, and `vacuum` would report `size_before`/`size_after`
read from the REAL on-disk file while operating on the placeholder — both dangerously
misleading (CopyPaste-crh3.7, `connection.rs:118-126`).

`db_backup` chmods the destination `0600` and **refuses to overwrite** an existing
file. `db_restore` is VALIDATE-then-SWAP (CopyPaste-8wbt/crh3.6/crh3.2): Phase A
copies to a throwaway staging file, opens it with the **real Keychain device key**
(not the daemon's degraded-mode throwaway), runs `integrity_check` + schema sanity;
Phase B quiesces, moves the live DB aside, copies in, reopens, and rebuilds the r2d2
read pool so reads stop serving stale data. Any Phase-B failure rolls back. `force`
only controls whether the aside safety copy is deleted on success.

> **Amended for v2 (2026-07-30).** Phase A is binding and is implemented as
> written. **Phase B is not**: v2 replaces the restored tables' contents inside a
> single SQLite transaction over an `ATTACH`ed staging database instead of moving
> files. The reason is inodes — the store's r2d2 pool and `daemon/src/meta` both
> hold open connections to the live file, so a rename leaves them reading and
> writing an unlinked one, and "rebuild the read pool" does not reach the second
> connection at all. A transaction keeps every connection valid, makes rollback
> the database's job, and removes the `-wal`/`-shm` juggling a rename needs.
> Two consequences worth stating: `force` and the aside safety copy no longer
> exist (there is nothing to keep or delete), and `sync_device_state` is
> deliberately **not** restored — it holds this device's identity, its cloud
> session and its settings, and a restore must not make this device start
> claiming to be the one the backup came from. A backup holding a table this
> build does not know how to restore is refused rather than partially applied.
> Implemented once in `crates/copypaste-core/src/storage/dbfile.rs`; the daemon
> and embedded backends only adapt its result to their client contracts.

`reset_database` sets `ready = true` and clears `degraded_reason` in-place on
success — the daemon recovers without a restart. The `reset` field is retained
purely because the TypeScript `ResetDatabaseResult` interface reads `data.reset`.

---

### 4.12 Peers & discovery (`handlers_peers.rs`)

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 38 | `get_own_fingerprint` | none | `{fingerprint: S}` (mTLS **cert** fingerprint, colon-hex) | untagged `err`: `"P2P is disabled (set COPYPASTE_P2P=1): no mTLS certificate to advertise for pairing"` | no | `:10` |
| 39 | `get_own_device_info` | none | `{fingerprint:S\|null, device_name:S, device_model:S, os_version:S, app_version:S, local_ip:S\|null, public_ip:S\|null}` | — | no | `:52` |
| 40 | `list_peers` | none | `{peers: [PeerRow]}` | untagged `err` (`"failed to load peers: {e}"`) | no | `:79` |
| 41 | `poll_peer_events` | none | `{events: [{kind:"connected"\|"disconnected", fingerprint:S}]}` — **draining read**, empty array is valid | — | no | `:333` |
| 42 | `list_discovered` | none | `{devices: [{device_id:S, device_name:S, ip_addrs:A<S>, port:U, bport:U\|null, paired:B}]}` | untagged `err` (`"discovery not available (P2P disabled)"`) | no | `:351` |
| 43 | `rescan_discovered` | none | `{devices: […]}` (same shape as `list_discovered`) | untagged `err` (`"discovery not available (P2P disabled)"`, `"rescan failed to start: {e}"`) | no | `:437` |

**PeerRow** = serialised `PairedDevice` (`daemon/src/peers/model.rs:13`) —
`fingerprint:S, name:S, added_at:I, address:S|null, model:S|null, os_version:S|null,
app_version:S|null, local_ip:S|null, device_id:S|null, public_ip:S|null,
first_sync_at:I|null, last_sync_at:I|null, supabase_account_id?:S` — **plus**
daemon-injected fields:

| Injected field | Type | When present | Note |
|---|---|---|---|
| `online` | bool | always | Single source of truth = live P2P peer-sinks map; falls back to `last_sync_at` recency when P2P is off |
| `last_seen_secs` | int | always | |
| `latency_ms` | uint | only when P2P running **and** ≥1 ping-pong completed | |
| `rekey_failures` | uint | only when P2P running **and** ≥1 failure recorded | CopyPaste-ptgcc |
| `trust` | `"verified"` | always | Stable lowercase enum value; every persisted peer completed PAKE (CopyPaste-vypo) |
| `transport` | `"p2p"`\|`"relay"`\|`"supabase"` | omitted when unknown | Priority P2P (live sink) > relay > supabase (CopyPaste-1jms.32) |

**Removed before serialisation:** `password_file_enc`, `password_file_b64`
(CopyPaste-5lm — I11).

`list_peers` first awaits any in-flight QR-bootstrap responder task, with a 5 s
timeout, taking the handle out of its slot so only the first call blocks
(CopyPaste-7mf). Placeholder/test fingerprints (all-same-byte) are filtered out
with a warning; `peers.json` is never auto-deleted.

`list_discovered.bport` is `null` for v1 peers → the UI must disable "Pair".
`paired` is computed by cross-referencing the mDNS `device_id` (which IS the cert
fingerprint) against `peers.json` (CopyPaste-vgpy).

---

### 4.13 Pairing — LAN/SAS (`handlers_pairing_sas.rs`, `pairing_ops_flows_discovered.rs`)

| # | Method | Params | Success `data` | Errors | Source |
|---|---|---|---|---|---|
| 44 | `pair_with_discovered` | `device_id: S` **req** | `{ok: true, state: "initiating"}` | `invalid_argument` (`"missing param: device_id"`, `"P2P is disabled (set COPYPASTE_P2P=1): cannot pair over the network"`, `"discovery not available (P2P disabled)"`, `"peer does not advertise a bootstrap port (v1 peer): SAS pairing unsupported"`); `not_found` (`"device not currently discoverable: {id}"`, `"peer has no resolved IP address"`); **`rate_limited`** (`"another pairing is already in progress"`) | `sas.rs:13`, `flows_discovered.rs:29` |
| 45 | `pair_get_sas` | none | `{state: S}` plus optional `sas: S`, `role: S`, `peer_device_name: S`, `peer_ip_addrs: A<S>`, `peer_fingerprint: S` — **fields are omitted, not null, when unknown** | — (never errors) | `sas.rs:37` |
| 46 | `pair_confirm_sas` | `accept: B` **req** | `{ok: true, accepted: B}` | `invalid_argument` (`"missing or non-boolean param: accept"`, `"no pairing is awaiting SAS confirmation"`) | `sas.rs:68` |
| 47 | `pair_abort` | none | `{ok: true}` | — (idempotent, always succeeds) | `sas.rs:96` |

`pair_with_discovered` returns **immediately** with `state: "initiating"`; the UI
then polls `pair_get_sas` until `sas` appears, shows the SAS to the human, and calls
`pair_confirm_sas`. Only ONE pairing may be in flight at a time — the concurrent
case is `rate_limited`, the **only producer of that code in the whole daemon**.
The discovery path uses a fixed, well-known, NON-SECRET PAKE password
(`copypaste_p2p::DISCOVERY_PAIRING_PASSWORD`); the human SAS compare is the
authenticator, not the password.

---

### 4.14 Pairing — password/PAKE (`handlers_pairing_password.rs`)

| # | Method | Params | Success `data` | Errors | Source |
|---|---|---|---|---|---|
| 48a | `pair_peer_with_password` (`step` absent or `"initiate"`) | `peer_fingerprint: S` **req, validated**; `password: S` **req, ≥6 chars** | `{session_id: S, message1_b64: S}` | `invalid_argument` (`"missing peer_fingerprint"`, `"invalid peer_fingerprint format: {fp}"`, `"missing password"`, `"password must be at least 6 characters"`); `internal_error` | `:19` |
| 48b | `pair_peer_with_password` (`step = "finish"`) | `peer_fingerprint: S` **req**; `session_id: S` **req**; `message2_b64: S` **req** | `{ok: true, message3_b64: S, initiator_confirm_b64: S}` | `invalid_argument` (`"missing session_id for step=finish"`, `"missing message2_b64 for step=finish"`, `"invalid base64 in message2_b64: {e}"`, unknown session); `auth_failed` (PAKE failure); untagged `err` (peers.json load/save) | `:101` |
| 48c | `pair_peer_with_password` (any other `step`) | — | — | `invalid_argument`: `"unknown step '{other}'; expected 'initiate' or 'finish'"` | `:250` |
| 49 | `pair_accept_password` | *(ignored)* | — | **always** `not_implemented`: `"pair_accept_password is disabled — use QR pairing (pair_generate_qr / pair_accept_qr) (c4q2.20)"` | `:262` |
| 50 | `pair_accept_finish` | `session_id: S` **req**; `message3_b64: S` **req**; `peer_fingerprint: S` | `{ok: true, responder_confirm_b64: S}` | `invalid_argument`; `auth_failed` (confirm-tag mismatch, PAKE failure); untagged `err` (peers.json) | `:274` |

`step` defaults to `"initiate"` when absent.

---

### 4.15 Pairing — QR (`handlers_pairing_qr.rs`, `pairing_ops_flows_qr.rs`)

| # | Method | Params | Success `data` | Errors | Source |
|---|---|---|---|---|---|
| 51 | `pair_generate_qr` | none | `{qr: "cppair://pair?p=CPPAIR2…", expires_in_secs: U}` | untagged `err`: `"P2P is disabled (set COPYPASTE_P2P=1): cannot generate a pairing QR without an mTLS certificate to advertise"` | `qr.rs:19` |
| 52a | `pair_accept_qr` — **network/initiator mode** (`qr` param present) | `qr: S` | `{ok: true, peer_fingerprint: S}` | `invalid_argument` (P2P disabled, `"failed to decode pairing QR: {e}"`, address resolution); `auth_failed` (`"network PAKE pairing failed: {e}"`) | `flows_qr.rs:171` |
| 52b | `pair_accept_qr` — **relayed/responder mode** (`qr` absent) | `message1_b64: S` **req**; `peer_fingerprint: S` **req, validated** | `{session_id: S, message2_b64: S}` | `invalid_argument` (`"missing message1_b64"`, `"missing peer_fingerprint"`, `"invalid peer_fingerprint format: …"`, no/expired active QR token); `internal_error`; `auth_failed` | `qr.rs:190` |

`pair_generate_qr` advertises the mTLS **cert** fingerprint (CRITICAL-1 fix — the
device-key fingerprint is never compared by the mTLS allowlist so pinning it could
never authenticate). `expires_in_secs` is `PAKE_SESSION_TTL.as_secs()`; the QR
payload's own `expires_at` is `now + QR_PAIRING_TTL_SECS` (120 s), which must equal
`copypaste_p2p::bootstrap::BOOTSTRAP_ACCEPT_TIMEOUT`. Only **one** QR token is
active at a time — regenerating replaces it. Both the wrapped `cppair://pair?p=…`
deep link and a bare `CPPAIR2…` string are accepted on the way back in.

---

### 4.16 Pairing — unpair/revoke (`handlers_pairing_revoke.rs`)

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 53 | `pair_peer` | *(ignored)* | — | **always** `not_implemented`: `"pair_peer is disabled: use pair_peer_with_password (QR/password) or pair_with_discovered (LAN/SAS) for authenticated pairing"` | no | `:45` |
| 54 | `unpair_peer` | `fingerprint: S` **req** | `{ok: true, removed: B}` | untagged `err` (`"missing param: fingerprint"`, `"failed to load peers: {e}"`, `"failed to save peers: {e}"`) | no | `:54` |
| 55 | `revoke_peer` | `fingerprint: S` **req, validated** | `{ok:true, removed:B, revoked_at:I, fingerprint:S}` — **plus `sync_key_rotated: B`** when built with `cloud-sync` or `relay-sync` | `invalid_argument` (`"missing param: fingerprint"`, `"invalid fingerprint format: {fp}"`); `internal_error`; untagged `err` | **yes** | `:248` |
| 56 | `revoke_all_peers` | none | `{ok:true, revoked:U, cleared:U, revoked_at:I}` — empty store is a **success** returning `revoked: 0` | untagged `err` (`"failed to load peers: {e}"`); `internal_error` | **yes** | `:371` |

`revoke_peer` auto-rotates the sync key with `SyncKey::random()` (no passphrase
needed) when sync is active (CopyPaste-gbo). `unpair_peer` is the non-destructive
sibling: it removes the peer record without writing a `revoked_devices` audit row.
Both queue an offline unpair signal for delivery when the peer reconnects.

---

### 4.17 Import / export (`handlers_transfer.rs`)

| # | Method | Params | Success `data` | Errors | DB | Source |
|---|---|---|---|---|---|---|
| 57 | `import` | `items: A<O>` **req**; each item: `content_type: S` **req**, `content_bytes_b64: S` **req** (std base64, decoded ≤ 4 MiB), `created_at_ms: I` **req**, `metadata: O\|null` (optional), `is_sensitive: B` (optional, used only as a **floor**) | `{inserted: U, skipped: U}` | `invalid_argument` (`"missing param: items"`, `"param 'items' must be an array"`, `"item[{i}]: missing 'content_type'"`, `"item[{i}]: missing 'content_bytes_b64'"`, `"item[{i}]: invalid base64 in 'content_bytes_b64': {e}"`, `"item[{i}]: decoded payload {n} bytes exceeds max {MAX} bytes"`, `"item[{i}]: missing or non-integer 'created_at_ms'"`); `internal_error` | **yes** | `:31` |
| 58 | `export` | `limit: U` (def 0 = all; >0 = most-recent N returned oldest-first); `include_sensitive: B` (**def false**) | `{items: [{id, item_id, content_type, content_bytes_b64, created_at_ms, wall_time, lamport_ts, is_sensitive}], skipped_non_text: U}` | `internal_error` (`"export failed: {e}"`, `"blocking task failed: {e}"`) | **yes** | `:336` |

`import` is on the **large-payload allow-list** (16 MiB request line). A malformed
entry **aborts the whole batch** — no partial insert. Deduplication is
SHA-256 of the decoded bytes against rows inserted in the last 5 minutes.
PG-26: the caller-supplied `is_sensitive` is only a floor — the daemon always
recomputes sensitivity from the plaintext and ORs the two, so a tampered export
cannot smuggle a credential in as non-sensitive.

`export` **excludes sensitive items by default** (P2-tj9s) and silently skips
image/file items, counting them in `skipped_non_text` so the CLI can warn
(CopyPaste-93yr). Rows that fail to decrypt are skipped with a log line. The audit
log records the item COUNT only, never content.

---

> **Amended for v2 (2026-07-30).** `export`/`import` are implemented as
> `copypaste_ipc::Method::{Export, Import}`. The bodies differ from the table
> above because v2 is text-only and keeps no wire compatibility (CLAUDE.md rule
> 3): an item carries `content: String` rather than `content_bytes_b64`, so there
> is no base64 layer and no `MAX_IMPORT_ITEM_BYTES` — the per-item bound is the
> `max_item_bytes` setting, and the batch is bounded at 10 000 items.
> `skipped_non_text` is kept and still counted (a peer or the cloud can deliver a
> non-text type), and `skipped_sensitive` and `skipped_undecryptable` are added
> beside it, because "the export is shorter than the history" needs an answer for
> all three reasons rather than one. `include_sensitive` still defaults to false,
> and PG-26's floor-not-ceiling rule for `is_sensitive` is implemented by routing
> every imported item through the daemon's ordinary ingest path. Implemented in
> `crates/copypaste-daemon/src/server/transfer.rs`.

---

### 4.18 Streaming — `watch_subscribe` (`connection.rs:517`)

| # | Method | Params | Framing | Source |
|---|---|---|---|---|
| 59 | `watch_subscribe` | none | see below | `connection.rs:446` (interception), `:517` (loop) |

This is the **only** streaming method. It is intercepted in `handle_connection`
**before** `dispatch()` so the loop can own the write half. Its wire protocol is
**not** the `Response` envelope:

```
C→D  {"id":"<id>","method":"watch_subscribe","params":{}}\n
D→C  {"ok":true,"event":"subscribed","id":"<id>"}\n                        ← ack
D→C  {"ok":true,"event":"new_item","id":"<id>","item_id":"<uuid>",
      "content_type":"<type>","wall_time":<i64 ms>,"is_sensitive":<bool>}\n  ← 0..n events
```

> **Amended for v2 (2026-07-30).** v2's `watch` keeps the interception and the
> gate ordering and changes the framing: an event is an ordinary `Response`
> envelope carrying `ResponseData::Event`, with the subscription's `id` echoed,
> so there is one frame type and one decoder on each side rather than two. The
> payload is deliberately smaller — `{event, item_count}`, no item metadata —
> because a subscriber re-reads through the ordinary methods, which keeps one set
> of rules about what a client may see rather than letting the push channel
> become a second, laxer one. The ack is sent **after** the daemon has
> subscribed, so a client that has seen it cannot miss a change that follows.
> Two bounds are new and are stated in `server/watch.rs`: a subscriber is exempt
> from the 30 s read deadline (being silent is the point) and is counted against
> a separate cap of 8, so watchers cannot consume the 64-connection budget.
> Implemented in `crates/copypaste-daemon/src/server/{listener,watch}.rs`.

Rules:

* **Gates still apply.** The version + readiness gates run first, with
  `force_requires_ready = true` — `watch_subscribe` streams item metadata so it
  requires a ready DB even though it is deliberately absent from the `requires_db`
  allow-list (CopyPaste-crh3.105). A gate rejection is sent as a **normal
  `Response` envelope** and the connection is closed. Tested by
  `watch_subscribe_rejected_when_not_ready` (`tests.rs:7386`).
* Event frames carry **no** `protocol_version`, **no** `data` wrapper, and **no**
  plaintext/ciphertext — only the same metadata `history_page` exposes. Security
  posture = the 0600 socket, same as every other method.
* The `id` echoed in every event frame is the subscribe request's id, recovered
  as string → `as_i64` → `as_u64` → literal `"?"`.
* Broadcast `Lagged(n)`: log and **continue** — a slow consumer must never wedge
  the sender or crash the daemon.
* Broadcast `Closed` or write error/timeout: return cleanly, connection dropped.
* `new_item_tx == None` (degraded mode / tests without a channel): send the ack,
  then return immediately. The client sees EOF after the ack.
* Client fallback contract: if the daemon responds with an **error** to
  `watch_subscribe` (pre-CopyPaste-44rq.19 build), the CLI falls back to a polling
  loop (`cli/src/commands/watch.rs:174`). The CLI also verifies
  `ack["event"] == "subscribed"` (`watch.rs:269`) and ignores any event type other
  than `new_item` for forward-compat (`watch.rs:308`).
* `watch_subscribe` on one connection must not disturb one-shot requests on
  others: `watch_subscribe_does_not_break_concurrent_one_shot_requests`
  (`tests.rs:7771`).

---

## 5. Error codes

All codes are lowercase snake_case strings. Canonical definitions:
`crates/copypaste-ipc/src/response.rs:15-43` (`&'static str` constants) and
`crates/copypaste-ipc/src/error.rs:38-66` (typed `ErrorCode` enum). The two are
kept in lockstep by `error_code_matches_existing_str_constants` (`error.rs:187`).
`protocol.rs:99-104` **re-exports** them (CopyPaste-c4q2.30) rather than
re-declaring, so daemon and shared crate cannot drift.

| Code | Meaning | Produced by (non-exhaustive) |
|---|---|---|
| `not_found` | Requested resource does not exist | `copy`/`paste`/`copy_item`/`get_item_*` unknown id; `pair_with_discovered` device not discoverable / no resolved IP |
| `auth_failed` | Bad credentials, decrypt failure, PAKE confirm mismatch, missing Supabase account id | `copy*` decrypt; `get_item_image`/`get_item_file` decode; `cloud_sign_in`; `set_sync_passphrase`/`rotate_sync_key`/`revoke_and_rotate` (no account id); `pair_accept_finish`; `pair_accept_qr` network PAKE |
| `invalid_argument` | Structurally valid JSON that violates the parameter contract; also **JSON parse errors** | Every missing/mistyped param; malformed `history_page` cursor; `confirm != true`; bad UUID; bad fingerprint; unknown pairing `step`; oversized/undecodable import item; envelope parse error |
| `not_implemented` | Method recognised but disabled or not compiled in | `list`; `pair_peer`; `pair_accept_password`; all `cloud-sync`/`relay-sync` verbs when the feature is off |
| `ipc_not_ready` | Daemon still booting or degraded; the backing DB is not usable | Readiness gate for every `requires_db` method; `watch_subscribe`; `db_restore` when degraded **and** Keychain unreachable. Human message: `"daemon is still starting up; retry shortly"` (`consts.rs:365`) |
| `internal_error` | Unexpected daemon-side failure (I/O, join failure, SQLite) | Almost every handler's `spawn_blocking` join-error arm and most DB-error arms |
| `migration_in_progress` | v4 key-rotation sweep in flight; ingest paths reject writes rather than mix key versions | **Reserved — no producer exists in the current tree.** Only imported at `protocol.rs:101`. The CLI nevertheless implements full retry (see §6) |
| `version_mismatch` | `protocol_version` outside `[MIN..=CURRENT]` | `check_request_gates` only (`connection.rs:701`) |
| `rate_limited` | Caller exceeded a rate limit | **Exactly one producer:** `pair_with_discovered` when a pairing is already in flight (`pairing_ops_flows_discovered.rs:117`) |
| `daemon_offline` | Socket missing / connection refused | **Client-side only.** Deliberately NOT re-exported into the daemon (`protocol.rs:97-98`) — the daemon can never emit it |
| `request_too_large` | Request line exceeded the applicable cap; rejected before full buffering | IPC read path only (`connection.rs:67`). CopyPaste-c4q2.27 |

### 5.1 Untagged errors

`Response::err` produces `ok=false` + `error` with **no** `error_code` field. This
is legacy and clients must handle it. Known untagged sites the rewrite must decide
about (either keep byte-identical or fix behind a version bump):

* unknown method — `"unknown method: {m}"` (`handlers_items.rs:37`)
* invalid UTF-8 line — `"invalid UTF-8: {e}"`, **id hard-coded to `"0"`** (`connection.rs:425`)
* `get_app_icon` missing `bundle_id` (`handlers_items_ingest.rs:273`)
* `get_own_fingerprint` / `pair_generate_qr` when P2P disabled
* `list_discovered` / `rescan_discovered` when discovery unavailable
* `unpair_peer` missing fingerprint + all peers.json load/save failures
* `get_config` serialise failure; `set_config` `"invalid config: {e}"` and write failures
* `key derivation failed: {e}` on all three sync-key verbs
* `copy_item` pasteboard-write failure (whereas `copy`/`paste` tag it `internal_error`)
* `delete_all`, `pin_item`, `reorder_pinned`, `history_page`, `get_item_thumbnail`,
  `get_item_file`, `get_item_image` DB-error arms

`legacy_ipc_arms_return_error_code_on_failure` (`tests.rs:6467`) pins the subset
that *is* tagged.

---

## 6. Readiness, degraded mode & required client retry behaviour

### 6.1 The `requires_db` allow-list

`IpcServer::requires_db` (`connection.rs:83-128`) — exact set:

```
delete, count, search, copy, paste, copy_item, delete_all, delete_item, stats,
pin, pin_item, reorder_pinned, history_page, import, export,
get_item_image, get_item_thumbnail, get_item_file, add_file_item,
revoke_peer, revoke_all_peers, revoke_and_rotate,
db_stats, db_backup, vacuum
```

Plus `watch_subscribe` via `force_requires_ready = true`.

Deliberately **absent** (must work while `ready == false`):
`status`, `get_private_mode`, `set_private_mode`, `get_own_fingerprint`,
`get_own_device_info`, `list_peers`, `poll_peer_events`, `list_discovered`,
`rescan_discovered`, all `pair_*`, `get_config`, `set_config`, all `cloud_*`,
`get_sync_status`, `set_sync_passphrase`, `rotate_sync_key`,
`store_cloud_password`, `get_app_icon`, `list`, **`reset_database`**,
**`db_restore`**.

The last two are the recovery escape hatches — gating them would make degraded mode
unrecoverable.

### 6.2 Required client retry behaviour

| Condition | Client MUST |
|---|---|
| `ipc_not_ready` | Retry with **fixed** 500 ms backoff, **max 3** attempts (~1.5 s total), reconnecting each time; then fail with `"error [ipc_not_ready]: daemon is not ready after 3 retries — please try again in a few seconds"`. Cap is deliberately tight because a *persistently* degraded daemon returns this code forever. (`cli/src/ipc.rs:38,44,338-357`, CopyPaste-crh3.10) |
| `migration_in_progress` | Retry with **exponential** backoff 250 ms → 500 ms → 1 s → 2 s → 2 s (cap 2 s), **max 5** attempts, reconnecting each time; then fail with `"error [migration_in_progress]: daemon key-rotation is still in progress after 5 retries — please try again in a few seconds"`. (`cli/src/ipc.rs:17-23,304-333`, CopyPaste-ro0r) |
| `version_mismatch` | **MUST NOT retry.** Surface an upgrade prompt. (ADR-007 §Client guidance) |
| Response `protocol_version` > client's | MUST NOT silently continue: surface an upgrade prompt, refuse further mutating requests; read-only requests MAY continue at the client's discretion. |
| `rate_limited` | Back off and retry at the caller's discretion (the CLI does not auto-retry). |
| `request_too_large` | Surface "payload too large, reduce size" rather than a generic error; the connection is already closed. |
| EOF with no response | Treat as a read-timeout drop; reconnect. |
| Any other code | Propagate immediately, no retry. |
| Unknown `error_code` string | Do not crash. Keep and display the raw string (`raw_error_code`); `ErrorCode::parse` returning `None` is a normal forward-compat path. |

The two retry counters are tracked **independently** so a migration-then-restart
sequence cannot exhaust either prematurely (`cli/src/ipc.rs:299-303`).

Each retry **must reconnect** — the daemon may have closed the connection after an
error response.

---

## 7. Legacy / deprecated verbs (reference only)

v0.4.x kept these alive so old clients received a diagnosable answer instead of
`"unknown method"`. v2 has no obligation to preserve these verbs or responses.

| Verb | Status | Required behaviour |
|---|---|---|
| `list` | Deprecated c4q2.17 | `not_implemented` + `"list is deprecated: use history_page with {limit, offset} — the response shape is identical but pinned items appear first (c4q2.17)"`. No DB access. |
| `pair_peer` | Disabled | `not_implemented` + `"pair_peer is disabled: use pair_peer_with_password (QR/password) or pair_with_discovered (LAN/SAS) for authenticated pairing"`. Tested: `pair_peer_is_disabled_returns_not_implemented` (`tests.rs:6652`). |
| `pair_accept_password` | Disabled c4q2.20 (security) | `not_implemented` + `"pair_accept_password is disabled — use QR pairing (pair_generate_qr / pair_accept_qr) (c4q2.20)"`. |
| `copy`, `paste`, `delete`, `pin` | Live legacy aliases | Still fully functional with their own (different!) response shapes and error-code tagging. **`copy` and `paste` are the same handler.** `handlers_items.rs:17`. The `METHOD_LIST`/`METHOD_COPY`/`METHOD_DELETE` constants were removed from `copypaste-ipc` (CopyPaste-crh3.76) — these arms are dispatched by **string literal**, and that is intentional. |
| `get_own_fingerprint` | Superseded by `get_own_device_info` | Still returns `{fingerprint}`. |
| `stats` vs `db_stats` | Intentionally distinct | `stats` = rich CLI diagnostic; `db_stats` = minimal typed UI widget (c4q2.23). Do not merge. |

`not_implemented` message divergence (CopyPaste-c4q2.11): the **daemon's**
`Response::not_implemented` emits
`"feature not compiled in: {f} (rebuild the daemon with --features {f} to enable this method)"`
(`protocol.rs:180-189`, fix 44rq.14), while `copypaste_ipc::Response::not_implemented`
emits `"not implemented: {f}"`. The daemon's inline test
`response_not_implemented_uses_stable_code` asserts the long form. **The daemon's
long form is what ships on the wire.**

---

## 8. Bug IDs cited in-source and the rules they encode

| Bug ID | Rule |
|---|---|
| `CopyPaste-crol` | `Request/Response.id` is `String`, not `u64` — matches the live wire. |
| `CopyPaste-cbfl` | Parse-error responses MUST echo the client's `id` (string → i64 → u64 → `"?"`) so the CLI's id guard does not reject them. |
| `CopyPaste-c4q2.11` | The daemon's `Request::protocol_version` default (`1`) and `not_implemented` message are deliberate divergences from `copypaste_ipc` — do not collapse without a coordinated migration. |
| `CopyPaste-c4q2.19` | `CURRENT_PROTOCOL_VERSION` re-exports `copypaste_ipc::PROTOCOL_VERSION`; never a separate literal. |
| `CopyPaste-c4q2.30` | Daemon re-exports the `ERR_CODE_*` strings; a local copy is a wire-contract bug waiting to happen. `ERR_CODE_DAEMON_OFFLINE` intentionally NOT re-exported (client-only). |
| `CopyPaste-c4q2.28` | Two-pass, method-aware size cap: 64 KiB before the method is known, 16 MiB only for `import`/`add_file_item`. Prevents 64 × 16 MiB ≈ 1 GiB RAM amplification from same-UID clients. |
| `CopyPaste-c4q2.27` | Oversized payloads emit `request_too_large` so clients show "payload too large". |
| `CopyPaste-c4q2.24` | Every write is bounded by `IPC_WRITE_TIMEOUT` (10 s) — a client that never drains would otherwise pin a semaphore permit forever (same-UID local DoS). |
| `CopyPaste-cce1` | Every read is bounded by `IPC_READ_TIMEOUT` (30 s); a stalled client would otherwise hold a connection slot AND the DB mutex. |
| `CopyPaste-6ot5` | 64-permit semaphore with non-blocking `try_acquire_owned`; excess connections are dropped, not queued. |
| `CopyPaste-ah1m` | `flock(2)` on `<socket>.lock` around probe→remove→bind; fixes the TOCTOU where two starters both conclude "stale". |
| `CopyPaste-dl1e` | pid-recycle guard before SIGTERM-ing a predecessor daemon: never pid 0/1/self; validate exe path contains "copypaste"; re-probe after signalling. |
| `CopyPaste-crh3.105` | `watch_subscribe` MUST run the same version + readiness gates as `dispatch()`, with `force_requires_ready = true`. |
| `CopyPaste-crh3.10` | `ipc_not_ready` client retry: fixed 500 ms, max 3, then fail promptly. |
| `CopyPaste-ro0r` | `migration_in_progress` client retry: exponential 250 ms→2 s, max 5, reconnect each time. |
| `CopyPaste-crh3.7` | `db_backup` and `vacuum` MUST be readiness-gated; `db_restore` MUST NOT be. |
| `CopyPaste-crh3.6` / `crh3.2` / `8wbt` | `db_restore` is VALIDATE-then-SWAP against the **real Keychain key**, and rebuilds the r2d2 read pool. |
| `CopyPaste-c4q2.18` | `get_config` returns a structurally-secret-free `AppConfigResponse` built by exhaustive destructuring. |
| `CopyPaste-c4q2.25` | Daemon's `AppConfig` is a re-export of `copypaste_ipc::AppConfig`; the shadow struct was retired. |
| `CopyPaste-5lm` | `list_peers` MUST strip `password_file_enc` / `password_file_b64`. |
| `CopyPaste-vypo` | `list_peers.trust` is the literal lowercase `"verified"`, not `"Verified"`. |
| `CopyPaste-1jms.32` | `list_peers.transport` priority: live P2P sink > relay > supabase > omitted. |
| `CopyPaste-ptgcc` | `list_peers.rekey_failures` present only when P2P is running and ≥1 failure recorded. |
| `CopyPaste-7mf` | `list_peers` awaits the in-flight QR bootstrap (5 s cap) so the responder sees the fresh peer. |
| `CopyPaste-vgpy` | `list_discovered.paired` is derived from mDNS `device_id` (= cert fingerprint), not from a TLS handshake. |
| `CopyPaste-48k0` | `private_mode_epoch` in `status` and `get_private_mode` so pollers can detect changes without a subscription. |
| `CopyPaste-ruep` | `status.device_key_fingerprint` = hex SHA-256(X25519 pubkey); informational, NOT the pairing fingerprint. |
| `CopyPaste-a3nu` | `history_page` keyset cursor is a flat `{wall_time,id,pinned,pin_order}` object; a present-but-unparseable cursor is `invalid_argument`, never a silent fallback to page 1. |
| `CopyPaste-mnte` | `history_page` previews are NFKC-normalised and `sensitive_spans` index the returned preview. Batch preview fetch (one `IN (…)` query per page). |
| `CopyPaste-tteo` | `search` gained `kind` filter + `preview`/`pinned`/`kind` field parity with `history_page`. |
| `CopyPaste-93yr` | `export.skipped_non_text` so the CLI can warn instead of silently dropping image/file items. |
| `CopyPaste-tj9s` (P2) | `export.include_sensitive` defaults to **false**; audit-log the count only. |
| `CopyPaste-PG-26` | `import`'s caller-supplied `is_sensitive` is a floor only; the daemon recomputes from plaintext and ORs. |
| `CopyPaste-cb7u` / `jvzm.3` | `delete_all` is one transaction reusing the canonical single-item tombstone helper. |
| `CopyPaste-ojhe` | Tombstone/promote lamport = `max(existing + 1, now_ms)` — unified value space. |
| `CopyPaste-8ebg.2` | Promote-on-copy recomputes `expires_at` from the live `sensitive_ttl_secs` (0 = disabled sentinel). |
| `CopyPaste-44rq.19` | `watch_subscribe` streaming method + its exact event framing. |
| `CopyPaste-44rq.67` | `relay_url: Some("")` is the explicit-clear sentinel and shuts down the relay orchestrator at runtime; `None` is not a clear. |
| `CopyPaste-bjh` | `p2p_enabled` is persist-only; takes effect next restart. |
| `CopyPaste-gbo` | `revoke_peer` auto-rotates the sync key with `SyncKey::random()`; `rotate_sync_key`/`revoke_and_rotate` widened to `relay-sync` too. |
| `CopyPaste-c4q2.20` | `pair_accept_password` disabled as a security concern. |
| `CopyPaste-c4q2.17` | `list` deprecated in favour of `history_page`. |
| `CopyPaste-crh3.76` | Legacy verb arms dispatched by string literal, not by removed `METHOD_*` constants. |
| `CopyPaste-c4q2.10` | `map_content_type_to_uti` lives in `copypaste-ipc` as the single content_type→UTI source of truth. |
| `CopyPaste-8ebg.59` / `.65` | `MAX_IPC_REQUEST_BYTES` and `QR_PAIRING_TTL_SECS` are single-source-of-truth in `copypaste-ipc`; `copypaste-p2p` depends on it. |
| `CopyPaste-c4q2.2` | The `src-tauri` socket-path copy was collapsed onto `copypaste_ipc::paths::socket_path()`. |
| `CopyPaste-8ebg.4` | The long client budget covers work that outlasts a request. v2 set: see `Method::is_long_running` and §3.4's amendment. |
| `CopyPaste-liaz` | Retry exhaustion returns `Err`, never `process::exit` — `Zeroizing` destructors must run. |
| `CopyPaste-FEACLI-8` | Client keeps `raw_error_code` verbatim so unknown daemon codes are still displayed. |
| `CopyPaste-2l1e` | Exhaustive serde round-trip test over every `ErrorCode` variant, with a no-`_`-arm match to force updates. |
| `CopyPaste-crh3.8` | `ipc_not_ready`'s human message is a real sentence, not the constant name. |
| `CopyPaste-i5b` | `cloud_signed_in` reflects real GoTrue state, set/cleared by the IPC sign-in/out path. |
| `CopyPaste-merc` / `1jms.22` | `badge_state` is computed daemon-side (with the in-flight variant) so all platforms render the same value. |
| `CopyPaste-1jms.34` | `get_sync_status.supabase_account_id` is the non-secret stable identity, omitted when `None`. |
| `CopyPaste-yw2k` | The non-secret `supabase_account_id` is advertised in-band during pairing and persisted per-peer so cross-account mismatches are detectable. |
| `CopyPaste-M3` | `get_sync_status.email` is masked. |
| `CopyPaste-vp63.52` | `extract_str_param` consolidates missing-param handling but **only** where the wire response was byte-identical — sites with extra validation or untagged errors were left alone on purpose. |
| `CopyPaste-eq9m` / `z1xt` / `iqkm` | Media handlers do the whole decrypt+base64 inside `spawn_blocking`, free the decoded bytes early, and wrap key copies in `Zeroizing`. |
| `CopyPaste-crh3.86` | Read handlers go through `with_read_db` (pool → writer fallback). |
| `CopyPaste-j8p` | Read-only handlers use the r2d2 pool to bypass the write mutex. |
| `CopyPaste-crh3.100` | `cloud_sign_out` is persistent (Keychain + config cleared) but keeps URL + anon key. |
| `CopyPaste-16vr` | `cloud_test_connection` uses `SYNC_HTTP_TIMEOUT` (30 s). |
| `CopyPaste-1d5l.58/.59` | `MAX_FRAME_BYTES` (16 MiB), `SYNC_MAX_BLOB_BYTES` (8 MiB), `RELAY_MAX_ITEM_BYTES` (10 MiB) are three deliberately-distinct ceilings living in `copypaste-ipc`; do NOT collapse. |
| `CopyPaste-crh3.53` | `BackoffScheduler` lives in `copypaste-ipc` (pure `Duration` state machine, no I/O) so `copypaste-supabase` can reuse it without pulling in `copypaste-core`. |

---

## 9. Acceptance tests to re-create

Port these as **black-box wire tests against a running daemon**, not as internal
unit tests. Legacy sources: `crates/copypaste-daemon/src/ipc/tests.rs` (7 899
lines), `crates/copypaste-ipc/tests/{snapshot,wire_roundtrip}.rs`,
`crates/copypaste-cli/src/ipc.rs` inline tests.

### 9.1 Envelope & framing

1. `request_string_id_deserializes` / `response_string_id_wire_shape` — `id` is a
   JSON string, never a number. (`wire_roundtrip.rs:19,38`)
2. Byte-exact serialisation snapshots for `Request` and `Response` in declaration
   field order. (`snapshot.rs`)
3. `response_omits_none_fields` — `data`/`error`/`error_code` absent (not `null`)
   when unset. (`response.rs:161`)
4. `request_default_version_is_1` — omitted `protocol_version` ⇒ 1.
   (`protocol.rs:248`)
5. `response_carries_protocol_version` — present on success **and** error frames.
   (`protocol.rs:271`)
6. `unsupported_protocol_version_rejected_with_error_code` — pv 0 and pv 99 both
   yield `version_mismatch`. (`tests.rs:867`)
7. `parse_error_echoes_id_from_raw_json` and
   `parse_error_uses_fallback_id_when_not_valid_json`. (`tests.rs:7100,7121`)
8. `unknown_method_returns_error` — untagged `"unknown method: X"`. (`tests.rs:846`)
9. `ipc_oversized_request_rejected_not_crashed`,
   `ipc_non_bulk_method_over_small_cap_rejected`,
   `ipc_bulk_import_over_small_cap_accepted`,
   `import_oversized_request_returns_clear_error`.
   (`tests.rs:1362,1437,1476,6780`)
10. `ipc_write_timeout_is_bounded_and_not_longer_than_read`. (`tests.rs:95`)
11. `ipc_client_mid_request_disconnect_does_not_panic`. (`tests.rs:2149`)
12. `connection_cap_semaphore_exhaustion_returns_err` +
    `ipc_server_connection_cap_is_max_concurrent_connections`. (`tests.rs:6398,6432`)
13. `concurrent_clients_in_process_consistent_state`. (`tests.rs:2103`)
14. `spawn_blocking_does_not_block_tokio_worker`. (`tests.rs:2211`)

### 9.2 Socket lifecycle

15. `socket_path_env_override_wins`; per-platform default path assertions.
    (`paths.rs:113-192`)
16. `bind_does_not_mutate_process_umask`. (`tests.rs:1336`)
17. `is_socket_live_false_for_missing_path` / `_for_stale_regular_file`.
    (`tests.rs:499,509`)
18. `bind_with_stale_cleanup_removes_dead_socket_and_rebinds`,
    `_refuses_unidentifiable_live_socket`,
    `_refuses_to_steal_healthy_same_version_daemon`,
    `_attempts_eviction_for_different_version`, `_creates_lockfile`.
    (`tests.rs:552,606,625,673,6612`)
19. `probe_listening_daemon_reads_version_and_pid`. (`tests.rs:722`)
20. `pid_exe_is_copypaste_returns_none_for_dead_pid`, `pid_exe_path_resolves_own_pid`.
    (`tests.rs:757,773`)

### 9.3 Readiness / degraded mode

21. `dispatch_returns_ipc_not_ready_when_not_ready` — for the whole `requires_db`
    set. (`tests.rs:1544`)
22. `db_backup_and_vacuum_gated_in_degraded_mode`. (`tests.rs:7331`)
23. `watch_subscribe_rejected_when_not_ready`. (`tests.rs:7386`)
24. `status_returns_running` / `status_includes_device_key_fingerprint` /
    `status_includes_private_mode_field`. (`tests.rs:784,804,1159`)
25. `build_version_is_crate_version_prefixed`. (`tests.rs:533`)
26. CLI: retry-then-succeed on `migration_in_progress`
    (`call_retries_on_migration_in_progress_and_succeeds`, `cli/src/ipc.rs:555`);
    equivalent for `ipc_not_ready`; **no** retry on `version_mismatch`.

### 9.4 Method contracts (representative — port all)

27. `list_clamps_oversize_limit_to_max_page`,
    `history_page_clamps_oversize_limit_to_max_page`,
    `search_clamps_oversize_limit_to_max_page`. (`tests.rs:1607,1629,1657`)
28. `history_page_cursor_pagination_stable_under_concurrent_insert`,
    `history_page_rejects_cursor_missing_required_fields`,
    `history_page_rejects_cursor_with_wrong_typed_field`. (`tests.rs:1979,2055,2078`)
29. `history_page_adversarial_unicode_preview_no_panic`,
    `history_page_spans_index_into_returned_preview_not_raw`,
    `byte_to_char_offset_clamps_and_never_panics`. (`tests.rs:1695,1750,1820`)
30. `history_page_pinned_items_sort_first`,
    `history_page_unpinned_item_reverts_to_recency_order`,
    `history_page_items_include_pinned_field`. (`tests.rs:1868,1921,1833`)
31. `history_page_returns_device_name_for_known_origin`,
    `history_page_shows_file_preview`,
    `{list,history_page}_reports_too_large_to_sync_per_item`.
    (`tests.rs:3175,5730,3090,3119`)
32. `pin_item_*` / `delete_item_*` / `copy_item_*` missing-id / bad-UUID /
    unknown-id / happy-path matrix. (`tests.rs:3673-3839`)
33. `delete_all_tombstones_non_pinned_leaves_pinned_intact`. (`tests.rs:6848`)
34. `get_item_thumbnail_serves_thumb_and_null_sentinel`,
    `get_item_thumbnail_lazy_backfill_missing_thumb`. (`tests.rs:4811,4943`)
35. `get_item_file_round_trips_bytes_and_meta`, `get_item_file_rejects_non_file_item`.
    (`tests.rs:5625,5676`)
36. `export_limit_returns_most_recent_n_oldest_first`,
    `export_excludes_sensitive_by_default_and_includes_with_flag`,
    `export_skipped_non_text_count_is_non_zero_for_image_items`.
    (`tests.rs:4420,4547,7140`)
37. `build_config_response_strips_password_and_email`,
    `build_config_response_reports_unset_when_none`,
    `merge_config_preserves_omitted_secrets`, `merge_config_incoming_secret_overrides`,
    `set_config_with_redacted_shape_preserves_stored_password`,
    `merge_config_preserves_relay_clear_sentinel`,
    `update_core_config_clears_relay_url_on_empty_sentinel`,
    `merge_config_preserves_and_overrides_lan_visibility`.
    (`tests.rs:113,380,4088,4129,4348,6240,6282,6146`)
38. `db_stats_empty_database_returns_zero_count`, `db_stats_reports_correct_item_count`,
    `db_backup_missing_dest_returns_error`, `db_backup_creates_backup_file`,
    `db_backup_refuses_overwrite`, `db_restore_requires_confirm`,
    `db_restore_missing_file_returns_error`,
    `restore_{valid,wrong_key,corrupt,wrong_schema,into_empty_path}_*`,
    `restore_rebuilt_pool_sees_restored_data_while_stale_pool_does_not`.
    (`tests.rs:7048-7603`)
39. `list_peers_strips_password_file_fields`, `list_peers_includes_trust_field`,
    `list_peers_transport_absent_when_no_transport_active`,
    `list_peers_response_includes_online_and_last_seen_fields`,
    `list_peers_online_{true_when_recent,false_when_stale,true_from_live_mtls_allowlist}`,
    `list_peers_surfaces_rekey_failure_count`,
    `list_peers_surfaces_supabase_account_id`,
    `responder_list_peers_sees_peer_immediately_after_initiator_completes`.
    (`tests.rs:215,273,323,5055,5111,5166,5219,5305,5502,5978`)
40. `pair_get_sas_reports_idle_initially`, `pair_confirm_sas_without_pending_errors`,
    `pair_confirm_sas_missing_accept_errors`, `pair_abort_is_idempotent_and_succeeds`,
    `pair_with_discovered_missing_device_id_errors`,
    `pair_with_discovered_can_begin_twice_sequentially`,
    `pair_get_sas_includes_peer_fingerprint_when_available`.
    (`tests.rs:3331-3415,6685`)
41. `pair_peer_with_password_validates_inputs`, `_initiator_step_works`,
    `pair_accept_finish_rejects_unknown_session`,
    `pair_accept_finish_rejects_absent_initiator_confirm_tag`,
    `pair_qr_full_round_trip`, `pair_accept_qr_without_token_is_rejected`,
    `pair_accept_password_rejects_short_password`,
    `stale_pake_sessions_are_evicted_on_insert`, `pake_session_cap_rejects_excess`.
    (`tests.rs:2253-2805,3563,3603,3631`)
42. `pair_peer_is_disabled_returns_not_implemented`. (`tests.rs:6652`)
43. `revoke_peer_validates_and_records_audit_row`,
    `revoke_peer_auto_rotates_sync_key_when_active`,
    `revoke_all_peers_empty_store_succeeds`, `revoke_all_peers_revokes_every_peer`.
    (`tests.rs:2805,2908,3841,3875`)
44. `get_sync_status_reports_real_signed_in_flag`, `cloud_sign_out_clears_signed_in_flag`,
    `cloud_sign_in_returns_invalid_argument_when_not_configured`,
    `cloud_sign_in_out_return_not_implemented_without_cloud_feature`,
    `require_cloud_account_id_errors_without_account_and_ok_with`.
    (`tests.rs:3952,3997,4029,6932,3051`)
45. `private_mode_epoch_increments_on_every_set`,
    `set_private_mode_{enable_then_get,then_disable,missing_param_returns_error,updates_shared_atomic}`.
    (`tests.rs:6535,1069,1106,1142,1177`)
46. `watch_subscribe_receives_push_events`,
    `watch_subscribe_does_not_break_concurrent_one_shot_requests`,
    `watch_subscribe_client_disconnect_does_not_wedge_daemon`.
    (`tests.rs:7699,7771,7807`)
47. `legacy_ipc_arms_return_error_code_on_failure`,
    `ipc_responses_carry_machine_readable_error_code`. (`tests.rs:6467,915`)

### 9.5 Error-code contract

48. `error_code_serde_roundtrip_all_variants` with a no-`_`-arm match so a new
    variant breaks the build. (`error.rs:269`)
49. `error_code_matches_existing_str_constants`. (`error.rs:187`)
50. `error_code_from_str_unknown_returns_none` — including case sensitivity
    (`"NOT_FOUND"` ⇒ `None`). (`error.rs:179`)

---

## 10. Known-unjustified complexity we should NOT port

### 10.1 The wire contract is modelled three times

| Model | Location | Used by | Verdict |
|---|---|---|---|
| **A.** Typed DTOs | `crates/copypaste-ipc/src/{request,response,error}.rs` + `src/methods/*.rs` | The CLI/UI import `ErrorCode`, `METHOD_*`, `PROTOCOL_VERSION`, `paths::socket_path`, `AppConfig`, `AppConfigResponse` — and **nothing else** | **Keep. Promote to the single source of truth.** |
| **B.** Daemon's private copy | `crates/copypaste-daemon/src/protocol.rs:66-190` | The whole daemon | **Delete.** Fold its two real divergences into A (see below). |
| **C.** CLI's untyped poking | `crates/copypaste-cli/src/ipc.rs` + `src/commands/**` | 138 `.as_str()/.as_bool()/.as_u64()/.as_i64()/.as_f64()/.as_array()/.as_object()` calls across 20 files | **Delete.** Replace with `serde` deserialisation into A. |

Model B exists for exactly two reasons, both documented at `protocol.rs:44-64`:

1. `Request::protocol_version` must default to `1`, not `0`.
2. `Response::not_implemented` must emit the long "feature not compiled in …
   rebuild with `--features X`" message.

Both are trivially fixable in A: change `copypaste_ipc::Request`'s serde default to
`1` (which is *also* the correct behaviour for every other consumer — there is no
scenario where defaulting to `0` is right), and either move the long message into A
or keep a daemon-side constructor helper. `protocol.rs` itself says "When both
divergences are resolved in a future clean-up, delete the local definitions."
**The rewrite is that clean-up.** Net saving: ~290 lines + an entire class of
silent-drift bug.

### 10.2 The typed DTOs are already written — and almost entirely unused

The following DTOs exist in `copypaste-ipc` today, are documented, are correct
against the wire, and are **not referenced by any producer or consumer**. They are
schema documentation that the compiler does not enforce. Reusing them as the actual
serialisation types is nearly free:

| DTO | Definition | Wire method | Currently used? |
|---|---|---|---|
| `Request` | `request.rs:21` | all | Shape only; daemon uses its own copy |
| `Response` | `response.rs:52` | all | Shape only; daemon uses its own copy |
| `ErrorCode` | `error.rs:38` | all errors | **Yes** — CLI parses with it |
| `AppConfig` | `methods/config.rs:36` | `set_config` params | **Yes** — daemon re-exports it (c4q2.25) |
| `AppConfigResponse` | `methods/config.rs:171` | `get_config` data | **Yes** — `build_config_response` returns it |
| `StatsResponse` | `methods/clipboard.rs:65` | `stats` data | **No** — daemon hand-builds `json!` |
| `DbStatsResponse` | `methods/db.rs:23` | `db_stats` data | **No** |
| `VacuumRequest` / `VacuumResponse` | `methods/db.rs:55,71` | `vacuum` | **No** |
| `ResetDatabaseRequest` / `ResetDatabaseResponse` | `methods/db.rs:101,119` | `reset_database` | **No** — there is even a comment at `handlers_db.rs:166` explaining what `ResetDatabaseResponse` carries, next to a hand-built `json!` |
| `DbBackupRequest` / `DbBackupResponse` | `methods/db.rs:154,166` | `db_backup` | **No** |
| `DbRestoreRequest` / `DbRestoreResponse` | `methods/db.rs:206,223` | `db_restore` | **No** |
| `StoreCloudPasswordRequest` / `StoreCloudPasswordResponse` | `methods/sync.rs:65,74` | `store_cloud_password` | **No** — the handler defines a *local* `StoreCloudPasswordParams` struct with the comment "so the daemon does not need to depend on `copypaste-ipc`" — which is false, it already does |
| `GetSyncStatusResponse` | `methods/sync.rs:96` | `get_sync_status` data | **No** |
| `SyncBadgeState` | `methods/badge.rs:40` | `get_sync_status.badge_state` | Indirectly — via `compute_sync_badge_state_with_inflight`, then `to_value` |
| `PeerTransport` | `methods/pairing.rs:113` | `list_peers[].transport` | **No** — daemon inserts a raw `Value::String` |
| `PeerSyncHealth` | `methods/badge.rs:191` | — | Not an IPC type |

**Rewrite action:** make the 15 unused DTOs the actual `serde` types on both sides.
Every `serde_json::json!({...})` in a handler becomes `Response::ok(id, dto)`, and
every CLI `.as_str()` becomes a field access. That single change removes both model
B and model C.

**Caveats the rewrite must honour when doing this:**

* `list_peers[]` is genuinely dynamic today — a serialised `PairedDevice` with six
  fields *injected* post-hoc. It needs a real `PeerRow` DTO with `Option` fields and
  `skip_serializing_if`, not a `Value` map.
* `pair_get_sas` omits absent fields rather than emitting `null`. A DTO with
  `skip_serializing_if = "Option::is_none"` reproduces this exactly.
* `cloud_test_connection` returns a diagnostic object whose `ok` is **not** the
  envelope's `ok`. Keep that name (clients read `data.ok`), but the type is small
  and fixed — DTO it.
* `search` returns a **strict subset** of `history_page`'s item fields. Do not
  unify them into one type without checking the UI.

### 10.3 Hand-rolled framing

`ipc/connection.rs:299-493` hand-rolls what `tokio_util::codec::LinesCodec`
provides: `BufReader` + `read_until(b'\n')` + `.take(n)` limiting + manual `\n`/`\r`
trimming + manual UTF-8 validation + manual empty-line skipping. `tokio-util` is
**already a dependency** (`CancellationToken` is imported at `ipc/mod.rs:72`, and
`LengthDelimitedCodec` is used in `copypaste-p2p`).

`LinesCodec::new_with_max_length(n)` gives: max-length enforcement with a typed
`LinesCodecError::MaxLineLengthExceeded`, UTF-8 validation, `\n`/`\r\n` handling,
and a `Stream`/`Sink` pair — replacing roughly 150 lines.

**But the two-pass, method-aware cap (CopyPaste-c4q2.28) is real and must be kept.**
`LinesCodec` has one fixed max length; the daemon needs 64 KiB for unclassified
requests and 16 MiB only for `import`/`add_file_item`. Recommended shape:

* Framed with `LinesCodec::new_with_max_length(SMALL_REQUEST_BYTES)` by default.
* On `MaxLineLengthExceeded`, peek the already-buffered prefix for `"method"`; if
  it is on the allow-list, swap in a `LinesCodec` with the 16 MiB limit (or use a
  custom `Decoder` that carries both limits) and continue; otherwise emit
  `request_too_large` and close.
* This keeps the security property while deleting `extract_json_string_field`'s
  hand-written byte scanner (`connection.rs:15-46`) for everything except the
  one prefix peek — and that peek can be a small, well-tested helper rather than
  the load-bearing path for *every* request.

### 10.4 Other things not to port

* **The `"invalid UTF-8"` response hard-codes `id: "0"`** (`connection.rs:425`)
  instead of echoing, which trips the CLI's own id-mismatch guard (I2). Fix it —
  it's a bug, not a contract.
* **`Response::err` (untagged) has ~25 remaining call sites.** Tag every one. This
  is additive per ADR-007 (new `error_code` on a response that previously had none
  is a new optional field), so it does **not** need a version bump. Note that
  `extract_str_param` (CopyPaste-vp63.52) explicitly *left untagged sites alone* to
  stay behaviour-preserving — the rewrite has no such constraint.
* **Two `Response::not_implemented` messages** for the same code. Pick one.
* **`copypaste_ipc::Request::protocol_version` defaulting to `0`** — a latent
  footgun for any new consumer that uses the shared type as-is.
* **`migration_in_progress` has no producer.** Either wire the v4 sweep gate or
  drop the code from the enum (dropping is a wire change; keeping an unused
  reserved code is cheap — recommend keeping and documenting it as reserved).
* **`dispatch_items_extra`'s awkward position.** `get_app_icon` and
  `add_file_item` are "items" verbs stranded at the end of a nine-link
  chain-of-responsibility purely as an artefact of the god-file split (ra15.1 /
  ADR-017). A flat method→handler table (a `match`, a `phf` map, or generated
  dispatch) is clearer and removes nine files' worth of `_ => self.dispatch_next()`
  plumbing.
* **`REQ_COUNTER` / `next_id()` in the CLI is `#[allow(dead_code)]`** — every CLI
  call hardcodes `id: "1"`, and the Tauri bridge hardcodes `"ui-1"`. Either use
  real correlation ids (needed the moment anything multiplexes) or delete the
  machinery. Do not ship it half-wired again.

---

## 11. Recommendation: keep JSON-over-NDJSON, do not adopt tarpc/jsonrpsee

**Recommendation: keep the current JSON-over-Unix-socket, newline-delimited wire
format exactly as specified above, and make the typed DTOs in `copypaste-ipc` the
single source of truth on both sides.** Do not adopt tarpc or jsonrpsee.

### Why keep it

1. **Shipped installs pin this wire.** Homebrew casks, `dist/`, and `packaging/`
   ship daemon and UI/CLI that upgrade independently. ADR-007 exists specifically
   to make mixed-version pairs work; a transport change breaks *every* mixed pair
   at once with no graceful path — the daemon cannot even send `version_mismatch`
   because the client would not be speaking a protocol in which that frame exists.
2. **Three independent client implementations, one of which is TypeScript.**
   `crates/copypaste-ui/src/lib/ipc.ts` reads raw JSON. tarpc's wire format is
   bincode-over-`tokio-serde` — there is no TS story. jsonrpsee would work over TS
   but would still change the envelope (`jsonrpc`/`result`/`error{code,message}`
   with **numeric** codes) and lose the `ok`/`data`/`error_code`-string shape all
   three clients branch on.
3. **The protocol's hard parts are not RPC plumbing.** The genuinely load-bearing
   complexity is the readiness gate, degraded-mode allow-list, per-method size caps,
   the stale-daemon takeover probe, and `watch_subscribe`'s bespoke event framing.
   None of that is provided by tarpc or jsonrpsee; all of it would have to be
   re-implemented on top of them anyway.
4. **The actual pain is model duplication, not the transport.** §10.1/§10.2 remove
   ~90% of the maintenance burden without touching a byte on the wire.
5. **JSON is debuggable.** `nc -U ~/Library/Application\ Support/CopyPaste/daemon.sock`
   plus a hand-typed line is a supported debugging workflow and is what
   `probe_listening_daemon` does programmatically.

### What adopting a typed RPC framework would break

| | tarpc | jsonrpsee |
|---|---|---|
| Wire format | bincode over `tokio-serde` — **fully incompatible** | JSON-RPC 2.0 — envelope changes: `jsonrpc:"2.0"`, `result`/`error{code:i32,message,data}` replaces `ok`/`data`/`error`/`error_code` |
| TS UI client | No path — would need a full Rust bridge with no direct TS fallback | Workable, but every `reply.ok` / `reply.error_code` branch in `lib/ipc.ts` rewrites |
| CLI | Full rewrite (though it needs one anyway — §10.1 C) | Full rewrite |
| `protocol_version` | Replaced by tarpc's own service-version handshake; ADR-007's semantics (accept `[MIN..=CURRENT]`, `version_mismatch` code) would be re-expressed | Would live in a params/extension field or be replaced by JSON-RPC error `-32xxx`; the string `"version_mismatch"` disappears |
| `error_code` strings | Become enum variants over bincode — the append-only string contract (I8) and the CLI's `raw_error_code` forward-compat (unknown codes still displayed) both disappear | Become **numeric** codes; every consumer's `snake_case` string branch breaks; forward-compat on unknown codes gets worse, not better |
| Method-aware size caps | Would need a custom `Decoder` anyway | jsonrpsee has one global max request size, not per-method |
| `watch_subscribe` | Would become a tarpc stream — a real improvement, but the existing bespoke framing still has to be served for old clients | jsonrpsee subscriptions are a genuinely good fit (`subscribe`/`unsubscribe`/notification), but the frame shape changes |
| Stale-daemon takeover probe | `socket.rs:53` hand-writes a `status` line pre-runtime, before tokio owns the socket. A tarpc client would need a runtime at that point | Same problem, plus an HTTP/WS-shaped client for a Unix socket |
| Dependency weight | `tarpc` + `tokio-serde` + `bincode` | `jsonrpsee-server` + `-core` + `-types` + `hyper`/`soketto` — heavy for a local socket |
| Debuggability | Lost (binary) | Retained |

### If a break is ever taken

Do it as `PROTOCOL_VERSION = 2` with a **dual-listen** window: the daemon binds the
existing socket speaking v1 NDJSON and a second socket (or content-sniffs the first
line) speaking the new protocol, for at least one full release cycle. Anything less
strands users mid-upgrade with no diagnosable error.

### Concrete rewrite plan for the IPC layer

1. Move `Request`/`Response`/`ErrorCode` to be **the** types — delete
   `daemon/src/protocol.rs`'s copies; change `copypaste_ipc::Request`'s
   `protocol_version` default to `1`; keep the daemon's long `not_implemented`
   message as the single message.
2. Turn the 15 unused DTOs into the real serialisation types on both sides; add
   `PeerRow`, `SasState`, `CloudTestResult`, `DiscoveredDevice`, `HistoryItem`,
   `SearchItem`, `HistoryCursor`, `WatchEvent`, `StatusResponse` to complete the set.
3. Replace the CLI's 138 `.as_*()` pokes with `serde` deserialisation into those
   DTOs.
4. Replace the hand-rolled reader with a `LinesCodec`-based `Framed`, retaining the
   two-tier method-aware cap as a small custom `Decoder`.
5. Replace the nine-link dispatch chain with one flat table.
6. Tag every remaining untagged error; fix the `id: "0"` UTF-8 response.
7. Port the acceptance tests in §9 as black-box wire tests so the rewrite is proven
   byte-compatible before any client is touched.

Every one of these is invisible on the wire except item 6 (purely additive per
ADR-007). `PROTOCOL_VERSION` stays at `1`.
