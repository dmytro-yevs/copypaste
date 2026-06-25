# ADR-011: Fate of copypaste-config + copypaste-telemetry in v0.3

## Status

Accepted — 2026-05-23

## Context

The v0.3 cleanup audit (`docs/audit/2026-05-23-cleanup-targets.md`) flagged two
workspace crates as "borderline": both compile, both have integration tests,
neither has any production consumer beyond its own tests. They must be either
wired in during v0.3, deleted, or explicitly frozen as strategic stubs.

### copypaste-config (425 LOC, commit `ed60088`)

Created in 0.2-beta as a third `AppConfig` struct centralising
paths/ports/log-level (`data_dir`, `socket_path`, `log_level`, `db_key_path`,
`relay_port`, `mdns_service`). Originally motivated by `arch-5` in
`docs/architectural-debt.md` (file since deleted; item: "merge `core::config` + `daemon::ipc`").

Current reality:

- `crates/copypaste-core/src/config.rs::AppConfig` — owns user-facing tunables
  (history limits, TTLs, image quality). Consumed by daemon + sync + bench.
- `crates/copypaste-daemon/src/ipc.rs::AppConfig` — owns IPC-mutable settings,
  serialised over the daemon socket. Consumed by daemon + CLI.
- `crates/copypaste-config::AppConfig` — owns paths/ports/log-level. **Zero
  production consumers.** Only its own `tests/load_save.rs` imports it.

The new crate **does not consolidate the two existing structs**; it introduces
a third orthogonal one. Wiring it in requires touching daemon, cli, ui, relay,
bumping the IPC protocol version (ADR-007), and rewriting tests. Estimated
effort: 1–2 weeks. Estimated user-facing benefit in v0.3: zero.

### copypaste-telemetry (343 LOC, commit `eef0155`)

Opt-in error reporter API surface — `ErrorReporter` trait, `NoopReporter`
(returns `Ok` for every event), `SentryReporter` stub (returns
`TelemetryError::NotImplemented`). Backed by `docs/privacy/telemetry-policy.md`
which is authoritative and explicitly states "the API surface ships so
downstream crates can pin to it" in 0.2-beta.

The audit recommends "defer one release, then delete if still unused". Privacy
policy commits to the API contract publicly. No runtime cost (NoopReporter is
the default and does nothing); no network I/O in any code path; lint-clean
(`#![forbid(unsafe_code)]`). v0.4 already implies a real Sentry backend will
ship, at which point daemon error paths become the natural first consumer.

The audit also flags two frozen-platform variants (`OsTag::Linux`,
`OsTag::Ios`) for removal as part of Stage 2; that work is orthogonal to this
decision and is tracked separately.

## Decision

### copypaste-config: DELETE

Remove `crates/copypaste-config/` and the corresponding workspace member entry
in the root `Cargo.toml`. The original motivation (`arch-5`) is **re-scoped**
to "consolidate `core::config::AppConfig` and `daemon::ipc::AppConfig` inside
the daemon crate" — those are the two structs that actually exist in
production. The third unified-paths struct is not the right shape for that
consolidation and carrying it across another release is technical debt.

Rationale:

- Three `AppConfig` types competing for the same name is worse than two.
- Wire-in cost (daemon + cli + ui + relay refactor + IPC version bump per
  ADR-007) is high; user-visible benefit is zero.
- Paths/ports/log-level today live where each consumer needs them
  (`daemon::paths`, `cli` clap args, `ui` Tauri invoke commands, `relay` clap args).
  None of these consumers have asked for unification.
- arch-5 remains a tracked debt item; v0.4 may revisit it with the right
  scope.

### copypaste-telemetry: KEEP

Keep the crate as-is for v0.3. It is a **strategic API stub** committed to in
a public privacy policy. Cost is bounded (343 LOC, zero runtime overhead,
zero network I/O). Removing and re-adding the API contract churns downstream
consumers we will add in v0.4.

Rationale:

- `docs/privacy/telemetry-policy.md` is authoritative and publishes the
  `ErrorReporter` API surface as a commitment. Deleting the crate breaks that
  commitment.
- `NoopReporter` is the default and is fully implemented. There is no risk of
  accidental data exfiltration — `SentryReporter` returns `NotImplemented`
  before any network call.
- v0.4 backend wiring (daemon error paths → Sentry) is a near-term beat;
  delete + reintroduce trades one release of dead code for two releases of
  API churn.
- Frozen-platform variant cleanup (`OsTag::Linux`, `OsTag::Ios`) is handled
  by audit Stage 2 and does NOT depend on this decision.

No `#[cfg(any())]` gate is added — the crate is intentionally compiled and
tested in CI so that the published API stays buildable.

## Consequences

### Positive

- **−425 LOC + −1 crate** (drops workspace member count 13 → 12).
- Eliminates "three AppConfig types" confusion permanently.
- Preserves the publicly-committed telemetry API contract.
- Unblocks v0.4 telemetry backend wiring with zero API drift.
- Reduces cognitive load for new contributors auditing the crate graph.

### Negative

- arch-5 ("merge `core::config` + `daemon::ipc`") remains open. We accept
  this debt; it is now correctly scoped to the daemon crate only.
- 343 LOC of compiled-but-unused telemetry code remains in the workspace
  through v0.3. Cost: ~0.5s additional `cargo check` time, negligible binary
  size impact (NoopReporter is monomorphised away in any consumer build).

### Neutral

- CHANGELOG entry needed for v0.3.0: "Removed: `copypaste-config` crate
  (never had production consumers)."
- `docs/audit/2026-05-23-cleanup-targets.md` Stage 4 entries for these two
  crates are now resolved; the audit doc itself need not be updated since it
  is a point-in-time snapshot.
- `docs/architectural-debt.md` arch-5 row (file deleted; row no longer
  actionable — arch-5 is re-scoped to daemon-internal consolidation, deferred
  to v0.4).

## Alternatives Considered

- **Wire copypaste-config into all four consumers in v0.3** — rejected.
  High refactor cost (IPC version bump, four consumer rewrites, test updates)
  for zero user-facing benefit. Defers real v0.3 work (cleanup, audit
  Stages 1–3) without solving the underlying arch-5 problem.
- **Keep copypaste-config behind `#[cfg(any())]`** — rejected. Hiding code
  from the compiler hides it from contributors too. A crate that exists but
  is never compiled is strictly worse than one that does not exist.
- **Delete copypaste-telemetry now, reintroduce in v0.4** — rejected. Breaks
  the published privacy-policy API commitment; creates two releases of churn
  for downstream consumers that we know are coming.
- **Wire copypaste-telemetry into daemon error paths in v0.3** — rejected.
  Out of scope for v0.3 (cleanup release). v0.4 owns the real backend; wiring
  the stub now means re-wiring it again when the backend lands.

## Implementation tasks

- [ ] `v3-config-delete` — Remove `crates/copypaste-config/` directory and
  its workspace member entry in root `Cargo.toml`. Update `Cargo.lock`. Add
  CHANGELOG entry. Run `cargo test --workspace` to confirm green.
- [x] `v3-arch-5-rescope` — `docs/architectural-debt.md` was deleted; arch-5
  re-scoped intent is captured in this ADR (consolidate the two daemon-internal
  AppConfigs only; defer to v0.4). No separate file update possible.
- [x] (separate, audit Stage 2) — `OsTag::Linux` and `OsTag::Ios` variants
  were removed from `crates/copypaste-telemetry/src/error.rs`; the enum now
  has `MacOs`, `Windows`, `Android`, `Unknown` only. Line references 79/85
  are stale (file changed). Telemetry crate version was bumped accordingly.
