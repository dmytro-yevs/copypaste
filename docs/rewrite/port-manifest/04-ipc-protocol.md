# Port Manifest 04 — Local IPC

CopyPaste v2 has one typed local IPC contract. `copypaste-ipc` owns request
methods, response payloads, error codes, limits and protocol metadata. The
daemon, CLI and Tauri bridge consume those types directly.

There are no alias commands, retired envelopes, string-dispatch escape hatches,
decode-only requests or dual-protocol listeners. A request either decodes as the
current typed contract or receives a bounded typed rejection.

## 1. Responsibilities

The IPC layer owns:

- owner-only local transport on macOS, Android development/test surfaces and
  Windows;
- bounded newline-delimited JSON framing;
- typed request correlation, methods, payloads and errors;
- protocol and readiness gates before dispatch;
- connection, watcher and timeout limits;
- a content-free change stream;
- path-redacted public failures.

Handlers own product behaviour. IPC validates shape and gates availability; it
does not reimplement storage, crypto, sync or presentation policy.

## 2. One authoring source

1. `Method` is a tagged enum. It owns command names and argument types.
2. `ResponseData` is a tagged enum. It owns successful payload association.
3. `ErrorCode` is a closed enum with exhaustive retry policy.
4. `Request`, `Response`, `PROTOCOL_VERSION` and frame/page limits live beside
   those enums.
5. Daemon routing is an exhaustive match on `Method`; no `_` arm turns a new
   method into runtime “unknown method”.
6. CLI and Tauri wrappers deserialize typed payloads. They do not claim an
   arbitrary result type for a string command.
7. TypeScript output is generated from the Rust owner and is never edited by
   hand.

One protocol number represents the current v2 contract. A different number is
a hard mismatch, not a hint to retry another decoder. The mismatch remains
useful when a stale v2 daemon and a newly launched client overlap on the same
machine; it does not imply support for another product.

## 3. Transport and framing

### 3.1 Local authentication boundary

The endpoint is local and owner-only. On Unix, the final socket is never
observable at a permissive mode: bind inside an owner-only staging directory,
set the final mode and rename atomically. On Windows, the named pipe applies the
equivalent current-user access control.

Filesystem paths are internal. Startup, bind and request errors shown to a user
must not contain the endpoint, data directory or username.

### 3.2 Endpoint lifecycle

The bind sequence is serialized. A live endpoint is never unlinked; a stale
socket is removed only after a failed connection probe while holding the bind
lock. This prevents two daemons from writing the same database.

### 3.3 Frames

- One request and one response are each a single UTF-8 JSON line.
- A maintained codec enforces newline framing, UTF-8 and the maximum frame
  length.
- The cap is applied before unbounded allocation or full JSON parsing.
- Empty, invalid UTF-8, malformed JSON and oversized frames cannot panic.
- A malformed request echoes a recoverable numeric id; otherwise id `0` is
  used. A successfully decoded request always receives its exact id.
- A reply is fully written under a deadline. A client that does not drain
  cannot hold a connection permit forever.

There is one frame cap. A large operation transports bounded typed data or uses
a product-level streaming design; it does not make the parser inspect a partial
JSON prefix to choose a larger allocation.

### 3.4 Timeouts

- Ordinary reads, writes and client calls have explicit deadlines.
- Long-running method classification is owned by `Method`, shared by every
  client and exhaustive for new variants.
- A watcher is exempt from the ordinary idle-read deadline, but not from write
  failure or cancellation cleanup.

### 3.5 Concurrency and liveness

- Total connections are bounded and acquired without queueing unbounded work.
- Watchers have a smaller independent cap because they intentionally outlive
  ordinary read deadlines.
- Blocking SQLite, AEAD and clipboard work runs off the async reactor. Network
  methods remain asynchronous.
- Disconnecting or cancelling a client cannot wedge the database or prevent the
  daemon from accepting later requests.

## 4. Capability catalogue

The typed method enum must cover these product capabilities. Exact request and
response fields remain in `copypaste-ipc`; this list prevents a typed rewrite
from silently dropping a feature.

| Area | Capabilities |
|---|---|
| Service | status, shutdown, content-free change watch |
| History | list by opaque cursor, full-text search, get, native copy, plain-text copy, bounded image preview, add, delete, delete-all ceiling, pin/unpin, reorder pins |
| Pairing and P2P | create invite, join, progress, confirm, cancel, unpair, revoke, peers, sync now, discovery and rescan |
| Transfer | export, import, backup and confirmed restore |
| Cloud | sign up/in/out, endpoint configuration, status and sync now |
| Settings | get/apply configuration and private-mode state |

A capability is removed by deleting its enum variant, handler, client wrapper,
UI surface, tests and feature-ledger ownership together. Keeping a disabled or
alias variant “just in case” is not removal.

### 4.12 Peer operations (stable section ID)

Peer-list and discovery backends implement the typed peer operations explicitly.
A backend that lacks the capability returns its typed unsupported state; it does
not fall through to an unrelated transport or command path.

## 5. Request and response contract

### 5.1 Correlation and shape

Every request carries a numeric id, current protocol number and exactly one
typed method. Every response carries the same id and either:

- `ok = true` with exactly one typed `ResponseData`; or
- `ok = false` with a path-redacted message and machine-readable `ErrorCode`.

Success never carries an error. Failure never carries successful data. Empty
success is an explicit payload variant, not an omitted value whose meaning a
client has to guess.

Unknown error-code strings may be preserved separately for diagnostic display,
but clients branch only on known enum variants. Unknown codes do not deserialize
as a known code and do not acquire a guessed retry policy.

### 5.2 Validation

- Required fields are required by deserialization unless the product contract
  explicitly defines a default.
- Ids, cursors, URLs, limits and enum values are validated before a handler
  mutates state.
- Page and content limits clamp or reject according to the shared typed limit
  policy; clients do not invent different maxima.
- A settings patch is all-or-nothing: one invalid value rejects the patch and
  leaves the effective configuration unchanged.
- Export excludes sensitive content unless the explicit include flag is true.
- After authentication, a legacy text body over the supported full-body limit
  `MAX_CONTENT_BYTES` is refused by get, native copy, plain-text copy and an
  included export as non-retryable `content_too_large`. Default sensitive and
  non-text export exclusions retain their filtering-before-authentication
  precedence. List, search and pin/unpin instead return a bounded preview
  marked `truncated`; the stored ciphertext is never altered.
- Restore requires affirmative confirmation before any replacement work.

### 5.2.1 Sync size-refusal count

Each successful P2P `SyncResult` carries `skipped_too_large` when the daemon
has final session statistics. It is the saturated count of locally withheld
outgoing live items that exceeded the P2P payload limit; it does not classify
replay, merge, crypto, protocol, or remote-ingress skips. A current successful
daemon sends zero explicitly only when it withheld no oversized items. Omission
means the daemon did not report a final count, including failed sessions, and
clients must present it as unknown rather than zero. On the wire, a present
value must be an unsigned 32-bit integer;
`null`, negative, fractional, string, and overflowing values are invalid.

### 5.3 Read-only versus side-effecting operations

Showing item detail uses a read-only operation. It must not write the plaintext
to the system clipboard. Native copy and plain-text copy are separate explicit
actions.

Watch events carry only the event kind and bounded counters such as live item
count, capture occurrence and auto-wiped count. They carry no item id, source
path, clipboard content or secret finding. Subscribers re-read through normal
typed methods, preserving one authorization and redaction path.

## 6. Readiness and shutdown

### 6.1 Readiness gate

Protocol validation happens before readiness validation. A client speaking the
wrong current protocol receives `protocol_mismatch`, even when the database is
not ready.

Every method is classified exhaustively as readiness-independent or
database-dependent. Status and shutdown remain answerable during startup so a
client can diagnose or stop a daemon that cannot finish opening. Pure in-memory
discovery/status operations may be exempt when their handlers truly do not
touch database-owned state. History, transfer, sync and persistence operations
return `not_ready` until their dependencies are usable.

There is no alternate degraded database mode. A method that requires the store
does not run against partial state.

### 6.2 Client retry and shutdown

`not_ready` and temporarily locked-key states are retryable with bounded
backoff; protocol mismatch, invalid request, authentication failure and unusable
key are not.

Shutdown acknowledges first, then begins graceful stop. It finishes or rolls
back in-flight atomic work and removes the endpoint. During the drain, protocol
validation still wins; status and shutdown remain usable, while no new mutable
request is admitted. The listener stays owner of its endpoint until accepted
work, final peer flush, and connection cleanup reach terminal outcomes. A
critical capture may outlive the cooperative five-second loop budget; a
permanent capture, blocking-task, peer-flush, or listener failure remains a
failed daemon exit after cleanup. A closed socket is not the only success
signal.

## 7. Error taxonomy

The shared enum distinguishes at least:

| Class | Meaning | Retry |
|---|---|---|
| `not_found` | item does not exist | no |
| `peer_not_found` | paired device does not exist | no |
| `invalid_request` | malformed or semantically invalid input | no |
| `protocol_mismatch` | client and daemon v2 contracts differ | no; prompt restart/update |
| `not_ready` | required service state is still starting | bounded retry |
| `auth_failed` | content authentication failed | no fallback |
| `key_locked` | keystore state is temporarily unavailable | bounded retry |
| `key_unusable` | stored secret cannot open this history | no |
| `content_too_large` | authenticated legacy text cannot fit a full bounded reply | no |
| pairing validation/limit/version errors | ceremony cannot proceed as requested | only the explicitly transient peer failures retry |
| `internal` | bounded unexpected failure | retry only where shared policy says so |

Messages are presentation-independent and path-free. Clients select friendly
copy from the code and preserve the raw message only for safe diagnostics.
When the shared client redactor sees an unquoted path, it redacts to that
line's end: whitespace cannot distinguish a filename tail from explanatory
prose. An unescaped closing quote is a boundary, so its following suffix is
retained. The redactor preserves LF/CRLF shape and leaves URLs or ordinary
slash prose alone.

Stable rule IDs used by source comments:

- **I9:** clients branch on the machine-readable error enum, never message text.
- **I14:** the owner-only local endpoint is the authentication boundary.
- **PG-26:** import re-runs detector, size and dedup policy and cannot smuggle a
  credential back in marked clean.

## 8. Method behaviour that must survive refactors

- List uses an opaque total-order cursor and is stable under concurrent inserts.
- Search scans the full FTS match set up to its bounded result limit and never
  returns sensitive items.
- Delete-all accepts a capture ceiling so an undo delay cannot delete items
  captured after the user's gesture.
- A full pinned ordering tolerates peers deleting or unpinning an id while the
  client holds the list.
- Item reveal is read-only and sensitive plaintext appears only after the
  client-owned explicit reveal gesture.
- Get, native copy, plain-text copy and included export refuse authenticated
  legacy text over the supported full-body limit before any clipboard write;
  list, search and pin/unpin retain bounded previews and protection controls.
- Export defaults to excluding sensitive items. Import reruns detector, size and
  dedup policy rather than inserting rows directly.
- Backup refuses overwrite. Restore validates current key, schema, integrity
  and sensitive index before durable replacement.
- Pairing persists nothing until both peers confirm the handshake-bound SAS.
- Unpair and revoke remain distinct: revoke bars the compromised pairing
  identity; unpair only forgets the local relationship.
- Settings responses never echo credentials.
- Watch coalescing may reduce refresh traffic but must preserve auto-wipe counts
  so unrequested deletion remains visible.

## 9. Acceptance tests

### 9.1 Type ownership

- Every `Method` variant is routed exactly once by an exhaustive match.
- Daemon, CLI and Tauri wrappers compile against the same request/result types.
- An unknown command literal and a mismatched result association fail typecheck.
- Generated TypeScript matches the Rust generation snapshot.
- Long-running and readiness classification are exhaustive; adding a method
  without deciding both fails compilation or a required test.

### 9.2 Framing and endpoint

- LF and CRLF frames decode; empty, malformed UTF-8 and malformed JSON reject
  without panic.
- The boundary-size frame succeeds and one byte over fails before handler work.
- Request ids are echoed on success, typed failure and recoverable malformed
  JSON.
- Socket/pipe permissions admit the owner and reject another local user.
- Two simultaneous starters yield one listener; the loser never unlinks the
  winner's endpoint.
- Stale endpoint cleanup succeeds under the bind lock.
- Slow-reader, slow-writer, idle-connection and watcher-cap tests release every
  permit.

### 9.3 Protocol and readiness

- The current protocol succeeds; lower, zero and higher values receive
  `protocol_mismatch` and are not dispatched.
- Status and shutdown answer before readiness.
- Every database-dependent method receives `not_ready` before its handler.
- Client retry policy retries only the shared transient set and never retries a
  protocol mismatch or authentication failure.
- Shutdown responds before the listener closes and leaves no stale endpoint.

### 9.4 Response and errors

- Every success payload round-trips through `ResponseData`, including empty
  collections whose element type cannot be inferred.
- Every `ErrorCode` has stable snake-case serialization and an exhaustive retry
  decision.
- Unknown error strings remain unknown and preserve their raw spelling.
- No failure response contains `/`, `\\`, a home-directory fragment, drive
  path, UNC path or username.
- Success cannot serialize error fields; failure cannot serialize data.

### 9.5 Product methods

- Cursor paging is stable under concurrent capture and rejects invalid cursors.
- List/search clamp shared limits and never expose tombstones or forbidden
  sensitive index content.
- Get/reveal has no clipboard side effect; native and plain-text copy exercise
  their distinct backends.
- Delete-all ceiling preserves a capture arriving during the undo window.
- Pin reorder tolerates unknown ids and keeps a total order.
- Export/import sensitive defaults and skip counts are exact.
- Backup/restore failure leaves current data and pool untouched.
- Pairing covers invite, join, bound SAS confirmation, cancel, timeout, unpair
  and revoke without alternate command paths.
- Watch receives items/peers changes, includes capture and swept counts, carries
  no content and survives concurrent one-shot requests.
- Configuration redacts secrets and rejects an invalid patch atomically.

## 10. Load-bearing choices

- Keep maintained framing and serialization packages; do not hand-roll byte
  scanners or parallel DTOs.
- Keep one flat typed dispatch owner. Feature modules may implement handlers but
  may not invent command strings or response maps.
- Keep endpoint permissions, bind locking, connection caps, deadlines and
  content-free events. They are security and liveness properties, not framing
  trivia.
- Treat a second protocol or alias command as a new product surface requiring a
  decision, UI ownership and acceptance tests. Do not add one as compatibility
  scaffolding.
