# ADR-0023: The retention batch is a scopeguard, not a handwritten one

## Status

Accepted. Supersedes the exemption claimed in the first draft of this ADR,
which was wrong about what `scopeguard` provides.

## Context

`RetentionBatch` defers a batch of writes to one end-of-batch retention sweep.
It must carry the store and settings it will sweep, let the caller retire it
without sweeping when no rows were added, and never run SQLite in a destructor
while a panic unwinds. AGENTS.md rule 1 requires reaching for a maintained
package first.

## Evaluated

[`scopeguard`](https://crates.io/crates/scopeguard) 1.2.0
([docs](https://docs.rs/scopeguard/1.2.0/scopeguard/),
[source](https://github.com/bluss/scopeguard)) — already in `Cargo.lock` as a
transitive dependency of `lock_api`; MIT/Apache-2.0; no dependencies of its own;
`no_std`-capable pure Rust, so it builds unchanged on macOS, Android and
Windows. It covers all four needs:

- `guard_on_success` skips the closure while unwinding, so the panic policy is
  the guard's rather than a handwritten `thread::panicking()` branch;
- `Deref`/`DerefMut` expose the protected value, which is where the store and
  settings live;
- `ScopeGuard::into_inner` retires a guard without firing it, which is both
  `finish` and `disarm`;
- its constructors are `#[must_use]`.

Every claim above was checked against the vendored `scopeguard-1.2.0` source in
the local registry, not inferred from its README. No exemption is claimed, so no
survey of narrower guard crates is load-bearing here.

`RetentionGate` is a `Mutex<Option<Instant>>` debounce clock, not a scope guard,
and is out of this ADR's scope.

## Decision

Depend on `scopeguard` directly and define `RetentionBatch` as
`ScopeGuard<BatchScope, fn(BatchScope), OnSuccess>`. No rule 1 exemption is
claimed.

The cost is one direct dependency that was already compiled into every build,
adding no new crate to the tree, no build time and no audit surface. It removes
a `Drop` impl, a `swept` flag and the unwind branch.

## Consequences

`finish` and `disarm` are free functions in `crate::retention`, because
inherent methods cannot be added to a foreign type. Enabling `use_std` for
`guard_on_success` is a feature addition to an already-linked crate.
