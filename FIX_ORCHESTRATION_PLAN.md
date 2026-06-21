# CopyPaste — Fix Orchestration Plan

Master coordination doc for **Stage 2+ (fixing)**. Source of findings: `AUDIT_FINDINGS.md` + bd (137 issues tagged `[AUDIT-0620 src:...]`). This file assigns owners and ordering; it does **not** restate every finding.

## Status
- **Stage 1 (audit/discovery): COMPLETE.** 137 findings filed to bd. P0=0, P1=23, P2=66, P3=45, P4=3. 5 refuted.
- **Stage 2 (fix P1): NOT STARTED** — gated on review of this plan.

## Fix-agent assignment matrix

| Agent | Owns (bd trace prefixes) | P1 count |
|---|---|---|
| Security & Crypto | B-*, phit, liaz, ki7p, E-C1 | 3 |
| Storage & Data Integrity | C-*, j9pv, d7um, P2-R03/R04 | 2 |
| Daemon & IPC | D-*, crol, dl1e | 3 |
| Sync/Relay/Cloud/P2P | E-*, P2-R06/R07/R08 | 0 |
| macOS/Tauri UI | F-*, fjvz, uw45, w6xc, ei27(web), CMP-002, P2-UX-08/13 | 3 |
| Android/UniFFI | G-*, fkx7, hh3w, mp1x, 2ifa, yel4, jwga, 7yno, kaf6, 8jx8, ei27(android) | 9 |
| UI Parity/Design | I-*, P3 parity items | 3 |
| Tests/CI/Release | H-*, sxr1, ekzn, ian9, TC-* | 3 |

## Coordination rules (per user mandate)
1. No two agents edit the same file concurrently — Android-UX cluster and macOS-UX cluster touch disjoint trees, so they may run in parallel; storage atomicity (`items.rs`, `devices.rs`) is single-owner.
2. Each fix = bd `--claim` → smallest reviewable commit → regression test → bd `--notes` → bd `close`. Notes before every close.
3. Architecture invariants are frozen: CLI/UI IPC-only, relay ciphertext-only, core has no daemon/UI/CLI dep, no plaintext storage/sync, no secret logging.
4. Do not break IPC wire-compat **except** `crol` (the protocol-id unification), which is an intentional, coordinated wire fix — bump `IPC_PROTOCOL_VERSION` and update daemon+CLI together.

## Dependency-aware order
`crol` (protocol types) and the shared destructive-action contract should land **before** the per-screen UX fixes that depend on them. `8jx8` (Android export) depends on a UDL change → regenerate UniFFI bindings → Compose screen. CI fixes (`H-*`) should land early so subsequent fix PRs are gated by the stronger pipeline.

## Skipped / invalid / deferred
- **5 refuted** (see AUDIT_REPORT §7) — not filed, do not re-open.
- **By-design, documented** (P4): `pair-qr --raw` scrollback, D-I9.1 plaintext-over-0600-socket, FTS plaintext-at-rest (to be documented, not removed).
- **Frozen:** Windows (`ipc_win.rs` dead_code) per ADR-012 — no action.

## Companion fix-stage deliverables (to be produced as fixing proceeds)
`FIX_SUMMARY.md` (what changed / tests added / remaining / risks) and an updated `VERIFICATION_REPORT.md` (full gate re-run incl. Android gradle) — both created during/after Stage 2, not now.
