# CopyPaste Telemetry Policy

_Last updated: 2026-05-23 (0.2.0-beta)_

CopyPaste ships an **opt-in, privacy-first** error reporter. This document is
the authoritative description of what the reporter does, what it sends, and
the user's rights. It binds the implementation in
[`crates/copypaste-telemetry`](../../crates/copypaste-telemetry).

## TL;DR

- Reporting is **OFF by default**. We never enable it for you.
- We never send clipboard contents, file paths, device IDs, IPs, account
  emails, or any free-form text.
- In 0.2-beta no events leave your machine at all — the Sentry backend is a
  stub. The API surface ships so downstream crates can pin to it.

## Defaults & consent

The reporter accepts one of three consent values:

| Value             | Behaviour in 0.2-beta                                            |
|-------------------|------------------------------------------------------------------|
| `Disabled`        | Default. Returns a `NoopReporter` that discards every event.     |
| `EnabledMinimal`  | Returns a `SentryReporter` stub. Reports fail with `NotImplemented`. |
| `EnabledFull`     | Same as `EnabledMinimal` in 0.2-beta; reserved for future fields.    |

A real backend will only ship in a later release. When it does, this
document will be updated _before_ the code is enabled.

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

## Retention & sharing

Until the Sentry backend is implemented, retention is **N/A** because no
data leaves the device. When the backend ships, this section will spell out:

- Retention window (target: 30 days).
- Storage location and provider.
- Data subject rights (access, deletion).
- Whether any sub-processor receives events.

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

If the source ever diverges from this document, the source wins and this
document is a bug — please open an issue.
