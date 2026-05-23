# copypaste-ipc

## Purpose
Shared IPC wire types for the CopyPaste daemon, UI, and CLI. Pulls the JSON-over-Unix-socket protocol into a dedicated crate so every consumer speaks the same shape without depending on the daemon binary.

## Public API
From `src/lib.rs`:

- `Request` — incoming method call (`id: u64`, `method`, `params`, `protocol_version`).
- `Response` — outgoing reply (`id`, `ok`, optional `data` / `error` / `error_code`, `protocol_version`).
- `ErrorCode` — typed error code enum.
- Stable error-code constants: `ERR_CODE_AUTH_FAILED`, `ERR_CODE_INTERNAL_ERROR`, `ERR_CODE_INVALID_ARGUMENT`, `ERR_CODE_IPC_NOT_READY`, `ERR_CODE_NOT_FOUND`, `ERR_CODE_NOT_IMPLEMENTED`.
- `ImportItem` — round-trip type for `cli import` / daemon ingest.
- `PROTOCOL_VERSION: u32 = 1` — bump on breaking wire changes.

Lint discipline: `#![deny(missing_docs)]`, `#![deny(rust_2018_idioms)]`.

## Platform support
All platforms.

## Status
beta. Ships the new arch-2 wire shape (numeric `id`, explicit `protocol_version`). The daemon/UI/CLI currently still use the legacy `id: String` shape from `copypaste_daemon::protocol`; consumer migration is staged in later beta waves.

## Internal vs published
Internal workspace crate. Not published to crates.io.

## Quick example

```rust
use copypaste_ipc::{Request, Response, PROTOCOL_VERSION};

let req = Request {
    id: 1,
    method: "status".into(),
    params: serde_json::Value::Null,
    protocol_version: PROTOCOL_VERSION,
};
```

## Tests
1 integration test under `tests/`: snapshot-based wire-format check.

```bash
cargo test -p copypaste-ipc
```

## Related ADRs
- [ADR-002](../../docs/adr/ADR-002-unix-socket-ipc.md) — Unix-socket IPC.
- [ADR-007](../../docs/adr/ADR-007-ipc-protocol-versioning.md) — Protocol versioning.
