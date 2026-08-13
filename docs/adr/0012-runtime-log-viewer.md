# ADR-0012 — In-app runtime log viewer

**Status:** accepted · 2026-08-01

## Decision

Reuse the already-installed `@tanstack/react-virtual` for the runtime-event
list. Its React adapter supports measured rows, overscan and a controllable
latest-item position, which covers a growing log without rendering every row.

The Rust runtime-log crate remains the sole reader: it returns pages of at
most 100 message-only, path-scrubbed events, holds at most 256 KiB of one file
in memory, and walks at most 2,000 rows per call. The WebView gets no file
locations, tracing fields, clipboard data, secret values or raw errors. Android
exposes its app-process events only; there is no invented daemon source.

Its cursor is a byte position in the append-only log — a day and an offset per
process — not a row offset and not a timestamp. Neither of the alternatives is
an identity. A row offset shifts down by one for every event written while the
viewer is open, so page two repeats a row and drops one the user never saw
(`CopyPaste-8ebg.57`). A timestamp plus a count of rows shown at it fails the
same way inside a single millisecond, which is the ordinary case for a log: rows
appended bearing that millisecond are counted as rows already shown, and the
next page skips them and replays old ones. A byte offset never moves.

Two things follow, and both are contract:

- **Join a head read to a paged one through the cursor, never by timestamp.** A
  page boundary may fall inside a millisecond, so "older than the oldest row on
  screen" discards rows that were never shown.
- **One call is bounded, the walk is not.** A call stops after a fixed number of
  rows and returns a cursor; an empty page with a cursor means "keep going", not
  "end of log". Treating the read window's edge as the end made every row older
  than it unreachable.

A cursor the reader did not write is refused rather than treated as "start from
the top". One naming a day that has rotated away is the end of the log: those
rows no longer exist.

## Considered

`rev_lines` and `rev_buf_reader` read a file's lines backwards, which is the
shape the reader needs, but neither reports the byte offset a line started at —
and that offset is the whole of the cursor identity above. The reader therefore
seeks bounded windows itself (rule 1 exemption 1).

`react-window` and a dedicated terminal-log viewer would add a second list
stack while this project already ships TanStack Virtual 3.14.9. The official
React documentation confirms the maintained adapter, measured elements and
React 19 configuration; no additional dependency is justified.

Source: https://tanstack.com/virtual/latest/docs/framework/react/react-virtual
