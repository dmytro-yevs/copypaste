# ADR-0012 — In-app runtime log viewer

**Status:** accepted · 2026-08-01

## Decision

Reuse the already-installed `@tanstack/react-virtual` for the runtime-event
list. Its React adapter supports measured rows, overscan and a controllable
latest-item position, which covers a growing log without rendering every row.

The Rust runtime-log crate remains the sole reader: it returns pages of at
most 100 message-only, path-scrubbed events from a one-megabyte / 2,000-event
window. The WebView gets no file locations, tracing fields, clipboard data,
secret values or raw errors. Android exposes its app-process events only;
there is no invented daemon source.

## Considered

`react-window` and a dedicated terminal-log viewer would add a second list
stack while this project already ships TanStack Virtual 3.14.9. The official
React documentation confirms the maintained adapter, measured elements and
React 19 configuration; no additional dependency is justified.

Source: https://tanstack.com/virtual/latest/docs/framework/react/react-virtual
