# ADR-009: Relay Storage — In-Memory HashMap with TTL Eviction

- **Status:** Accepted
- **Date:** 2026-05-23
- **Track:** beta-w4 (relay durability)
- **Supersedes:** —
- **Related:** ADR-003 (SQLCipher at-rest, daemon-side), ADR-004 (SQLite WAL, daemon-side)

## Context

The `copypaste-relay` crate is the ephemeral pairing/handoff hub between
devices. Its responsibilities are narrow:

1. Hold a small list of registered devices (`device_id → bearer_token,
   public_key`).
2. Buffer encrypted clipboard items in a per-device inbox so that a
   sender can push while the recipient is offline, and the recipient
   can pull when it next reconnects.
3. Drop items that have aged out (default 24 h) — the source of truth
   for clipboard state lives on each device, not the relay.

The relay is **stateless from the user's perspective**: losing its data
is equivalent to a missed-while-offline window, not data loss. The
maximum item size is 10 MiB and a free-tier account is capped at 5
devices with 500 items per inbox, so total RAM footprint is bounded.

We need to decide how the relay should persist (or not persist) items
between requests, and how stale items should be evicted.

## Decision

**Use an in-memory `HashMap<DeviceId, RelayItem>` behind an
`Arc<Mutex<RelayStore>>`, with a background `tokio` task that prunes
items whose `inserted_at_unix + RELAY_TTL_SECS <= now_unix` every 60 s.**

- No on-disk storage. `RelayStore` lives entirely in process memory.
- TTL is `RelayConfig::sync_ttl_secs`, configured via the
  `RELAY_SYNC_TTL_SECS` env var (default `86_400` s = 24 h).
- The evictor is implemented in [`src/store.rs`](../../crates/copypaste-relay/src/store.rs)
  via [`spawn_ttl_evictor`]; the prune logic lives on
  `RelayStore::prune_expired` in [`src/state.rs`](../../crates/copypaste-relay/src/state.rs).
- Eviction uses a **server-recorded `inserted_at_unix`** rather than the
  client-supplied `wall_time`, so a malicious client cannot extend the
  lifetime of its data by sending a future timestamp.

## Alternatives Considered

### A. SQLite (with or without SQLCipher)
- Pros: durable across restarts, joinable, queryable.
- Cons:
  - The relay's data **has no value across restarts** — clients
    re-push on reconnect because they keep local copies.
  - Adds `rusqlite` build dependency on every relay host (relay is
    deployed as a small Linux binary; we'd like to avoid linking
    libsqlite3 / libsqlcipher).
  - Adds disk-IO latency and a failure mode (disk-full / corrupt-DB)
    that the rest of the relay design has no answer for.
  - A `db.rs` scaffold already exists in-tree (commit pre-dating this
    ADR) but was never wired into `main.rs`. **It is now dead code and
    can be removed** in a follow-up; this ADR explicitly chooses not
    to wire it in.

### B. Redis / external KV store
- Pros: durable, shareable across relay replicas, native TTLs.
- Cons: an additional service to deploy and monitor for zero functional
  gain — relay state is already disposable.

### C. In-memory `HashMap` without active eviction (rely on inbox cap)
- Pros: simplest possible code.
- Cons: a device that pushes one item every TTL period and never reads
  retains its inbox forever (up to the 500-item cap). Long-lived items
  also waste memory and increase the blast radius if the process is
  ever introspected (e.g. core-dumped).

### D. `dashmap` for finer-grained locking
- Pros: avoids the global `Mutex` on `RelayStore`.
- Cons: at our scale (≤ N×500 items where N≤5 per account), a single
  `Mutex` is not the bottleneck. The hot paths (`/devices/:id/items`
  push/pull) hold the lock for microseconds. Adding a dep is not
  justified.

## Consequences

### Positive
- Zero on-disk state → trivial deployment, easy to scale horizontally
  (each relay replica is independent; users hash-route or random-route
  to one).
- Restart wipes data → forces clients to re-push, which means a buggy
  client can't poison a long-lived relay inbox.
- TTL eviction makes worst-case memory bounded by
  `MAX_FREE_DEVICES × MAX_PUSH_ITEMS_PER_DEVICE × max_item_bytes` ≈
  5 × 500 × 10 MiB = 25 GiB per *account*, and clients prune their
  own pushes by deleting on ack, so the steady-state footprint is much
  smaller.

### Negative
- Items pushed while a recipient is offline for more than TTL seconds
  are lost forever. Clients must treat the relay as best-effort.
- A process crash drops all in-flight items.
- Horizontal scaling requires sticky routing per `device_id`
  (otherwise a push on relay A is invisible to a pull on relay B). For
  the beta the relay is single-instance, so this is deferred.

### Operational
- New env var: `RELAY_SYNC_TTL_SECS` (already existed; now actually
  honoured by eviction).
- The evictor tick interval is hardcoded at 60 s
  (`TTL_EVICTOR_TICK_SECS` in `main.rs`). Items live for at most
  `ttl + 60` s, which is acceptable for a 24 h TTL.

## Implementation Notes

- `SyncItem` gained a server-recorded `inserted_at_unix: u64` field. All
  existing `RelayStore` consumers were updated; the wire protocol
  (`PullItem`) is unchanged — `inserted_at_unix` is server-internal.
- `prune_expired(now_unix, ttl_secs) -> usize` takes the clock as an
  argument so tests can drive eviction with `tokio::time::pause` +
  `advance` without touching `SystemTime::now()`.
- `spawn_ttl_evictor` returns a `JoinHandle<()>` that the caller can
  abort; in `main.rs` it is intentionally bound to `_evictor` and left
  to run for the process lifetime.

## Follow-ups

- Remove the dead `crates/copypaste-relay/src/db.rs` scaffold (separate
  PR — out of scope for this task per branch-isolation policy).
- If we ever need multi-replica relay, add a Redis-backed
  `RelayStore` impl behind a trait and feature-gate it; do **not**
  retrofit SQLite for this purpose.
