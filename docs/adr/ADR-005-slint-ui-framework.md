# ADR-005: Slint as UI Framework

## Status

Accepted

Date: 2026-05-23
Deciders: Project owner

## Context

CopyPaste needs a native desktop UI for macOS (and eventually Windows). Options considered:
1. **Tauri 2** — WebView wrapper + HTML/CSS/JS frontend
2. **Slint** — Native Rust UI toolkit with proprietary DSL
3. **egui** — Immediate mode GUI, pure Rust
4. **iced** — Elm-architecture Rust GUI

## Decision

**Slint** is the chosen UI framework.

## Rationale

| Criterion | Tauri | Slint | egui | iced |
|-----------|-------|-------|------|------|
| Bundle size | ~20MB (WebView) | ~2MB native | ~5MB | ~8MB |
| Startup time | ~1-2s | <100ms | <100ms | ~200ms |
| macOS feel | Web (non-native) | Native-ish | Immediate | Custom |
| Windows support | OK | OK | OK | OK |
| Design quality | High (CSS) | Medium (.slint) | Low (basic) | Medium |
| Rust integration | Tauri commands | Direct Rust | Direct Rust | Direct Rust |
| License | MIT | GPL/commercial | MIT | MIT |
| WebView dependency | Required | None | None | None |

**Why not Tauri:**
- Requires Node.js/npm in build pipeline
- WebView dependency adds complexity and macOS permission prompts
- Larger bundle, slower startup
- Previously implemented (Phase 3) but abandoned after evaluating Slint

**Why not egui:**
- Immediate mode = redraws every frame (battery drain on menu bar app)
- Limited styling capabilities

**Why Slint:**
- No WebView — pure native rendering
- Tiny binary (~2MB)
- Good macOS rendering quality
- Clean Rust API with type-safe bindings
- .slint DSL is readable and maintainable

## Consequences

- All UI code in `crates/copypaste-ui/` using Slint 1.8+
- No Tauri, no WebView, no npm/Node.js in build pipeline
- Windows support via same Slint codebase (cross-platform)
- Design limited to Slint's styling system (no CSS)
- License: GPL for open source use (acceptable for this project)

## Implementation

- `crates/copypaste-ui/ui/*.slint` — UI definitions
- `crates/copypaste-ui/src/` — Rust bindings and IPC wiring
- Built via `slint-build` in `build.rs`
