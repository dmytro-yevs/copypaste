# copypaste-relay

## Purpose
Optional self-hostable HTTP relay for clipboard sync between devices that cannot reach each other directly (NAT, mobile networks). In-memory store with TTL eviction — relay is opaque to ciphertext.

## Public API
Binary-only crate (`src/main.rs`). Internal modules:

- `routes` — Axum router: `health`, `items` (POST/GET sync payloads), `devices` (list paired devices).
- `api` — `metrics` endpoint (Prometheus textfile).
- `auth` — token/key validation for clients.
- `middleware` — rate limiting, request logging.
- `state` — `RelayStore` (in-memory ciphertext map, see ADR-009).
- `store` — TTL evictor background task.
- `quota` — per-device and per-IP quotas.
- `config` — `RelayConfig` (port, TTL, quotas from env).
- `error`, `models` — wire types.

Default port: `7777`. TTL: `86400 s` (24 h).

## Platform support
All platforms (Linux container is the deployment target).

## Status
beta.

## Internal vs published
Internal binary crate. Not published to crates.io. Container image is the distribution unit.

## Quick example

```bash
COPYPASTE_RELAY_PORT=7777 cargo run -p copypaste-relay
curl http://localhost:7777/health
curl http://localhost:7777/metrics
```

## Tests
5 integration tests under `tests/`: auth hardening, end-to-end integration, metrics, rate limiting, store eviction.

```bash
cargo test -p copypaste-relay
```

## Related ADRs
- [ADR-009](../../docs/adr/ADR-009-relay-storage-choice.md) — In-memory store + TTL.
