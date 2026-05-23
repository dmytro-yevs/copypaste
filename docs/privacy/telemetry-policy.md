# CopyPaste Telemetry Policy

_Last updated: 2026-05-23 (0.3.0-dev)_

CopyPaste ships an **opt-in, privacy-first** error reporter. This document is
the authoritative description of what the reporter does, what it sends, and
the user's rights. It binds the implementation in
[`crates/copypaste-telemetry`](../../crates/copypaste-telemetry).

## TL;DR

- Reporting is **OFF by default**. We never enable it for you.
- We never send clipboard contents, file paths, device IDs, IPs, account
  emails, or any free-form text.
- In 0.3-dev the real Sentry SDK is wired behind `SentryReporter`. It only
  ever fires when the caller passes an `Enabled*` consent value **and** a
  DSN. The default path (`init(consent)` with no DSN, or `Disabled` with a
  DSN) still performs zero I/O.

## Defaults & consent

The reporter accepts one of three consent values:

| Value             | Behaviour in 0.3-dev                                                                 |
|-------------------|--------------------------------------------------------------------------------------|
| `Disabled`        | Default. Returns a `NoopReporter` that discards every event. No SDK init, no network. |
| `EnabledMinimal`  | Returns a `SentryReporter` that dispatches scrubbed events to the configured DSN.    |
| `EnabledFull`     | Same wire shape as `EnabledMinimal` in 0.3-dev; reserved for future opt-in extras.   |

`init(consent)` returns `NoopReporter` for every consent value because no
DSN is supplied. Network reporting is only reachable via `init_with_dsn`
(or by constructing `SentryReporter` directly) with an `Enabled*` consent.

The user-facing surfaces (CLI, UI, daemon) are responsible for surfacing the
consent prompt and persisting the choice. The crate itself never reads or
writes any consent state on disk.

## What may be collected (when opted in)

Only the fields on `ReportableError`:

- `crate_name`: which CopyPaste crate raised the error (e.g.
  `copypaste-daemon`).
- `crate_version`: the semver of that crate.
- `error_class`: a short developer-defined taxonomy string
  (e.g. `keychain.read_failed`).
- `os`: a coarse platform tag — one of `macos`, `linux`, `windows`,
  `android`, `ios`, `unknown`. No version, no build, no hostname.

There is intentionally **no free-form message field**. Adding one requires
updating this policy first.

## What is never collected

- Clipboard contents, history entries, or hashes thereof.
- File paths, environment variables, working directory.
- Device identifiers, MAC addresses, IP addresses.
- Account emails, display names, or any identifier tied to a person.
- Crash stack traces, backtraces, or panic messages.
- Network endpoint URLs.

## Backend

When `EnabledMinimal` or `EnabledFull` is selected **and** a DSN is
provided, events are dispatched via the official `sentry` Rust SDK
(v0.34, crate-local — not promoted to the workspace) to the configured
Sentry endpoint (sentry.io SaaS, or any Sentry-compatible relay if the
DSN points elsewhere).

SDK configuration is locked at construction time:

- `send_default_pii = false` — Sentry's automatic IP / user-id capture
  is **off**.
- `traces_sample_rate = 0.0` — no performance / tracing samples.
- `attach_stacktrace = false` — no automatic backtrace capture.
- `release = sentry::release_name!()` — server-side grouping only.

The PII scrubber runs **before** the SDK is consulted. With
`Disabled` the report is dropped before both the scrubber and the SDK,
so a disabled reporter is observably a true no-op.

## What is sent (wire shape)

When opted in, each `report()` produces exactly one Sentry message of
level `Error` with a body of the form:

```
<crate_name>@<crate_version> [<os>] <error_class>
```

That string is the entirety of the payload. Concretely:

- `crate_name` — e.g. `copypaste-daemon`
- `crate_version` — e.g. `0.3.0-dev`
- `os` — coarse tag: `MacOs`, `Linux`, `Android`, `Ios`, `Unknown`
  (Windows is frozen — see ADR-012). No version, no build, no hostname.
- `error_class` — developer-defined taxonomy string, scrubbed through
  `PiiScrubber` before send.

No IP address, no user identifier, no timestamp at finer than the hour
Sentry itself records server-side. No breadcrumbs, no contexts, no
attachments.

## Retention & sharing

- **Storage**: Sentry SaaS (sentry.io) or the operator-configured
  Sentry-compatible endpoint, indexed by the DSN.
- **Retention window**: target 30 days. The exact retention is governed
  by the Sentry project settings at the configured endpoint.
- **Data subject rights**: because no user identifier is sent, deletion
  requests must be scoped by time window or by the originating crate +
  version combination. Reach out via the issue tracker.
- **Sub-processors**: Sentry Inc. is the sole sub-processor when the
  default sentry.io endpoint is used. Operators using a self-hosted
  endpoint take on that responsibility themselves.

## PII scrubber

As a defence-in-depth measure, every event handed to the `SentryReporter`
is first run through a `PiiScrubber` (see
`crates/copypaste-telemetry/src/scrubber.rs`) before any transmission or
local debug-tracing call. Producers MUST still avoid putting user data
into `ReportableError` — the scrubber is a safety net, not a license to
include PII.

The default scrubber redacts, in order:

1. **Long hex strings** (UUIDs with or without dashes, ≥32-char hex
   digests) → `<REDACTED-HEX>`.
2. **JWT-like tokens** (three base64url segments, each ≥20 chars) →
   `<REDACTED-JWT>`.
3. **URL credentials** (`user:pass@` inside any `scheme://`) →
   `scheme://<REDACTED-AUTH>@…` (scheme and host preserved).
4. **Email addresses** → `<REDACTED-EMAIL>`.
5. **IPv4 and IPv6 addresses** → `<REDACTED-IP>`.
6. **Home directory prefixes** (`/Users/<name>/`, `/home/<name>/`) →
   `~/` (structural path tail preserved).

Custom organisation-specific patterns can be layered on with
`PiiScrubber::add_custom`; they redact to `<REDACTED-CUSTOM>`.

Scrubbing is deterministic and idempotent — re-scrubbing a scrubbed
string yields the same string. The `pii_scrubber` integration test suite
(`crates/copypaste-telemetry/tests/pii_scrubber.rs`) pins this contract.

The `NoopReporter` does not invoke the scrubber because it discards every
event without inspection or transmission.

## Reading the source

- API surface: `crates/copypaste-telemetry/src/lib.rs`
- Event shape: `crates/copypaste-telemetry/src/error.rs`
- PII scrubber: `crates/copypaste-telemetry/src/scrubber.rs`
- Opt-out tests: `crates/copypaste-telemetry/tests/opt_out.rs`
- PII scrubber tests: `crates/copypaste-telemetry/tests/pii_scrubber.rs`
- Sentry backend tests: `crates/copypaste-telemetry/tests/sentry_backend.rs`

If the source ever diverges from this document, the source wins and this
document is a bug — please open an issue.
