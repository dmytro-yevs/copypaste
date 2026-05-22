# Relay v2 — Rate Limiting & Device Quotas

## Overview

This document records the implementation decisions for rate limiting and device
quota enforcement added to `copypaste-relay` in the `feature/relay-v2-quotas`
branch.

---

## Rate Limiting

### Crate choice: `tower_governor` 0.4

`tower_governor` was already present in `Cargo.toml` (added during Phase 2 for
basic per-IP protection).  It wraps the `governor` 0.6 GCRA (Generic Cell Rate
Algorithm) implementation and integrates with axum as a `tower::Layer`.

**Why GCRA over sliding window?**  GCRA provides smooth rate limiting with
configurable burst — no "reset storm" at window boundaries.

### Router split: exempt vs. rate-limited routes

`/health` and `/stats` are diagnostic endpoints that must remain available even
under load.  They are mounted on a separate `Router` that has **no**
`GovernorLayer` attached.

```
Router::new()
  .merge(exempt_router)   ← /health, /stats — no limit
  .merge(item_routes)     ← per-device (60/min) + per-IP (200/min)
  .merge(device_routes)   ← per-IP (200/min) only
```

### Per-IP limit: 200 requests/minute

Parameters:
- `per_second(3)` — refill 3 tokens/s → 180 req/min steady-state
- `burst_size(60)` — allow short bursts up to 60 tokens
- Effective capacity before throttle: 180 + 60 = 240 req/min → covers 200 req/min target

### Per-device limit: 60 requests/minute

Parameters:
- `per_second(1)` — refill 1 token/s → 60 req/min steady-state
- `burst_size(20)` — allow short bursts

**Key: why per-IP for "per-device"?**  `tower_governor` 0.4 uses the client IP
as its key extractor by default.  A true per-device key would require reading
the `Authorization` header inside the middleware layer — not supported without a
custom `KeyExtractor`.  Since each device connects from a (roughly) stable IP in
practice, the per-IP tight limit on device-scoped routes (`/devices/:id/items`)
provides equivalent protection without added complexity.  This is explicitly
documented in `middleware/rate_limit.rs`.

### Response on limit exceeded

`tower_governor` automatically returns:
- `HTTP 429 Too Many Requests`
- `Retry-After: <seconds>` header

No custom error handler needed.

---

## Device Quotas

### Architecture: in-memory (RelayStore) + SQLite schema

The relay uses an in-memory `RelayStore` (not SQLite at runtime) for low-latency
item delivery.  Quota enforcement is applied directly in `RelayStore` methods.

SQLite (`db.rs`) has a new `device_quotas` table (with `tier`, `quota_override_*`
columns) for future persistence/admin use — it is not wired to the in-memory
store yet.

### Tier model (`quota.rs`)

```
Tier::Free   max_devices=5    max_history=Some(1000)   text=1MiB  image=10MiB
Tier::Pro    max_devices=10   max_history=None          text=1MiB  image=10MiB
```

`Tier::Free` is the default for all new device registrations.

### Enforcement points

| Quota | Where enforced | Error returned |
|-------|---------------|----------------|
| Max devices | `RelayStore::register_device_with_tier` (before insert) | `403 DEVICE_QUOTA_EXCEEDED` |
| Item size | `routes/items.rs::upload` (before state lock) | `413 ITEM_SIZE_EXCEEDED` |
| History per inbox | `RelayStore::upload_item` (after token verify) | item silently dropped |

**Why silent drop for history quota?**  Consistent with the existing
`MAX_ITEMS_PER_DEVICE` hard-cap eviction behaviour.  Raising an error would
require the sender to know which recipient inboxes are full — not feasible in
the current fan-out model.

### Item size: conservative pre-check at HTTP layer

`routes/items.rs` checks the decoded ciphertext size against `Tier::Free` limits
before acquiring the store mutex.  This means:
- Free devices: `text` ≤ 1 MiB, `image` ≤ 10 MiB
- Pro devices: same limits at HTTP layer (safe, conservative)

A future enhancement could look up the sender's tier from the bearer token and
apply tier-specific limits.

---

## Files changed

| File | Change |
|------|--------|
| `src/quota.rs` | New — `Tier` enum, `check_device_quota`, `check_item_size`, `check_history_quota` |
| `src/middleware/mod.rs` | New — middleware module declaration |
| `src/middleware/rate_limit.rs` | New — rate limit constants + smoke tests |
| `src/error.rs` | Add `DeviceQuotaExceeded`, `ItemSizeExceeded`, `HistoryQuotaExceeded` variants |
| `src/state.rs` | Add `tier: Tier` to `DeviceRecord`; add `register_device_with_tier`; enforce history quota in `upload_item` |
| `src/routes/mod.rs` | Split router into exempt + rate-limited sub-routers; wire per-IP and per-device `GovernorLayer` |
| `src/routes/items.rs` | Replace hard-coded 10 MiB check with `quota::check_item_size` |
| `src/routes/devices.rs` | No change needed — `register_device` delegates to `register_device_with_tier` |
| `src/main.rs` | Declare `mod middleware; mod quota;` |
| `src/db.rs` | Add `device_quotas` table to SQLite schema |
| `tests/integration.rs` | Add `#[path]` import for `quota` module |

---

## Test count

```
108 tests total, all passing
```

Breakdown:
- `quota.rs` unit tests: 17
- `state.rs` unit tests (existing + new quota/tier tests): 17
- `error.rs` unit tests (existing + new error variant tests): 9
- `config.rs` unit tests: 2
- `middleware/rate_limit.rs` unit tests: 4
- `db.rs` unit tests: 2
- Integration tests (`tests/integration.rs`): 8
- Subtotal relay-only: 59 (new quota-related: ~25 new tests added)
