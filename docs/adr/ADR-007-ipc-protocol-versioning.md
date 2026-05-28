# ADR-007: IPC Protocol Versioning

## Status

Accepted

Date: 2026-05-23
Scope: `copypaste-daemon` IPC wire format (`crates/copypaste-daemon/src/protocol.rs`)
Supersedes: none
Related: ADR-002 (Unix socket IPC)

## Context

The alpha line (v0.1.x) shipped its IPC `Request` / `Response` structs without
any explicit version field. As long as the daemon and every client were built
from the same commit, this was fine. It stops being fine the moment we ship
beta:

* The CLI, the tray, the Tauri UI, the Android relay client, and third-party
  scripts all talk to the daemon over the same Unix socket.
* They are upgraded independently. A user can run a newer daemon with an
  older CLI (or vice versa) for hours or days before noticing.
* Beta is the first release where we expect external integrations, so silent
  schema drift will produce bug reports that are very hard to triage
  ("status returned wrong fields" with no way to tell which side is stale).

Without a version field, the daemon cannot reject a clearly incompatible
client cleanly — it falls into `serde` parse errors or wrong-shape responses,
which clients then surface as generic "IPC error".

## Decision

Every `Request` and `Response` on the IPC wire carries an integer
`protocol_version` field.

* `CURRENT_PROTOCOL_VERSION` (`u32`, starts at `1`) is the version this build
  of the daemon produces and accepts.
* `MIN_SUPPORTED_PROTOCOL_VERSION` (`u32`, starts at `1`) is the inclusive
  lower bound the daemon will still service. The daemon accepts any version
  in `[MIN_SUPPORTED_PROTOCOL_VERSION ..= CURRENT_PROTOCOL_VERSION]`.
* On `Request`, the field is `Option<u32>` via `#[serde(default = ...)]` —
  if the client omits it, the daemon treats it as `1`. This keeps alpha
  clients working until they upgrade.
* On `Response`, the field is **always** serialised, even on error paths.
  Clients use it to detect a daemon-side downgrade or rollback.
* Requests outside the supported window are rejected with
  `error_code = "invalid_argument"` and a human-readable message naming the
  unsupported version and the daemon's supported range. (We considered a
  dedicated `version_mismatch` code and reserved it for future use, but
  reusing `invalid_argument` keeps the client-side branching surface small
  for v1 — clients only need one "stop and prompt the user to upgrade"
  handler, keyed on the message prefix `unsupported protocol version`.)

### Versioning policy

The protocol version is **only** bumped on changes that break wire
compatibility:

* Renaming or removing a field on `Request` / `Response` / params / data.
* Removing a method or changing its semantics in a way old clients can't
  detect.
* Changing the type of an existing field (e.g. `u32` → `String`).

The following changes are **backwards-compatible** and DO NOT bump the
version:

* Adding a new method.
* Adding a new optional field (defaulted on deserialize).
* Adding a new `error_code` variant.
* Adding a new field to a response `data` object (clients ignore unknown
  fields).

This mirrors semver-major intent at the protocol layer.

### Client guidance

A client receiving a response whose `protocol_version` is higher than what
it was built against MUST NOT silently continue:

1. Surface an upgrade prompt to the user.
2. Refuse to send further mutating requests (`save`, `delete`, `pin`, …).
3. Read-only requests (`status`, `list`) MAY continue at the client's
   discretion if the response shape is still parseable.

A client receiving `error_code = "invalid_argument"` with a message
beginning `unsupported protocol version` MUST NOT retry — this is a hard
mismatch that requires a daemon or client upgrade.

## Consequences

**Positive:**

* Daemon and clients can be upgraded independently with a clean,
  deterministic failure mode at the boundary.
* External integrators have a documented contract they can pin against.
* Future breaking changes (e.g. switching `id: String` to `id: u64`) become
  shippable without coordinated lock-step releases.

**Negative:**

* Every `Response` now carries one extra `protocol_version: 1` field
  (~25 bytes per response). Acceptable given the local-socket transport.
* Tests and fixtures that hand-craft JSON requests/responses must be aware
  of the field, though `#[serde(default)]` keeps most fixtures green
  without changes.

**Neutral:**

* `CURRENT_PROTOCOL_VERSION` will need a bump in the same PR as any
  breaking IPC change. A CI lint to enforce this is out of scope for
  beta-w1.
