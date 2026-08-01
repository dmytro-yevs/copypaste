# ADR-0011 — Runtime logs are bounded and message-only

**Status:** accepted · 2026-08-01

## Decision

Both app processes write daily files through `tracing-appender`. Each keeps at
most seven files and uses its non-blocking worker. Support export contains a
bounded redacted snapshot of those files and a distinct diagnostics section.

The formatter retains only timestamp, level, target and the event message.
All fields are refused because error, clipboard, pairing and server-response
fields cannot safely be treated as diagnostic data.

## Consequence

`tracing-appender` supplies rotation, retention and async writing. The small
formatter is the required final privacy boundary; no maintained dependency can
decide which application event fields are clipboard content or secrets.
