# Architectural Debt Register — Post-alpha

Tracks items deferred from `docs/audit/2026-05-23-fix-plan.md`.

| ID | Source | Item | Plan |
|----|--------|------|------|
| arch-1 | architecture CRITICAL #1 | Orphan crates (p2p, sync, supabase) not wired into daemon | Beta: integrate or remove |
| arch-2 | architecture CRITICAL #2 | UI duplicates IPC wire types — no shared `copypaste-ipc` crate | Beta: extract |
| arch-3 | architecture CRITICAL #4 | Relay `db.rs` imports rusqlite but not in Cargo.toml + not mod-included | Beta: delete or wire |
| arch-4 | architecture HIGH #5/#6 | `Arc<Mutex<Database>>` chokepoint, no spawn_blocking | Beta: pool + spawn_blocking |
| arch-5 | architecture HIGH #7 | Single `AppConfig` merge core::config + daemon::ipc | Beta: ADR + migration |
| arch-6 | architecture HIGH #8 | IPC protocol unversioned | Beta: add version negotiation |
| arch-7 | architecture HIGH #9/#10 | Platform abstraction (delete or wire) | Phase 5b |
| arch-8 | architecture HIGH #11 | Workspace dep dedup (reqwest 0.11→0.12, rustls 0.21→0.23, hyper 0→1) | Beta |
| arch-9 | best-prac MEDIUM #13 | ipc.rs 931+ LOC, 4 other oversize files | Beta: split |
| arch-10 | edge-cases LOW #32 | Credit-card multi-line regex | Backlog |
| arch-11 | edge-cases INFO #36-38 | macOS image fixture, Windows placeholder, Android tests | Phase 5 |
| arch-12 | sec MEDIUM #12 | SQLCipher migration probe side-channel | Backlog |
