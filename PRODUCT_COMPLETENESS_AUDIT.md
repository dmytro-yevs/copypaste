# CopyPaste v0.7.5 — Product Completeness Audit

## Executive Summary

CopyPaste v0.7.5 is **not production-ready for a public v1.0 release** and should not ship in its current state without resolving at least the 18 confirmed release blockers. The product's core clipboard-history and macOS-daemon stack is well-engineered, but the codebase suffers from six systemic failure modes that cut across every layer. First, **dead settings that silently mislead on privacy and data-cost**: six Android toggles (`sync_on_wifi_only`, `excludedAppBundleIds`, `lanVisibility`, `imageQuality`, `pasteAsPlainText`, `maxHistoryItems`) are persisted and displayed but have zero runtime effect, and the macOS `sync_enabled` master kill-switch is disputed or carries stale warning copy — together creating a pattern where the settings panel is a trust-destroying museum of inert controls. Second, **trust-state indicators that can never show distrust**: both UIs hard-code "Verified" for every pairing, `list_peers` emits no `trust` field, and a legacy `pair_peer` arm can add a peer to the live mTLS allowlist with no PAKE or SAS — meaning the badge the user relies on to decide whether to paste sensitive content conveys nothing. Third, **sensitive data leaking via FTS, export, and logs**: plaintext credentials indexed in `clipboard_fts` are returned over the IPC socket by `search`; `export --include-sensitive` ships the entire decrypted history in one unauthenticated call; the daemon tracing sink has no `PiiScrubber` layer; Android logs write to world-discoverable external storage without redaction. Fourth, **macOS↔Android parity drift**: private-mode capture suppression, sensitive image/file capture, sync transport configuration, and auto-apply clipboard behavior diverge silently between the two client implementations, with the more capable macOS behavior rarely the Android default. Fifth, **destructive actions without confirmation or undo**: macOS bulk delete fires with no confirm and no undo, Android single-item delete propagates an irreversible tombstone to all peers with no confirm, and "Clear All" from Android Settings swallows errors and never drains the mutation queue. Sixth, **test theatre**: multiple Android test classes reimplement the logic they assert (tautologies), daemon IPC/poll integration tests are universally `#[ignore]`, mutating storage operations have no backend regression tests, and `SyncEngine`/`LamportClock` are exhaustively tested but dead on the production code path. A 1.0 launch with the Android release APK falling back to a debug-signed build on a missing keystore would immediately strand all installed users on an un-upgradable build. Addressing the 18 release blockers plus the systemic themes around dead settings, trust indicators, and sensitive-data leakage is the minimum bar before any public release.

---

## Totals

| Metric | Count |
|--------|-------|
| Distinct features | 101 |
| Category placements | 170 |
| Complete | 45 |
| Mostly complete | 43 |
| Partial | 37 |
| Stub | 12 |
| Broken | 11 |
| Risky | 16 |
| Needs major improvement | 4 |
| **Gap findings (P0–P4)** | **128** |
| P0 | 1 |
| P1 | 28 |
| P2 | 68 |
| P3 | 28 |
| P4 | 3 |
| **Release blockers** | **18** |

---

## Top 10 Product Risks

1. Silent DB-key regeneration on Android wipes the entire clipboard history with no user signal (PCA-001)
2. Android `private_mode` toggle does not suppress capture — sensitive clips are silently stored and synced (PCA-003)
3. `sync_enabled` master kill-switch is either a daemon stub or carries stale warning copy — sync may continue when "off" (PCA-007)
4. `export --include-sensitive` dumps all decrypted history over IPC/stdout in a single unauthenticated call (PCA-008)
5. Legacy `pair_peer` registers a peer in the live mTLS allowlist with no PAKE or SAS, while the trust badge permanently shows "Verified" (PCA-006 + PCA-005)
6. Android release APK falls back to a debug-signed build when keystore secrets are absent, stranding all future installers (PCA-015)
7. Sensitive plaintext indexed in FTS is returned in full over the IPC socket by `search` with only a client-side blur (PCA-002)
8. `upsert_fts` non-atomic DELETE+INSERT can render items permanently unsearchable on crash (PCA-018)
9. `replace_cloud_item_by_item_id` INSERT may omit the `deleted` column, silently resurrecting deleted items after cloud sync (PCA-016)
10. Android background clipboard capture is non-functional on stock Android 10+ for ordinary users, with two contradictory implementations each making false claims (PCA-029)

---

## Top 10 Platform Parity Gaps

1. Android `private_mode` has a UI toggle that does nothing; macOS enforces it at daemon tick level (PCA-003)
2. Android `sync_on_wifi_only` flag is never read by any transport; macOS enforces it in relay and cloud (PCA-025)
3. Android sensitive image/file capture silently drops the item; macOS daemon stores it with `is_sensitive=true` + TTL (PCA-004)
4. Trust label is hardcoded "Verified" on both UIs; `list_peers` emits no `trust` field; no unverified state can be rendered on either platform (PCA-005)
5. Cloud (Supabase) sync strips `pinned`/`pin_order` on macOS ingest; P2P and Android relay carry it correctly (PCA-017)
6. Android sync-key rotation (`revoke_and_rotate`) has FFI but no UI wiring; a revoked peer keeps reading cloud/relay blobs on Android (PCA-060)
7. Android auto-apply silently overwrites the user's current clipboard on every sync round-trip; macOS has an opt-in `auto_apply_synced_clip` toggle (PCA-055)
8. macOS Lamport clock is logical; Android upgrade pollutes the clock with wall-millis, biasing LWW toward Android items until macOS catches up (PCA-041)
9. Android online/offline derivation uses mDNS IP-correlation + 60s lastSync window; macOS uses live P2P sinks — same peer can show different status on each platform (PCA-069)
10. Android has no history export/import, no in-app DB-reset recovery, and no P2P LAN sync loop despite the core FFI existing (PCA-112)

---

## Top 10 UX Problems

1. macOS bulk delete (select-all + Delete) fires immediately with no confirmation and no undo, destroying the entire history silently (PCA-020)
2. Android single-item delete propagates an irreversible tombstone to all paired devices with no confirmation or undo snackbar (PCA-021)
3. Android "Clear All" from Settings swallows errors, assumes success, and never drains the mutation queue so peers are never cleared (PCA-023)
4. Android shows raw exception text (`DecryptionFailed`, `database is locked`, `NullPointerException`) directly in user-facing toasts and the QR error surface (PCA-024)
5. macOS `private_mode` active state shows "Copy something and it will appear here" instead of a dedicated private-mode empty state (PCA-080)
6. SyncStatusChip can show a stale "connected" green for up to 10 seconds after the daemon or network goes offline (PCA-079)
7. macOS LogView shows a raw error string (can include the filesystem path and username) with no retry, no RestartDaemonButton, no offline state, and zero tests (PCA-073)
8. Android sync failures are invisible — every error path logs only `Log.w`; a 401 collapses to an empty list so a revoked device stalls silently (PCA-056)
9. macOS "Clear history" and "Revoke all" use a tiny inline Yes/No confirm inside a dense settings row — no modal, no consequence explanation, high misclick risk (PCA-022)
10. HistoryView does not refresh after a successful backup import, making users believe the import failed (PCA-074)

---

## Top 10 Missing Tests

1. Mutating storage operations (`pin_item`, `reorder_pinned`, `delete_item`, `delete_all`, `soft_delete_item`) have no backend tests; IPC/poll integration tests are all `#[ignore]` (PCA-099)
2. Android critical runtime paths (capture dispatch, key storage, service restart, FFI error mapping) have zero JVM coverage; many existing tests are cosmetic tautologies (PCA-118)
3. Cross-device relay token rejection, FTS cleanup on TTL expiry, PAKE confirm-tag rejection, nonce uniqueness (large-N), SQLCipher wrong-key rejection, detector true-positive recall corpus, exhaustive ErrorCode serde round-trip, Keychain ACL attribute — none are runnable CI tests (PCA-111)
4. CI gaps: no ESLint, no `cargo test --all-features`, Android lint not on PRs, instrumented crypto tests never run, committed keystores, security-critical quality jobs are `continue-on-error: true` (PCA-110)
5. `SyncEngine`/`LamportClock` are exhaustively tested but dead on the production code path — tests give false confidence about production LWW behavior (PCA-104)
6. Migration ladder integration tests end at v4; v5–v11 have no on-disk real-file tests; the v5 crash-resume stub is `todo!()`; vacuum `REINDEX` is a no-op for FTS5 (PCA-108)
7. No `search` IPC round-trip test; sensitive FTS indexing policy is undocumented and untested from the IPC contract perspective (PCA-002)
8. `revoke_device` non-atomic DELETE+INSERT has no crash-mid-revoke test asserting audit-row integrity (PCA-019)
9. `upsert_fts` non-atomic DELETE+INSERT has no crash-between-statements test confirming item remains searchable (PCA-018)
10. Android `DeviceKeyStore` has no test; no Robolectric test for the silent key-regen on unwrap failure (PCA-001)

---

## Release Blocker List

| PCA-ID | Severity | Title | Platforms |
|--------|----------|-------|-----------|
| PCA-001 | P0 | Silent DB-key regeneration wipes entire Android clipboard history | Android |
| PCA-002 | P1 | Sensitive plaintext indexed in FTS and returned by `search` over IPC | macOS, daemon, Linux |
| PCA-003 | P1 | Android `private_mode` toggle does not suppress capture | Android |
| PCA-004 | P1 | Sensitive images/files silently dropped at capture on Android | Android |
| PCA-006 | P1 | `pair_peer` adds a trusted/allow-listed peer with no PAKE/SAS | daemon |
| PCA-007 | P1 | `sync_enabled` master kill-switch stub / UI text drift | macOS, Android |
| PCA-008 | P1 | `export --include-sensitive` dumps all plaintext over IPC/stdout unguarded | macOS, CLI, Linux |
| PCA-009 | P1 | `revoke_all_peers` destructive action with weak/absent UI confirmation | macOS, Android, daemon |
| PCA-013 | P1 | PiiScrubber not applied to daemon tracing log sink (PII leak risk) | macOS, Linux |
| PCA-015 | P1 | Android release APK falls back to debug-signed when keystore secrets absent | Android |
| PCA-016 | P1 | `replace_cloud_item_by_item_id` INSERT may omit `deleted` → tombstone resurrection | macOS |
| PCA-018 | P1 | `upsert_fts` non-atomic DELETE+INSERT → items permanently unsearchable | macOS, Android, Linux |
| PCA-019 | P1 | `revoke_device` non-atomic DELETE+INSERT → lost revocation audit | macOS, Android |
| PCA-020 | P1 | macOS bulk delete: no confirmation and no undo | macOS |
| PCA-021 | P1 | Android single-item delete: no confirmation/undo, propagates tombstone | Android |
| PCA-023 | P1 | Android "Clear All" from Settings swallows errors, skips queue drain | Android |
| PCA-024 | P1 | Android raw exception text shown in user-facing toasts / QR error | Android |
| PCA-097 | P2 | macOS Release ad-hoc signing / no notarisation (blocks 1.0 only) | macOS |

---

## Recommended Fix Order

### Group A — Security/Privacy (fix before any public build)

1. Fix silent DB-key regen on Android: show a persistent warning and require explicit user confirmation before regenerating (PCA-001)
2. Exclude `is_sensitive` items from FTS index and `search` IPC response, or enforce the chosen policy consistently (PCA-002)
3. Fix Android `private_mode` — add the `privateMode` check to `dispatchClipData`/`clipListener` (PCA-003)
4. Fix Android `captureImageClip`/`captureFileClip` early-returns — route all three types through `sensitive_capture_decision` (PCA-004)
5. Guard `export --include-sensitive` with a daemon-minted one-time token; require `--output` or explicit `--stdout`; add a mandatory confirm dialog in the UI (PCA-008)
6. Wire `PiiScrubber` as a `tracing_subscriber::Layer` on the daemon file-log sink (PCA-013)
7. Move Android logs/crashes from external storage to internal `filesDir`; add a `scrub()` helper in `AppLogger` (PCA-028, PCA-053)
8. Remove or gate `pair_peer` so it cannot register into the live mTLS allowlist without completed PAKE/SAS (PCA-006)
9. Wrap `upsert_fts` DELETE+INSERT in `unchecked_transaction()` (PCA-018)
10. Wrap `revoke_device` DELETE+INSERT in `unchecked_transaction()` (PCA-019)
11. Fix `replace_cloud_item_by_item_id` INSERT to include the `deleted` column (PCA-016)

### Group B — Critical UX / Data Safety (before any beta)

12. Fix Android release pipeline — fail the job (not warn) when `ANDROID_KEYSTORE_BASE64` is absent on a tag ref (PCA-015)
13. Replace macOS bulk-delete with a `ConfirmModal` and/or a batched undo window (PCA-020)
14. Add a confirm/undo to Android single-item delete; confirm for "Clear All" from Settings; ensure mutation-queue drain fires (PCA-021, PCA-023)
15. Map raw exceptions to friendly error strings in all Android toast paths and the QR error surface (PCA-024)
16. Replace macOS "Clear history" and "Revoke all" inline Yes/No with modal confirm dialogs (PCA-022, PCA-009)
17. Resolve the `sync_enabled` discrepancy: verify the daemon field end-to-end and remove stale warning copy (PCA-007)
18. Add a `trust`/`pairing_method` field to `list_peers` and render it on both UIs so the trust badge can show "Unverified" (PCA-005)

### Group C — Dead Settings / Parity Drift (before public GA)

19. Kill or label Android dead toggles: wire `sync_on_wifi_only` to transports, hide `excludedAppBundleIds`, wire `lanVisibility` to NSD, hide `pasteAsPlainText`, wire or label `maxHistoryItems` (PCA-025, PCA-026, PCA-027, PCA-083, PCA-085)
20. Wire `imageQuality` into the image encode path on both platforms, or remove the slider (PCA-082)
21. Fix cloud sync `build_local_item` to carry `pinned`/`pin_order` from the Supabase row (PCA-017)
22. Add an opt-in `auto_apply_synced_clip` toggle on Android; stop force-overwriting the clipboard (PCA-055)
23. Wire Android sync-key rotation (`revoke_and_rotate`) into the Devices revoke flow (PCA-060)

### Group D — Reliability / Atomicity / Data Integrity

24. Fix `has_sensitive_items` to return `Result<bool>` instead of swallowing DB errors (PCA-031)
25. Add a proper migration for the `revoked_devices` table (PCA-032)
26. Set timeouts on the fallback `reqwest::Client` and the IPC test-connection client (PCA-033)
27. Add a retry queue + `BackoffScheduler` on relay push non-401 transient errors (PCA-034)
28. Store and drop JoinHandles for relay/P2P tasks; log panics and restart subsystems (PCA-035)
29. Treat a poisoned sync key-cache Mutex as fatal; restart the sync subsystem (PCA-036)
30. Wrap P2P `push_catchup` `sink.send().await` in `tokio::time::timeout` (PCA-037)
31. Apply bounded batch fetching in `migration_v4` instead of `i64::MAX` (PCA-038)
32. Persist the relay watermark to DB/JSON so restarts resume from the last-seen cursor (PCA-039)
33. Fix Android relay image/file ingest to use `storeItemWithLww` instead of plain `storeItem` (PCA-040)
34. Wrap IPC `read_until` in a 30s `tokio::time::timeout` (PCA-047)
35. Gate `lsappinfo` fork on the exclusion list being non-empty; set `MissedTickBehavior::Skip` (PCA-048)

### Group E — Test Coverage / CI

36. Add storage unit tests and a non-ignored daemon IPC harness; extend `snapshot.rs` to cover all clipboard methods (PCA-099)
37. Add the seven missing security test categories (cross-device token, FTS-on-TTL, PAKE confirm, nonce, wrong-key, recall corpus, ErrorCode round-trip) (PCA-111)
38. Fix CI: add ESLint, `cargo test --all-features`, Android lint on PRs, emulator instrumented tests, promote fuzz to blocking (PCA-110)
39. Add on-disk migration ladder tests for v5–v11; append FTS5 `rebuild` to the vacuum handler (PCA-108)
40. Add Android unit tests for capture dispatch, key storage, service restart, and loaded-`.so` FFI error mapping (PCA-118)

### Group F — UX Polish / Observability

41. Fix macOS LogView: friendly error classification, retry, RestartDaemonButton, tilde-collapsed path, ≥2 tests (PCA-073)
42. Add a `private_mode` empty state in macOS HistoryView (PCA-080)
43. Add polling after `requestAccessibilityPermission` to confirm grant; show a success state (PCA-075)
44. Surface Android sync failures (auth, credential, transport) via notification or Settings banner (PCA-056)
45. Wrap each macOS `<View>` in its own `<ErrorBoundary>` to avoid full-window blanking (PCA-076)
46. Fix HistoryView to refresh after a successful import (PCA-074)
47. Fix tray Private Mode checkmark to re-sync after daemon restart (PCA-078)
48. Reduce SyncStatusChip polling interval to ≤2s or switch to event-driven push (PCA-079)

### Group G — Architecture / Docs / Debt

49. Make `protocol.rs` (string-id) the authoritative IPC definition; retire the duplicate `u64`-id `copypaste-ipc` types (PCA-010)
50. Update ARCHITECTURE.md dep edges + pins; centralize `QR_PAIRING_TTL_SECS`; wire relay rate-limit constants into the governor (PCA-119)
51. Wire or remove the `copypaste-telemetry` crate; update privacy-policy docs to match (PCA-030)
52. Obtain an Apple Developer ID cert and notarise the macOS release binary; remove the Cask `xattr` strip (PCA-097)
53. Add `depends_on formula: "sqlcipher"` to the Cask; add daemon-stop to the restore script; add a round-trip backup CI test (PCA-096)
