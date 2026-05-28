# copypaste-ui

## Purpose
Tauri v2 + React/TypeScript/Vite desktop UI for CopyPaste. Provides the history view, settings, and P2P pairing flow. Talks to the daemon through the IPC client over a Unix socket; does not link `copypaste-core`.

## Structure

- `src/` — React + TypeScript frontend (Vite-built, bundled at release time).
- `src-tauri/` — Rust Tauri backend: IPC bridge, tray host, vibrancy, activation policy.
- `src-tauri` is the Cargo workspace member.

## Platform support
- **macOS**: primary.
- **Windows / Linux**: Tauri supports these targets but they are untested and not actively maintained.
- **Android**: not applicable.

## Status
beta.

## Internal vs published
Internal crate. Not published to crates.io.

## Quick start

```bash
# Run the UI alongside a running daemon.
cd crates/copypaste-ui
pnpm install
pnpm tauri dev
```

## Related ADRs
- [ADR-013](../../docs/adr/ADR-013-tauri-ui.md) — Tauri v2 + React chosen as the UI framework (supersedes ADR-005).
