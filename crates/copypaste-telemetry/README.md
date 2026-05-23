# copypaste-telemetry

## Purpose
Opt-in, privacy-first error reporting surface for CopyPaste. Ships only the trait + a no-op default in 0.2-beta so downstream crates can depend on the API today without locking in a backend.

## Public API
From `src/lib.rs`:

- `ErrorReporter` (trait) — `report(&self, event: ReportableError) -> Result<(), TelemetryError>`. Implementations MUST be non-panicking, non-blocking, and never read or write user payload beyond `ReportableError`.
- `ReportConsent` — `Disabled` (default), `EnabledMinimal`, `EnabledFull`.
- `NoopReporter` — accepts every event and discards it.
- `SentryReporter` — stub backend; always returns `TelemetryError::NotImplemented` in 0.2-beta.
- `init(consent) -> Box<dyn ErrorReporter>` — `Disabled` → `NoopReporter`; opt-in variants currently route to the stub.
- `TelemetryError` — `NotImplemented`, `BackendError(String)`.
- `ReportableError`, `OsTag` — anonymized event payload (`crate_name`, `version`, `error_class`, OS tag).

Lint discipline: `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`.

## Platform support
All platforms.

## Status
beta — API frozen, backend stubbed. No network I/O in any code path today.

## Internal vs published
Internal workspace crate. Not published to crates.io.

## Quick example

```rust
use copypaste_telemetry::{init, ReportConsent, ReportableError, OsTag};

let reporter = init(ReportConsent::Disabled);
let event = ReportableError::new("copypaste-core", "0.2.0-beta.0", "db.open.fail", OsTag::current());
let _ = reporter.report(event); // Noop: Ok(())
```

## Tests
1 integration test under `tests/` (`opt_out.rs`) plus inline unit tests covering default consent, noop reporter, and stub-error path.

```bash
cargo test -p copypaste-telemetry
```

## See also
`docs/privacy/telemetry-policy.md` — authoritative privacy policy (what may be sent, retention, user rights).
