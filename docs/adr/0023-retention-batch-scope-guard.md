# ADR-0023: RetentionBatch is a handwritten scope guard

## Status

Accepted.

## Context

`RetentionBatch` (added in `b2d6ea5d`) is a typed proof-token that
accumulates state during an import batch and makes a conditional
sweep-or-skip decision on drop. `RetentionGate` is a debounce clock
for the sync source. AGENTS.md rule 1 requires evaluating a maintained
package before writing either.

## Evaluated

**`scopeguard` 1.2.0** — already a transitive dependency (via `lock_api`).
Provides `ScopeGuard<T, F>` which runs a closure on drop, and
`defer!`/`guard` macros.

`scopeguard` fits a stateless on-drop action. `RetentionBatch` is not
that: it carries a `swept` flag, a `disarm()` gate, and references to
the store and settings it will sweep. The DMY-156 correction adds a
further conditional decision — skip the sweep when pin restoration
failed — which is runtime state, not a fixed closure. Wrapping this in
`ScopeGuard<(Store, Settings, bool, bool), F>` would reproduce the
struct with an extra closure indirection and lose the `#[must_use]`
proof-token contract that `ingest_into_batched` relies on.

`RetentionGate` is a `Mutex<Option<Instant>>` debounce; `scopeguard`
has no debounce concept.

No other maintained crate on crates.io provides a conditional-proof-token
scope guard.

## Decision

Exemption 1: no maintained package provides the behaviour. The handwritten
`RetentionBatch` is 30 production lines of a typed struct with a
conditional `Drop`; replacing it with `scopeguard` would not reduce code
and would remove the type-level proof that a sweep is owed.
`RetentionGate` is a debounce timer, not a scope guard.

## Consequences

Both types stay in `crate::retention`. If a future change removes the
conditional logic and the proof-token contract, `scopeguard::guard`
becomes the simpler alternative and this ADR should be revisited.
