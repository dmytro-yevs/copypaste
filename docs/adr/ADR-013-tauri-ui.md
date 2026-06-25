# ADR-013: Replace Slint with Tauri v2 + React/TypeScript/Vite for Desktop UI

**Status:** Accepted
**Date:** 2026-05-28
**Deciders:** Project owner
**Supersedes:** ADR-005 (Slint UI framework — file deleted along with the Slint crate)

## Context

CopyPaste's desktop UI (`crates/copypaste-ui`) was built with Slint 1.8.
Over the course of v0.3 development two problems became blocking:

1. **API churn.** Slint's Rust binding API changed significantly between
   minor releases, breaking the build on routine dependency bumps and
   requiring frequent `=1.8.0` exact pins to stabilise the workspace.

2. **DSL friction.** The bespoke `.slint` declarative DSL made UI iteration
   slow — every layout change required toggling between Rust, the LSP, and
   a preview tool, with no browser DevTools equivalent.

The UI has always been an IPC-only shell: it never links `copypaste-core`,
all data flows through the Unix socket owned by `copypaste-daemon`, and
rendering quality/fidelity is a product concern (users compare it to the
macOS system UI). Tauri v2 satisfies all these constraints while opening the
entire web-frontend ecosystem for UI iteration.

## Decision

Replace the Slint UI with **Tauri v2** (Rust shell) + **React** +
**TypeScript** + **Vite** (frontend toolchain). All UI code lives in
`crates/copypaste-ui/` (frontend) and `crates/copypaste-ui/src-tauri/`
(Rust Tauri backend). The crate is a workspace member via
`crates/copypaste-ui/src-tauri`.

## Rationale

- **Web tooling velocity.** CSS, React components, and browser DevTools make
  UI iteration substantially faster than a bespoke DSL with a separate
  preview tool.
- **IPC architecture is unchanged.** The frontend talks to the Tauri Rust
  layer via `invoke()`; the Rust layer forwards to the daemon over the
  existing Unix socket. No `copypaste-core` linkage is introduced.
- **Tray, vibrancy, activation policy.** Handled by the Tauri APIs and the
  `window-vibrancy` crate, with the same macOS-specific behaviour as before.
- **Single distribution binary.** Tauri bundles the frontend assets into the
  binary; no separate Node runtime is shipped.
- **Workspace isolation.** Dropping `slint = "=1.8.0"` and `slint-build =
  "=1.8.0"` from `[workspace.dependencies]` removes the GPL/LicenseRef dual-
  licence exception block from `deny.toml` and the `=`-pinned exact version
  from the dependency audit surface.

## Consequences

- The `copypaste-ui-snapshot` crate (headless Slint PNG renderer used in CI)
  is deleted — it has no equivalent in the Tauri architecture.
- `slint`, `slint-build`, and all `i-slint-*` transitive deps are removed
  from the workspace dependency graph.
- The Windows note in ADR-012 referencing the Slint Windows backend remains
  accurate: Windows support is still frozen; the Tauri Windows backend is
  untested.
- Frontend build now requires Node.js + pnpm at development time (not at
  runtime; assets are compiled in at release build time via `tauri build`).

## Implementation

- `crates/copypaste-ui/src/` — React + TypeScript frontend (Vite-built).
- `crates/copypaste-ui/src-tauri/src/` — Rust Tauri commands, IPC bridge,
  tray host, vibrancy, and activation policy.
- Workspace member: `crates/copypaste-ui/src-tauri` (Cargo workspace).
- Frontend entry: `crates/copypaste-ui/index.html`.
