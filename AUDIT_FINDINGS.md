# CopyPaste — Consolidated Audit Findings (Master Table)

Canonical, deduplicated findings from 11 audit streams. Severity P0–P4. Every row is a filed bd issue; the **trace code** matches the `[AUDIT-0620 src:...]` tag in the bd description. Per-stream raw detail (with full reproduce/fix/test text) lives in `.audit/<stream>.md`.

**Streams:** A=architecture · B=crypto · C=storage · D=daemon/IPC/CLI · E=sync/relay/P2P/cloud · F=macOS/Tauri · G=Android · H=tests/CI · I=parity · P1=completeness · P2=reliability/UX · P3=test-coverage.

**Tally:** P0=0 · P1=23 · P2=66 · P3=45 · P4=3 (137 total). Raw≈205 → deduped 137. 5 refuted.

---

## P1 — High (23) — release-relevant; each has a bd id

| bd id | trace | title | files |
|---|---|---|---|
| phit | B-F01/D-C10.3 | `export` (include_sensitive) dumps full decrypted history over IPC/stdout, no warning | daemon/ipc.rs:8028-8206; cli/export.rs:73 |
| j9pv | C-F02 | `upsert_fts` non-atomic DELETE+INSERT → orphaned/unsearchable item | core/storage/items.rs:1487-1496 |
| d7um | P2-R02 | `revoke_device` non-atomic DELETE+INSERT → lost revocation audit record | core/storage/devices.rs:74 |
| crol | D-I5.1 | Wire-incompatible protocol defs: ipc crate `id:u64` vs live wire `id:String` | ipc/response.rs:48; daemon/protocol.rs:87; cli/ipc.rs:54 |
| ki7p | D-D2.1 | `persist_private_mode` writes flag file 0644 world-readable | daemon/daemon.rs:2966 |
| dl1e | D-D3.2 | `evict_stale_daemon` SIGTERMs peer-reported PID (recycle TOCTOU) | daemon/ipc.rs:8831 |
| liaz | D-C10.1/2 | `process::exit` skips `Zeroizing` drops of passwords/keys | cli/common.rs:90; cli/ipc.rs:284 |
| fkx7 | G-F1 | Android ABI `.so`↔Kotlin mismatch is non-fatal → silent crypto corruption | CopyPasteApp.kt:39; CopypasteBindings.kt:964 |
| hh3w | G-F2 | ProGuard may strip UniFFI entry → release runs no-crypto stub mode | CopypasteBindings.kt:964; build.gradle.kts:175 |
| mp1x | G-F3 | Android background capture needs `READ_LOGS` (not UI-grantable), no clear unavailable state | LogcatCaptureService.kt; AndroidManifest.xml:56 |
| fjvz | P2-UX-01 | macOS bulk delete fires immediately, no confirm/undo | ui HistoryView.tsx:2191 |
| uw45 | P2-UX-02 | macOS "Revoke all" via 12px inline Yes/No, no modal | ui DevicesView.tsx:1194 |
| 2ifa | P2-UX-03 | Android single-item delete, no confirm | HistoryActivity.kt:2880 |
| yel4 | P2-UX-04 | Android "Clear All" swallows errors + skips sync drain | SettingsActivity.kt:580 |
| jwga | P2-UX-05 | Android shows raw Kotlin/Rust exception text in toasts (9 ops) | ClipboardViewModel.kt:212-389 |
| 7yno | P2-UX-06 | Android raw exception (incl socket path) in QR pairing error | DevicesActivity.kt:1277 |
| w6xc | P2-UX-07 | macOS "Clear history" misclick-prone inline confirm, no undo | ui SettingsView.tsx:2590 |
| kaf6 | I-P1-2 | Android has no delete-undo (macOS has 5s undo) | android vs HistoryView.tsx:1531 |
| ei27 | I-P1-3 | Theme defaults dark on BOTH platforms, contradicts PARITY-SPEC light-first | App.tsx:283; Settings.kt:55 |
| 8jx8 | I-P1-1 | Android has no history export/import/backup (macOS GUI+CLI do) | android UDL/app |
| sxr1 | TC-01 | No regression test: cross-device relay-auth (foreign bearer → foreign inbox) | relay/state.rs:970 |
| ekzn | TC-02 | No regression test: FTS orphan on TTL expiry (secret leaks via search) | core/items.rs:1971 |
| ian9 | TC-03 | No unit test for mandatory PAKE confirm tag (all pairing e2e `#[ignore]`) | p2p/pairing_e2e.rs |

## P2 — Medium (66) — by stream (bd ids via `bd list --priority=2`, descriptions carry trace codes)

**Architecture (3):** A-F2 doc missing sync→core edge · A-F3 doc missing Android p2p/sync deps · A-F14 live `todo!()` in lifecycle.rs tests.

**Crypto (3):** B-F02 `derive_storage_key_v1` returns un-zeroized `[u8;32]` · B-F03 telemetry scrubber misses dot-less base64url tokens (scrubber.rs:69) · B-F04 `DeviceKeypair::ecdh` returns un-zeroized shared-secret copy (keys.rs:187-195).

**Storage (4):** C-F01 FTS5 plaintext-at-rest tradeoff undocumented · C-F03 sensitive TTL fires stale `expires_at` after recopy · C-F05 dual TTL semantics (wall_time vs expires_at) unpredictable · C-F09 detector misses `access_token`/`client_secret`/`refresh_token`/`db_password` (patterns.rs:139-146).

**Daemon/IPC/CLI (10):** D-D3.3 no per-request IPC read timeout (slot+DB-mutex DoS) · D-D3.1 socket not cleaned on SIGKILL · D-D1.1/D4.2 `lsappinfo` forked every tick blocks signals · D-D1.2 self-write sentinel off-by-one dup · D-D2.3 file stat+read TOCTOU · D-D4.1 broadcast `Lagged` drops unmetered · D-I9.1 decrypted plaintext over socket + bind→chmod TOCTOU (by design, narrow window) · D-C10.4 `status --json` exit 0 empty on serialize fail · D-C10.5 status reason to stdout not stderr · D-C10.6 `restore` no confirm before replacing live DB.

**Sync/Relay/P2P/Cloud (6):** E-R1 SSE subscribe no per-device conn cap · E-R2 rate-limit keyed on pre-auth `device_id` (bypassable) · E-S2 `clamp_timestamps` not enforced at deserialize · E-P1-1 mTLS verifier skips cert expiry · E-C1 `SUPABASE_ANON_KEY` in WSS query string (re-rated P1→P2) · E-C2 no Supabase WSS cert pinning.

**macOS/Tauri (5):** F-F3 LogView no offline guard (raw FS error) · F-F5 QR error renders socket path + username in DOM · F-F7 discoverError raw IPC text · F-F8 single app-wide ErrorBoundary blanks whole window · F-F10 tray Private-Mode desyncs after daemon restart.

**Android (4):** G-F4 only arm64-v8a; 32-bit silently stubs FFI · G-F5 TextKind Kotlin fallback diverges from Rust (misleading coverage) · G-F6 QR not re-blurred on 120s refresh (live token visible) · G-F7 SAS code copyable to clipboard during pairing.

**Tests/CI/Release (5):** H-F01 no ESLint/CI lint for frontend · H-F02 `--all-features` tests never run in CI · H-F03 Android lint only on tag, not PR · H-F04 committed `debug.keystore` w/ creds + `keystore-beta.jks` not gitignored · H-F05 verify `local.properties` untracked.

**Parity (7):** I-P2-1 Android no P2P/mTLS LAN sync (undocumented) · I-P2-2 Android SyncBackend mutually-exclusive vs macOS additive · I-P2-3 no SQLCipher backup/restore in either GUI · I-P2-4 vacuum/stats have daemon support but no macOS-UI · I-P2-5 Android no degraded-DB reset recovery · I-P2-6 Android SAS omits peer-metadata card · I-P2-7 Android discovered-peer shows only first IP.

**Completeness (3):** CMP-002 macOS "Enable sync" stale "requires daemon update" warning (daemon honors it) · CMP-003 `copypaste-telemetry` orphaned (0 dependents) · CMP-004 cloud IPC `not_implemented` in non-cloud builds + `cloud_sign_in/out` missing METHOD const.

**Reliability (9):** P2-R03 `has_sensitive_items()` false on DB error (sensitive persists past TTL) · P2-R04 `revoked_devices` table outside migrations (panic risk) · P2-R06 relay push no retry queue (item loss) · P2-R07 relay/P2P JoinHandles dropped (silent task death) · P2-R08 poisoned `SyncCrypto` mutex recovered silently · P2-UX-08 SyncStatusChip stale "connected" ~10s · P2-UX-11 Android SYNCING badge dead code · P2-UX-12 Android Max-History slider not enforced · P2-UX-13 macOS private-mode wrong empty-state copy.

**Test coverage (7):** TC-04 relay fanout N>2 untested · TC-05 nonce uniqueness 2-sample only · TC-06 relay constant-time no regression guard · TC-07 SQLCipher wrong-key no test · TC-08 detector recall corpus missing · TC-09 IPC ErrorCode serde roundtrip missing · TC-10 Keychain ThisDeviceOnly tests all `#[ignore]`.

## P3 — Low (45)

Filed individually (`bd list --priority=3`). Areas: dead code & `#[allow]` debt (A-F8/F10/F12/F13, D-D1.3), key-handling edges (B-F06/F07/F09), storage hygiene (C-F04/F06/F08/F10 incl. **no VACUUM ever runs** and **private IPs auto-wiped at 0.70 confidence**), daemon/CLI polish (D-D2.4/D3.4/D4.3/I8.1/C10.7/C10.8), relay hardening (E-R3/R4/R5/S3/C3), macOS UI polish (F-F9/F11/F14/F17), Android (G-F8/F9/F10), CI (H-F06/F08/F09/F10/F12), parity cosmetics (I-P3-1/2/3/5/7), and completeness gaps (CLI device-mgmt absent, CLI backup/restore bypass daemon, phantom `migrate_ciphertext_v1_to_v2`/`get_lamport`).

## P4 — Nice-to-have (3 umbrellas)

Cosmetic polish (poll cadence, redundant `app_bundle_id`, stale comments) · completeness P4 batch · `pair-qr --raw` scrollback note (by-design).
