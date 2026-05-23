# ADR-002: Unix socket over HTTP/gRPC for daemon IPC

## Status

Accepted

Date: 2026-05-22

## Context

The daemon needs an IPC channel for the CLI and Tauri UI to query clipboard history.

## Decision

Use Unix domain socket at `~/Library/Application Support/CopyPaste/daemon.sock` with newline-delimited JSON.

## Rationale

1. **No port conflicts** — no TCP port to manage or firewall rules needed
2. **Filesystem ACL** — socket file permissions control access without auth tokens
3. **Lower latency** — no TCP handshake overhead for local requests
4. **No TLS** — local communication doesn't need TLS, simpler stack
5. **Simple protocol** — newline-delimited JSON is debuggable with `nc` and `echo`

## Consequences

- Windows requires Named Pipes (`\\.\pipe\CopyPaste`) — abstracted in Phase 5a
- Not accessible remotely (by design — sync uses relay over HTTPS)
