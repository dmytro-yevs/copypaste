# CopyPaste — Cross-Platform Gap Report

Platform drift between macOS/Tauri, Android, CLI, daemon. Full per-feature matrix: `PARITY_MATRIX.md`. Per-gap detail (expected/actual/files/fix/test): `.audit/I-parity.md`. All gaps are bd issues.

**Token/design layer: in genuine parity** — `parity-check.mjs` 53/53 and `check-skin-parity.mjs` 21/21 both pass; every previously-reported token drift is already fixed. The remaining gaps are **feature/behavior**, concentrated on Android + a spec contradiction affecting both.

## P1 platform gaps

| bd | gap | macOS | Android | CLI |
|---|---|---|---|---|
| ei27 | Theme default contradicts PARITY-SPEC §0 light-first | defaults **dark** | defaults **dark** | n/a |
| 8jx8 | History export/import/backup | GUI ✅ + CLI ✅ | ❌ absent (not in UDL/app) | ✅ |
| kaf6 | Delete-undo | ✅ 5s undo | ❌ none | n/a |

## P2 platform gaps

| bd trace | gap | macOS | Android |
|---|---|---|---|
| I-P2-1 | P2P/mTLS LAN sync | ✅ | ❌ relay/cloud only (undocumented) |
| I-P2-2 | Multi-transport model | additive (relay+cloud) | mutually exclusive |
| I-P2-3 | SQLCipher backup/restore in GUI | ❌ (CLI only) | ❌ (CLI only) |
| I-P2-4 | vacuum/stats UI surface | ❌ (daemon supports) | partial |
| I-P2-5 | Degraded-DB "Reset database" recovery | ✅ | ❌ |
| I-P2-6 | SAS dialog peer-metadata card | ✅ | ❌ omitted |
| I-P2-7 | Discovered-peer addresses | all IPs joined | first IP only |
| jwga/7yno | Error surfaces | mapped (mostly) | raw exception/socket-path text |
| G-F6 | QR re-blur on refresh | n/a (different flow) | not re-blurred (live token visible) |
| P2-UX-08/11 | Sync-status truth | stale "connected" ~10s | SYNCING badge is dead code |

## Documented platform limitations (intentional, keep visible in UI)
- **Android background clipboard** requires `READ_LOGS` via `adb` (Android 10+ restriction) — not a bug, but must show a clear unavailable state (`mp1x`).
- **Android lacks NSPasteboard-style polling** — it is a UniFFI node, not a daemon/IPC/socket host; several "missing IPC method" observations are reframed by this (see `.audit/I-parity.md` architecture note).
- **CLI has no device/pairing surface** — currently UI/Android-only; either add or document as intended scope (P3 CMP-P3a).

## Fix policy applied
Security/privacy/identity/sync-status drift ≥ P1; undocumented missing parity ≥ P2; visual-only ≤ P3. No "platform difference" was accepted as an excuse unless the limitation is real and now documented above.
