# CopyPaste — Consolidated Feature Gap Report

This report consolidates **20 source audit fragments** (8 per-scope "MINE" audits in the native bd-title-uk format, and 12 folded-in "EXTERNAL" audits using mixed MEDIUM/LOW/INFO and P0–P4 severity scales). Across those fragments there were roughly **240 raw findings** (counting every finding, sub-finding, and PASS/no-issue note). After normalizing severity to P0–P4 (MEDIUM→P2, LOW→P3, INFO→P4; explicit P-levels kept as-is), dropping pure PASS/no-issue/positive observations, and merging findings that share a root cause (same feature + file:line + symbol), there are **128 unique findings** catalogued below as `PCA-001`…`PCA-128`, ordered by severity (all P0 first, then P1, …) then by area. IDs were assigned in pass order within each severity band; the lower-severity bands (P3/P4) include several deliberately grouped entries that fold many small same-theme source findings into one traceable block (every contributing source ID is preserved in each entry's `sources:` field).

Merges of note (per the known-overlap clusters, all verified): telemetry `PiiScrubber` dead-code (SENS-03 + LOG-01 + CMP-003 + B-crypto context) → one entry; sensitive plaintext in FTS / `search` (SENS-02 + C-F01 + D-I9.1) → one; `export --include-sensitive` plaintext dump + image/file skip (B-F01 + UI-09 + STOR-02 + C10.3 + B-F09) → two related entries; hardcoded "Verified" trust badge + trust-on-assert `pair_peer` (AND-05 + PAIR-02/03 + UX-20 + CMP-012) → two entries; Android dead settings (sync_on_wifi_only/excludedAppBundleIds/imageQuality/lanVisibility/maxHistoryItems/pasteAsPlainText: AND-06..09/24/25 + CMP-001/019 + UX-12 + I-parity) → six entries; `sync_enabled` stub (UI-01/06 + F-4 + CMP-002 + UX-10) → one; Android `private_mode` dead toggle (SENS-01 + AND "private mode") → one; `ip_with_port` autowipe (C-F10 + SENS-06) → one; cloud strips pins / tombstone resurrection (F-04/F-05 sync) → two; Android background-capture self-contradiction (AND-03 + G-F3 + A-arch) → one; silent DB-key regen P0 (AND-01 + G context) → one; CI gaps (H-F01..F12 + TC-* + REL-02) → grouped; `upsert_fts`/`revoke_device` non-atomicity (C-F02 + R-01 / R-02) → two; relay watermark not persisted (F-01 sync + R-15) → one; `SyncEngine`/`LamportClock` dead (F-08 sync + S1 + clock) → one; `SyncBadgeState::Syncing` never emitted (F-19 sync + UX-11 + F-13 sync) → one; LogView no offline/tests (UI-02/11 + F-3 macos) → one; macOS bulk-delete no confirm (CLIP-07 + UX-01) → one.

## Severity histogram

| Severity | Count |
|----------|-------|
| P0 | 1 |
| P1 | 28 |
| P2 | 68 |
| P3 | 28 |
| P4 | 3 |
| **Total** | **128** |

## Release-blocker index

Findings explicitly marked `release blocker: yes` by at least one contributing source (or whose merged severity is release-gating).

| PCA-ID | severity | title | platforms |
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

Notes: PCA-007 (sync_enabled) and PCA-016 (cloud tombstone) carry conditional blockers (one source marks them release-blocking; verify reachability/backend state — see each entry). PCA-097 is a beta-OK / 1.0-blocker. A second tier of findings was flagged release-blocking by the P2-reliability-ux audit (it marks its entire P1+P2 set "blocks release") — those map to PCA-005, PCA-022, PCA-025–029, PCA-031–038, PCA-055–059, PCA-079–081, PCA-082, PCA-084, PCA-085 and are individually scored here on the consolidated rubric rather than carried as hard blockers; review them before a release cut.

(Remaining findings are not release-blockers; full list follows.)

---

## Findings

### PCA-001 — Silent encryption-key regeneration wipes entire Android clipboard history
- severity: P0
- status: risky
- platforms: Android
- files: `android/app/src/main/java/com/copypaste/android/Settings.kt:986-1033` (regen at `:1020`), `DeviceKeyStore.kt`
- expected: If the KEK cannot unwrap the stored DB key (keystore cleared, OS upgrade invalidating keys, restore to a different device), the user is told their history is unreadable and asked to confirm a reset — never silent loss.
- actual: On `unwrapKey` failure the code logs a warning, deletes the wrapped key, and falls through to `ByteArray(32).also { SecureRandom().nextBytes(it) }`, minting a fresh random key. The SQLCipher DB was opened with the old key; under a new key every previously stored ciphertext is permanently undecryptable. The inline comment acknowledges the loss but the user is never signalled.
- why it matters: Irreversible, silent loss of the entire local clipboard history. KEK invalidation happens from routine events (some OEM OS updates, lock-screen credential changes on older APIs).
- recommended fix: On unwrap failure set a one-shot pref flag, surface a persistent notification/banner ("encryption key lost — history unreadable; reset?"), and require explicit user confirmation before regenerating.
- test plan: Robolectric/instrumented test: wrap a key, simulate unwrap failure (corrupt/clear the KEK alias), assert the app does NOT silently regenerate without setting the warning flag.
- release blocker: yes
- sources: AND-01 (audit-android), G-android context
- bd-title-uk: Тихе перегенерування ключа шифрування у `Settings.loadOrCreateKey` знищує всю історію
- bd-type: bug
- bd-priority: 0

### PCA-002 — Sensitive plaintext indexed in FTS and returned by the `search` IPC method
- severity: P1
- status: risky
- platforms: macOS, daemon, Linux
- files: `crates/copypaste-core/src/storage/items.rs:446-453,1648-1683`, `crates/copypaste-daemon/src/ipc.rs:3519-3551`
- expected: `search` should not return sensitive items' plaintext spans, or `search_items` should exclude `is_sensitive = 1` rows by default (matching how relay push and P2P sync skip them).
- actual: `insert_item_with_fts` writes sensitive items' plaintext into `clipboard_fts`; `search_items` runs `WHERE clipboard_fts MATCH ?1 AND ci.deleted = 0` with no `is_sensitive` filter, returning the item with a fully-decrypted `preview` over the IPC socket. UI blurs it client-side, but the plaintext crosses the socket. Note I-parity refutes any *cross-platform drift* here (Android FTS behaves identically) — both index sensitive plaintext; this entry is the underlying policy/leak concern, not a parity gap.
- why it matters: Any process sharing the daemon UID can call `search` with a common prefix (`AKIA`, `sk-`, `Bearer `) and retrieve sensitive credentials in plaintext, bypassing encryption-at-rest.
- recommended fix: Preferred — pass empty `plaintext_for_fts` when `item.is_sensitive`, plus a one-time sweep to purge existing sensitive FTS rows. Weaker — add `AND ci.is_sensitive = 0` to `search_items` and strip sensitive items from the response. Either way, decide and document the searchable-vs-masked policy consistently across macOS/Android (see also PCA-061).
- test plan: Insert a sensitive AWS key, call `search "AKIA"`, assert empty result and no `clipboard_fts` row for that id.
- release blocker: yes
- sources: SENS-02 (audit-sensitive), C-storage F-01, D-daemon-ipc I9.1, I-parity refuted-3 (parity aspect), CLIP-15 (audit-clipboard, masking aspect)
- bd-title-uk: `search_items` повертає чутливі елементи — `clipboard_fts` індексує їх попри `is_sensitive = 1`
- bd-type: bug
- bd-priority: 1

### PCA-003 — Android `private_mode` toggle is a dead control; capture is not suppressed
- severity: P1
- status: broken
- platforms: Android
- files: `android/app/src/main/java/com/copypaste/android/Settings.kt:840`, `ClipboardService.kt:162`, `SettingsActivity.kt:204`
- expected: Enabling Private Mode on Android stops clipboard capture, mirroring macOS where `daemon.rs:1697` checks `private_mode.load()` in `handle_tick`.
- actual: `Settings.privateMode` is a `SharedPreferences` boolean surfaced in the UI, but `ClipboardService` reads only `settings.captureEnabled` — `privateMode` is never read by the capture path. Items copied in "private mode" are silently stored and potentially synced.
- why it matters: A user who enables Private Mode expects capture to halt (as documented in the settings subtitle); instead sensitive clips are stored.
- recommended fix: In `dispatchClipData`/`clipListener`, add `if (!settings.captureEnabled || settings.privateMode) return`; update `prefsListener` to react to `"private_mode"` changes.
- test plan: Unit test: `settings.privateMode = true` → `clipListener` fires → assert `repository.storeItem` not called.
- release blocker: yes
- sources: SENS-01 (audit-sensitive), audit-android (Private mode row)
- bd-title-uk: Android `private_mode` не блокує захоплення буфера — `ClipboardService` ігнорує `Settings.privateMode`
- bd-type: bug
- bd-priority: 1

### PCA-004 — Sensitive images and files silently dropped at capture on Android
- severity: P1
- status: broken
- platforms: Android
- files: `android/app/src/main/java/com/copypaste/android/ClipboardService.kt:1050-1053` (image), `:1226-1230` (file); UDL `crates/copypaste-android/uniffi/copypaste_android.udl:39-42,73-74`
- expected: Per the UDL migration, the sensitive early-returns for image/file are removed and all three content types route through `sensitive_capture_decision`, storing sensitive items with `is_sensitive=true` + TTL (matching the macOS daemon, which stores them).
- actual: Migration half-done. Text no longer early-returns, but `captureImageClip`/`captureFileClip` still `if (sensitive) { …; return }`. Worse, `sensitive_capture_decision` is never called anywhere; text sensitivity is recomputed lazily at read time. A password-manager screenshot or `passwords.csv` is silently never captured on Android, while the same content syncs fine from macOS.
- why it matters: (a) cross-device parity broken for images/files; (b) capture-time vs read-time sensitivity verdicts diverge; the user has no idea their sensitive image/file was dropped.
- recommended fix: Remove the early-returns and route all three capture paths through `sensitive_capture_decision`/`storeClipboardItem` so `is_sensitive`/`expires_at` are stamped once at capture.
- test plan: Unit-test a capture-decision helper for sensitive image/file URIs: assert stored with `isSensitive=true` + expiry, not dropped.
- release blocker: yes
- sources: AND-02 (audit-android), F-16 (audit-sync, ingest side context)
- bd-title-uk: `captureImageClip`/`captureFileClip` досі відкидають sensitive елементи замість виклику `sensitive_capture_decision`
- bd-type: bug
- bd-priority: 1

### PCA-005 — Trust label hardcoded "Verified" on both platforms; no daemon-sourced trust field
- severity: P1
- status: stub
- platforms: macOS, Android, daemon, IPC
- files: `crates/copypaste-ui/src/components/DeviceCard.tsx:313-324`, `android/.../DevicesActivity.kt:222` (`trustLabel() = "Verified"`, rendered green at `:1570-1584`), `crates/copypaste-daemon/src/ipc.rs:5935-5996` (no `verified`/`trust` key in `list_peers`)
- expected: Trust state shown to the user is derived from a daemon-authoritative signal (paired-via-SAS / paired-via-QR-PAKE / imported-legacy-unverified) so the badge cannot lie and unverified entries are visually distinct.
- actual: Both UIs compute the badge as a constant green "Verified" for any non-null peer; `list_peers` emits no trust field. The claim rests on the assumption that every roster entry completed SAS — which `pair_peer` (PCA-006) violates.
- why it matters: A security indicator that can never show distrust trains users to trust the badge while it conveys nothing; any non-SAS-added peer still reads "Verified."
- recommended fix: Add a `trust`/`pairing_method` field to persisted `PairedDevice` and to `list_peers`; render from it on both UIs; default existing records conservatively + document migration.
- test plan: Daemon: `list_peers` includes `trust`; UI tests assert badge text follows the field including an unverified case.
- release blocker: no
- sources: AND-05 (audit-android), PAIR-03 (audit-pairing-devices), UX-20 (P2-reliability-ux), I-parity refuted-2 (web badge exists), F-macos-tauri context
- bd-title-uk: Мітка довіри жорстко закодована "Verified"; немає поля trust у `list_peers` і немає стану unverified
- bd-type: feature
- bd-priority: 1

### PCA-006 — Legacy `pair_peer` trusts a peer by asserted fingerprint with no PAKE/SAS
- severity: P1
- status: risky
- platforms: daemon (IPC)
- files: `crates/copypaste-daemon/src/ipc.rs:6305-6366`
- expected: Every path that adds a peer to the trusted roster + live mTLS allowlist requires cryptographic proof (PAKE) and/or human SAS confirmation, consistent with the QR/discovery paths.
- actual: `pair_peer` accepts `{fingerprint, name}`, validates only fingerprint *format*, pushes into `peers.json`, and calls `register_live_peer` — registering it in the live mTLS allowlist with no PAKE/SAS/key exchange. Any IPC client (the socket is the trust boundary) can add an arbitrary trusted peer. No `METHOD_*` constant, no UI/CLI caller (CMP-012), but the live handler exists.
- why it matters: Trust-state integrity hole; combined with PCA-005 a `pair_peer`-added peer shows "Verified". mTLS allow-listing alone is a meaningful trust escalation.
- recommended fix: Remove `pair_peer` if dead; or gate it so it cannot register into the live allowlist without completed PAKE/SAS; or mark such entries un-verified and surface it (ties to PCA-005).
- test plan: `rg` for `pair_peer"` callers; if none, delete + assert the method errors. If kept, assert it does not call `register_live_peer` without proof.
- release blocker: yes (if reachable from a shipping affordance — verify)
- sources: PAIR-02 (audit-pairing-devices), CMP-012 (P1-completeness)
- bd-title-uk: Legacy `pair_peer` додає довірений peer без PAKE/SAS і реєструє його в live mTLS allowlist
- bd-type: bug
- bd-priority: 1

### PCA-007 — `sync_enabled` master kill-switch is a daemon-side stub (or UI text drift)
- severity: P1
- status: stub
- platforms: macOS, Android
- files: `crates/copypaste-ui/src/lib/ipc.ts:316-323`, `crates/copypaste-ui/src/views/SettingsView.tsx:718-722,1572-1587,2023-2068`; backend (per P1-completeness) `crates/copypaste-core/src/config/mod.rs:129`, consumers in `sync_orch.rs`/`relay.rs`/`cloud.rs`/`daemon.rs`
- expected: "Enable sync" toggle disables all sync transports when off, and the UI accurately reflects whether it works.
- actual: Conflicting evidence across fragments. The macOS-UI/clipboard/sync audits report the toggle is sent via `set_config` but the daemon's `AppConfig` has no `sync_enabled` field, so it is silently ignored; the UI visually gates per-transport switches but sync keeps running, and the InfoPopover warns "Requires a daemon update (CopyPaste-j9xj)". The completeness audit reports the backend DOES now implement it (default true, gates all three transports, with contract tests) and that only the UI warning copy is stale. Either way there is a real defect: a safety-critical control whose UI text and daemon behavior disagree.
- why it matters: If the daemon ignores it, a user pausing sync still shares clipboard data (privacy gap). If the backend works, users distrust/avoid a working feature because of the shipped "stub" warning.
- recommended fix: Verify the daemon `AppConfig::sync_enabled` end-to-end; if implemented, delete the stale "daemon update"/"stub" comments and InfoPopover clause; if not, implement it and honour it in the sync dispatch, then remove the warning.
- test plan: Integration test that toggling off stops IPC sync events / outbound pushes; RTL test that the InfoPopover no longer says "daemon update"/"stub".
- release blocker: yes
- sources: UI-01/UI-06 (audit-macos-ui-settings), CLIP-13 (audit-clipboard), F-4 (F-macos-tauri), CMP-002 (P1-completeness), UX-10 (P2-reliability-ux)
- bd-title-uk: Перемикач `sync_enabled` — неузгодженість UI↔daemon (stub або застаріле попередження)
- bd-type: bug
- bd-priority: 1

### PCA-008 — `export --include-sensitive` dumps all decrypted history over IPC/stdout, unguarded
- severity: P1
- status: risky
- platforms: macOS, CLI, Linux
- files: `crates/copypaste-daemon/src/ipc.rs:8028,8164-8190`, `crates/copypaste-cli/src/commands/export.rs:68-78` (stdout path), `crates/copypaste-ui/src/views/SettingsView.tsx:2499-2534` (weak warning)
- expected: Exporting sensitive items requires explicit, hard-to-bypass acknowledgment; non-`--output` (stdout) export should not silently emit decrypted content.
- actual: With `include_sensitive=true` ALL sensitive items are serialized as base64 plaintext into the JSON response over the 0600 socket with only an `info!` count log. The CLI writes the same JSON (incl. non-sensitive plaintext, which can contain unmarked secrets) to stdout when `--output` is absent, with no warning. The macOS UI shows a warning only while the checkbox is checked, no final confirm, generic filename, direct Blob download.
- why it matters: A local process or socially-engineered CLI invocation exfiltrates the entire clipboard history (incl. credentials) in one unauthenticated call; `export | …` can ship full history into a log/pipe unintentionally.
- recommended fix: Require an operator-level confirmation (daemon-minted one-time token) before accepting `include_sensitive=true`; log the exporting PID at `warn`; in the CLI require `--output` or an explicit `--stdout` opt-in (mirroring `pair-qr --raw`); in the UI add a mandatory confirm dialog and `-SENSITIVE` filename suffix.
- test plan: assert default `include_sensitive=false` excludes sensitive rows; assert CLI stdout export without `--stdout` warns/aborts; UI test asserting confirm dialog before download when sensitive included.
- release blocker: yes
- sources: B-crypto F-01, UI-09 (audit-macos-ui-settings), C10.3 (D-daemon-ipc)
- bd-title-uk: `export --include-sensitive` віддає весь розшифрований вміст через IPC/stdout без захисту
- bd-type: bug
- bd-priority: 1

### PCA-009 — `revoke_all_peers` is destructive but lacks a strong UI confirmation
- severity: P1
- status: risky
- platforms: macOS, Android, daemon
- files: `crates/copypaste-daemon/src/ipc.rs:6650-6700` (handler correct/atomic), `crates/copypaste-ui/src/lib/ipc.ts:832`, `crates/copypaste-ui/src/views/DevicesView.tsx:1194-1222` (inline 12px Yes/No)
- expected: A bulk "revoke every pairing" action is guarded by an explicit modal confirmation explaining that all P2P trust keys are revoked and re-pairing is required.
- actual: The daemon handler is atomic, but the macOS UI gates it behind only a tiny inline Yes/No button pair (no modal, no consequence explanation); per-peer dialogs exist but the bulk path is weak. Android bulk path unverified.
- why it matters: An accidental misclick revokes trust with all devices — a network-trust-breaking, irreversible action forcing re-pairing of every device.
- recommended fix: Replace the inline confirm with a modal dialog with an explicit destructive-action button; verify/equalize on Android.
- test plan: Click "Revoke all" → assert a cancellable modal appears; cancel → assert IPC not called.
- release blocker: yes
- sources: UI-04 (audit-macos-ui-settings), UX-02 (P2-reliability-ux), PAIR-05 (audit-pairing-devices)
- bd-title-uk: "Відкликати всі пристрої" деструктивна дія без надійного діалогу підтвердження
- bd-type: bug
- bd-priority: 1

### PCA-010 — Two divergent, wire-incompatible IPC protocol definitions (`id: u64` vs `id: String`)
- severity: P1
- status: risky
- platforms: daemon, IPC, CLI
- files: `crates/copypaste-ipc/src/request.rs:18` + `response.rs:48` (`id: u64`); `crates/copypaste-daemon/src/protocol.rs:40-49,85-102` (`id: String`, the live wire type); `crates/copypaste-cli/src/ipc.rs:52-54,351` (`id: String`)
- expected: One authoritative protocol definition describes the live wire; the published `copypaste-ipc` crate matches it.
- actual: The daemon dispatch returns `crate::protocol::Response` (`id: String`) and the CLI sends `"id":"1"` — but the `copypaste-ipc` crate (which claims wire-compatibility) defines a parallel near-duplicate type set with `id: u64`. Any consumer deserializing real frames into `copypaste_ipc::Response` fails on the string id. Two copies of `ERR_CODE_*` and two `ErrorCode`-equivalents must be hand-synced.
- why it matters: Maintenance hazard + a misleading "shared protocol" crate that does not describe the wire; silent drift risk across three clients.
- recommended fix: Make the string-id `protocol.rs` authoritative; have `copypaste-ipc` re-export it; delete/convert the `u64` types.
- test plan: a cross-crate round-trip test serializing a daemon `Response` and deserializing via `copypaste_ipc` (would have caught the divergence).
- release blocker: no
- sources: D-daemon-ipc I5.1
- bd-title-uk: Дві розбіжні несумісні визначення IPC-протоколу (`id: u64` проти `id: String`)
- bd-type: bug
- bd-priority: 1

### PCA-011 — `persist_private_mode` writes the flag file world-readable (0644)
- severity: P1
- status: risky
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/daemon.rs:2966` (vs `write_text_atomic_0600` at `:2833`)
- expected: The private-mode flag file is written 0600 like config.
- actual: `std::fs::write` creates `…/CopyPaste/private_mode` at umask (usually 0644). The "0"/"1" content leaks the private-mode behavioral fact to any local user; the daemon already has `write_text_atomic_0600` but bypasses it here.
- why it matters: Minor local information disclosure of a privacy-relevant behavioral flag.
- recommended fix: Use `write_text_atomic_0600`.
- test plan: adapt the `ipc_socket_chmod_is_0600` pattern; assert the flag file is mode 0600.
- release blocker: no
- sources: D-daemon-ipc D2.1
- bd-title-uk: `persist_private_mode` пише файл прапорця світо-читабельним (0644) замість 0600
- bd-type: bug
- bd-priority: 1

### PCA-012 — `NSFilenamesPboardType` fallback treats a plist XML array as a `file://` URL → files dropped
- severity: P1
- status: broken
- platforms: macOS
- files: `crates/copypaste-daemon/src/clipboard.rs:499-534` (esp. `:502-503`, silent drop `:529`)
- expected: Files copied from legacy Cocoa apps that set only `NSFilenamesPboardType` are captured.
- actual: The fallback calls `stringForType(NSFilenamesPboardType)`, which returns a plist-encoded XML array, not a URL. `strip_prefix("file://")` + `percent_decode_path` + `is_absolute()` all fail (the string starts with `<`), and the capture is silently dropped with no log.
- why it matters: A whole class of file copies (legacy apps) never enters history, silently.
- recommended fix: Decode via `propertyListForType:` into an `NSArray`, take the first `NSString`.
- test plan: copy a file from an app setting only `NSFilenamesPboardType`; assert an item is captured.
- release blocker: no
- sources: D-daemon-ipc D2.2
- bd-title-uk: Фолбек `NSFilenamesPboardType` трактує plist-масив як `file://` URL — файли мовчки відкидаються
- bd-type: bug
- bd-priority: 1

### PCA-013 — PiiScrubber not applied to the daemon `tracing` log sink (PII leak risk)
- severity: P1
- status: risky
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/logging.rs:65-66,78-88`, `crates/copypaste-telemetry/src/scrubber.rs`
- expected: A scrubbing `Layer` (or a guaranteed no-sensitive-fields invariant) protects the daily JSON log written to `~/Library/Logs/CopyPaste/daemon.log`.
- actual: The file log layer is a raw `fmt::layer().json()` with no `PiiScrubber` wrapper; the scrubber only runs inside the (unwired) Sentry path. Audited call-sites log only counts today, but any future `tracing::debug!(key = ?key_hex)` or content snippet would persist verbatim in an unencrypted log.
- why it matters: Logs are harvestable by Xcode Console / crash reporters / enterprise MDM; clipboard content or key material in logs would be a direct privacy violation.
- recommended fix: A `tracing_subscriber::Layer` applying `PiiScrubber::scrub()` before the file appender; interim, keep prod level at `info` and add a CI lint failing on forbidden field names (`content`/`plaintext`/`key`/`password`/`token`) in `debug!`/`trace!`.
- test plan: capture-buffered subscriber emits an event with a fake email field; assert persisted output is scrubbed.
- release blocker: yes (before any production log collection)
- sources: LOG-02 (audit-storage-logs-release)
- bd-title-uk: daemon logging: `PiiScrubber` не застосовується до tracing-sink — ризик витоку PII у лог-файли
- bd-type: bug
- bd-priority: 1

### PCA-014 — `evict_stale_daemon` SIGTERMs an IPC-reported PID without kernel validation
- severity: P1
- status: risky
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/ipc.rs:8814-8863` (pid read `:8793`, kill `:8831`)
- expected: Before signaling, the daemon confirms the target PID is actually a CopyPaste daemon.
- actual: The PID comes from the peer's `status` JSON; any process winning the socket path can report an arbitrary PID. Guards block 0/1/self but not PID recycling (TOCTOU between probe and `kill`). An unrelated recycled PID could receive SIGTERM.
- why it matters: A spoofed/recycled PID could be killed by the takeover path.
- recommended fix: Verify the PID's executable (`proc_pidpath` / `/proc/N/exe`) is the CopyPaste daemon before signaling, or refuse takeover on start-time/socket-mtime disagreement.
- test plan: add a spoofed-pid case asserting no signal is sent to a non-daemon PID.
- release blocker: no
- sources: D-daemon-ipc D3.2
- bd-title-uk: `evict_stale_daemon` надсилає SIGTERM на PID зі `status` JSON без перевірки виконуваного файлу
- bd-type: bug
- bd-priority: 1

### PCA-015 — Android release APK falls back to debug-signed when keystore secrets are absent
- severity: P1
- status: risky
- platforms: Android
- files: `.github/workflows/release.yml:350-366`, `scripts/build-android-apk.sh:124` (`KEYSTORE_PASS="copypaste-beta"` fallback)
- expected: A release-tag push always produces a release-signed APK; a debug-signed APK cannot upgrade over a release-signed install (`INSTALL_FAILED_UPDATE_INCOMPATIBLE`).
- actual: The pipeline emits a warning but continues debug-signed when `ANDROID_KEYSTORE_BASE64` is unset; the build script silently uses a known fallback password and writes a keystore not in `.gitignore`. A debug-signed APK published under a release tag breaks all future upgrades for installed users.
- why it matters: After the first public install base exists, an accidental keystore-less release strands users on an un-upgradable build, and the well-known fallback password is an attack surface.
- recommended fix: Make `build-android` **fail** (not warn) when `ANDROID_KEYSTORE_BASE64` is absent on a tag ref (`startsWith(github.ref,'refs/tags/')`); keep debug fallback for PR/dispatch only; gitignore `keystore-beta.jks`; require `KEYSTORE_PASS` explicitly.
- test plan: trigger release on a tag with `ANDROID_KEYSTORE_BASE64` unset; assert the job fails with a meaningful error.
- release blocker: yes (for any tag after first public APK)
- sources: REL-02 (audit-storage-logs-release), H-tests-ci F-06
- bd-title-uk: Android Release: відсутній keystore → debug-підписаний APK у release тегу — ризик несумісності оновлень
- bd-type: bug
- bd-priority: 1

### PCA-016 — `replace_cloud_item_by_item_id` INSERT may omit `deleted` → tombstone resurrection
- severity: P1
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-daemon/src/sync_common.rs` (`replace_cloud_item_by_item_id`)
- expected: When a Supabase tombstone row (`deleted=true`) is downloaded, the local row is marked `deleted=1`.
- actual: The INSERT does not include the `deleted` column; if the tombstone ingest path goes through this function, the local row gets the SQLite default `deleted=0`, silently resurrecting a deleted item. (Needs final verification: cloud.rs may route tombstones through a separate `soft_delete_item`/`insert_tombstone` fast-path; relay.rs uses those correctly. cloud.rs is 263 KB and was only partially read.)
- why it matters: A deletion on device A could reappear on device B after cloud sync — silent data integrity violation.
- recommended fix: Add the `deleted` column to the INSERT, or confirm and document that tombstones always take the `soft_delete_item` path.
- test plan: soft-delete on A via cloud, sync to B, assert B marks deleted (not resurrected).
- release blocker: yes (if confirmed reachable)
- sources: F-05 (audit-sync), E-sync-relay cross-cutting (tombstone/dedup)
- bd-title-uk: `replace_cloud_item_by_item_id` INSERT не включає `deleted` — потенційне воскресіння видаленого item
- bd-type: bug
- bd-priority: 1

### PCA-017 — Cloud (Supabase) sync does not carry `pinned`/`pin_order` to macOS
- severity: P1
- status: broken
- platforms: macOS
- files: `crates/copypaste-daemon/src/sync_common.rs` (`build_local_item()` hardcodes `pinned: false, pin_order: None`)
- expected: Pins sync bidirectionally over cloud; a pinned item on device A appears pinned on device B after cloud sync.
- actual: `build_local_item` ignores `pinned`/`pin_order` from the cloud row, so a Supabase row with `pinned=true` is stored `pinned=false`. P2P carries it correctly via LWW; Android relay calls `applyAuthoritativePinState`; cloud macOS strips it.
- why it matters: Pin state silently lost across cloud sync — a visible feature gap and divergence between transports.
- recommended fix: Pass `CloudClipboardRow.pinned`/`pin_order` into `build_local_item` and the INSERT, or call `apply_pin_state` after cloud ingest.
- test plan: pin on macOS, upload to Supabase, poll from a second macOS device, assert pin present.
- release blocker: no
- sources: F-04 (audit-sync), E-sync-relay context
- bd-title-uk: cloud (Supabase) sync не переносить `pinned` + `pin_order` на macOS
- bd-type: bug
- bd-priority: 1

### PCA-018 — `upsert_fts` non-atomic DELETE+INSERT → items permanently unsearchable
- severity: P1
- status: risky
- platforms: macOS, Android, Linux
- files: `crates/copypaste-core/src/storage/items.rs:1484-1496` (public re-export at `lib.rs:55`)
- expected: FTS index update is atomic — a crash leaves the row searchable or not, never missing from FTS.
- actual: Standalone `upsert_fts` runs `DELETE` then `INSERT` as two separate autocommit `execute()` calls with no wrapping transaction (the primary capture path `insert_item_with_fts` is atomic; `upsert_fts` is used for post-decryption backfill). A crash/concurrent writer between the two leaves the item permanently unsearchable with no repair path.
- why it matters: Silent search data loss — the user cannot find items they know were copied.
- recommended fix: Wrap the DELETE+INSERT in `conn.unchecked_transaction()`, mirroring `insert_item_with_fts:447-454`.
- test plan: insert, kill process between the two statements (debug panic hook), reopen DB, confirm item appears in FTS.
- release blocker: yes
- sources: C-storage F-02, R-01 (P2-reliability-ux)
- bd-title-uk: `upsert_fts` неатомарний DELETE+INSERT — елемент може назавжди зникнути з пошуку
- bd-type: bug
- bd-priority: 1

### PCA-019 — `revoke_device` non-atomic DELETE+INSERT → lost revocation audit
- severity: P1
- status: risky
- platforms: macOS, Android
- files: `crates/copypaste-core/src/storage/devices.rs:74-93`
- expected: Peer removal and revocation audit entry commit atomically (the batch `revoke_devices()` already does this).
- actual: Two separate autocommit statements; a crash between them drops the revocation record while removing the device — the device is unpairable again with no audit trail.
- why it matters: Security-audit integrity; a revoked device may appear clean.
- recommended fix: Wrap in `unchecked_transaction()`.
- test plan: simulate crash mid-revoke; assert `revoked_devices` has the entry and `devices` does not.
- release blocker: yes
- sources: R-02 (P2-reliability-ux)
- bd-title-uk: `revoke_device` неатомарний DELETE+INSERT — втрата запису про відкликання
- bd-type: bug
- bd-priority: 1

### PCA-020 — macOS bulk delete has no confirmation and no undo
- severity: P1
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/views/HistoryView.tsx:2190-2218` (bulk loop) vs `:2034-2058` (single-delete undo)
- expected: Destructive bulk delete gets at least the same safety as single delete (5s undo) or a confirmation modal; Android bulk delete confirms.
- actual: Single-item delete is a 5s-deferred optimistic delete with an Undo toast; the BulkActionBar multi-select delete fires immediately with no undo and no confirm. Select-all + Delete destroys the entire history silently.
- why it matters: Irreversible multi-item loss with one click; inconsistent with single-delete safety and Android's bulk confirm.
- recommended fix: Gate `handleBulkDelete` behind a `ConfirmModal` and/or a batched undo window.
- test plan: select all, click Delete → assert confirmation modal; cancel → items remain.
- release blocker: yes
- sources: CLIP-07 (audit-clipboard), UX-01 (P2-reliability-ux)
- bd-title-uk: Масове видалення в macOS HistoryView без підтвердження та без undo
- bd-type: bug
- bd-priority: 1

### PCA-021 — Android single-item delete: no confirmation/undo, propagates tombstone to peers
- severity: P1
- status: risky
- platforms: Android
- files: `android/app/src/main/java/com/copypaste/android/HistoryActivity.kt:3214-3221` (text), `:2879-2888` (image), `:2985-2992` (file), `:1447-1452` (preview overlay)
- expected: Destructive single deletes are confirmed or undoable, consistent with bulk delete/clear-all which route through `ConfirmationDialog`, and with macOS's 5s undo.
- actual: A single-row trash tap calls `onDelete(item.id)` → `repository.deleteItem`, writing an irreversible soft-delete tombstone that propagates to every synced device. No confirm, no undo. The pin and delete icons are adjacent (high misclick risk).
- why it matters: One mis-tap permanently destroys a clip locally and on all paired devices.
- recommended fix: Gate single/preview delete through `ConfirmationDialog` (add `DELETE_ONE`) or a swipe-to-delete with undo snackbar (parity with macOS).
- test plan: tap row delete → assert a confirm/undo appears before the tombstone is written; tap Undo within window → row restored, repository delete never fired.
- release blocker: yes
- sources: AND-04 (audit-android), UX-03 (P2-reliability-ux), P1-2 (I-parity)
- bd-title-uk: Видалення одного рядка історії у Android без підтвердження/undo — поширюється на всі пристрої
- bd-type: bug
- bd-priority: 1

### PCA-022 — macOS "Clear history" and "Revoke all" use misclick-prone inline confirms
- severity: P1
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/views/SettingsView.tsx:2590-2625` (clear-history inline Yes/No), `DevicesView.tsx:1194-1222` (revoke-all inline; see PCA-009)
- expected: Irreversible actions use a modal confirmation with explicit irreversibility warning (and undo where feasible).
- actual: "Clear history" shows a tiny inline `deleteConfirm` Yes/No inside a dense settings row, no modal, no undo; same pattern for revoke-all. (This is the macOS clear-history facet; the bulk-history facet is PCA-020 and the revoke-all facet PCA-009.)
- why it matters: Misclick-prone destructive action in a dense row destroys all history with no recovery.
- recommended fix: Replace the inline confirm with a modal dialog with irreversibility warning.
- test plan: click "Clear history" → assert a modal confirm appears; cancel → items remain.
- release blocker: no
- sources: UX-07 (P2-reliability-ux)
- bd-title-uk: macOS "Clear history" вбудоване підтвердження схильне до випадкового кліку, без модалки/undo
- bd-type: task
- bd-priority: 1

### PCA-023 — Android "Clear All" from Settings swallows errors and skips sync-queue drain
- severity: P1
- status: broken
- platforms: Android
- files: `android/app/src/main/java/com/copypaste/android/SettingsActivity.kt:580-582`
- expected: Errors are surfaced; tombstones propagate to peers via the mutation-queue drain (as the HistoryActivity clear path does).
- actual: `scope.launch(Dispatchers.IO) { repository.clearAll() }` — fire-and-forget; exceptions swallowed, success assumed, and `ClipboardService.requestMutationQueueDrain()` is never called, so other devices never receive the clear.
- why it matters: Clear appears to succeed but may silently fail, and even on success peers are not cleared.
- recommended fix: Collect the `Result`, show an error toast on failure, and call `requestMutationQueueDrain()` after success.
- test plan: inject a `clearAll` failure → assert error surfaced; success → assert drain requested.
- release blocker: yes
- sources: UX-04 + UX-15 (P2-reliability-ux)
- bd-title-uk: Android "Clear All" у Settings ковтає помилки і не дренажить чергу мутацій
- bd-type: bug
- bd-priority: 1

### PCA-024 — Android shows raw exception text in user-facing toasts and the QR error surface
- severity: P1
- status: needs-improvement
- platforms: Android
- files: `android/app/.../ClipboardViewModel.kt:212,249,266,280,294,308,324,344,389`, `DevicesActivity.kt:1277`
- expected: Friendly, category-based error messages; raw detail only in logcat.
- actual: `e.message ?: e.javaClass.simpleName` is posted directly to `_errors` for all 9 operations and rendered in glass toasts; users see `DecryptionFailed`, `database is locked`, `NullPointerException`. The QR error surface renders the raw exception, which can expose socket paths / FFI error codes.
- why it matters: Confusing/scary errors and potential path/internal disclosure.
- recommended fix: Map known error types to string resources, fall back to a generic message + logcat detail; sanitize the QR error to a generic string.
- test plan: trigger each error path → assert a friendly message, not the raw class/message.
- release blocker: yes
- sources: UX-05 + UX-06 (P2-reliability-ux), UX-23 (P2 scan error)
- bd-title-uk: Android показує сирий текст винятків у тостах і помилці QR
- bd-type: bug
- bd-priority: 1

### PCA-025 — Android `sync_on_wifi_only` toggle is dead; sync runs on cellular
- severity: P1
- status: broken
- platforms: Android
- files: UI `SettingsActivity.kt:1210-1216`; persisted `Settings.kt:670-676`; no consumer in `FgsSyncLoop.kt`/`SupabasePollWorker.kt`/`SyncManager.kt` (macOS enforces it: `relay.rs:104,842`, `cloud.rs:915,1900`)
- expected: When on, sync push/poll is skipped on metered/cellular networks (parity with daemon).
- actual: No transport checks the flag; sync runs on cellular regardless. Only status-display code reads it.
- why it matters: Users on metered data are billed despite the toggle — a data-cost/privacy violation and misleading status.
- recommended fix: Gate each poll/push on `NET_CAPABILITY_NOT_METERED`/`TRANSPORT_WIFI` when `syncOnWifiOnly` is set; or remove the toggle.
- test plan: mocked metered network + flag on → assert push/poll skipped.
- release blocker: no
- sources: AND-06 (audit-android), I-parity, P1-completeness context
- bd-title-uk: `syncOnWifiOnly` не застосовується — синхронізація йде по стільниковій мережі
- bd-type: bug
- bd-priority: 1

### PCA-026 — Android `excludedAppBundleIds` privacy control is unenforceable
- severity: P1
- status: broken
- platforms: Android
- files: editable UI `SettingsActivity.kt:2200-2270`; persisted `Settings.kt:821-832`; no capture-time consumer in `ClipboardService.kt` (`ClipboardItem.sourceApp` never populated, `ClipboardItem.kt:48`). macOS enforces via `lsappinfo front` (`daemon.rs:1768-1795`).
- expected: Clips from excluded apps are never captured.
- actual: The list is editable and presented as "apps whose clipboard is never captured", but nothing consults it before `storeItem`, and Android has no API to resolve which app wrote the system clipboard — so it cannot work as designed.
- why it matters: A privacy control that silently does nothing — users believe a password manager is excluded when it is not.
- recommended fix: Remove the control from the Android UI or label it macOS-only; revisit only if source-app attribution becomes available.
- test plan: verify the control is hidden/labeled on Android.
- release blocker: no
- sources: AND-07 (audit-android), CMP-023 (P1-completeness), I-parity
- bd-title-uk: `excludedAppBundleIds` показано в Android UI, але ніколи не застосовується при захопленні
- bd-type: bug
- bd-priority: 1

### PCA-027 — Android `lanVisibility` toggle does nothing; device always discoverable
- severity: P1
- status: broken
- platforms: Android
- files: UI `SettingsActivity.kt:1202-1208`; persisted `Settings.kt:868-885`; `startDiscovery(...)` called unconditionally at `ClipboardService.kt:365`
- expected: Per its own doc, mDNS NSD register/advertise gates on the flag and hot-applies via observe.
- actual: mDNS advertise/browse + the standing SAS responder run regardless of the toggle.
- why it matters: A user on an untrusted network who turns LAN visibility off is still advertised/discoverable — privacy/security gap and misleading status.
- recommended fix: Gate native `start_discovery`/advertise on `settings.lanVisibility`; observe for hot-apply.
- test plan: toggle off → assert `start_discovery` not invoked / `stop_discovery` called.
- release blocker: no
- sources: AND-09 (audit-android)
- bd-title-uk: `lanVisibility` не керує mDNS — `startDiscovery` викликається безумовно
- bd-type: bug
- bd-priority: 1

### PCA-028 — Android logs and crash reports written to world-discoverable external storage
- severity: P1
- status: risky
- platforms: Android
- files: `android/app/.../AppLogger.kt:94-100`, `CrashHandler.kt:87`, `res/xml/file_paths.xml:12-14`
- expected: Logs in internal `MODE_PRIVATE` storage (`context.filesDir`), never cross-app readable.
- actual: Logs/crashes go to `getExternalFilesDir(null)/logs/` (`/sdcard/Android/data/com.copypaste.android/files/logs/`), readable on API<30 via `READ_EXTERNAL_STORAGE` and reachable via adb/MTP/file managers on 30+. No redaction layer; logs contain device ids, truncated fingerprints, relay/Supabase URLs, sizes, item ids, and full stack traces. Crash files never rotated/capped.
- why it matters: Cross-app metadata exposure on older devices; a crash message embedding content/a URL-with-token persists verbatim.
- recommended fix: Move logs/crashes to internal `filesDir`; export via FileProvider; gate external-storage logging behind `BuildConfig.DEBUG`; cap crash-file count; scrub secret-shaped substrings.
- test plan: assert files written under `filesDir`; on API 29 confirm another app cannot read them; log a mock AWS key and assert it is not present.
- release blocker: no
- sources: AND-10 (audit-android), SENS-04 (audit-sensitive)
- bd-title-uk: Логи та crash-звіти пишуться у зовнішнє сховище замість внутрішнього `MODE_PRIVATE`
- bd-type: bug
- bd-priority: 1

### PCA-029 — Android background clipboard capture unreliable on 10+ and self-contradictory
- severity: P1
- status: broken
- platforms: Android
- files: `ClipboardService.kt:58-66,643-652` vs `ClipboardFloatingActivity.kt:28-29,58-62`; `AndroidManifest.xml:48-56`; `LogcatCaptureService.kt:58-60`
- expected: A reliable background read path, or an honest "best-effort only" status. Android 10+ blocks background `getPrimaryClip()` for non-focused apps.
- actual: The FGS adds a 1×1 `TYPE_APPLICATION_OVERLAY` with `FLAG_NOT_FOCUSABLE` and claims it "lifts the restriction"; `ClipboardFloatingActivity`'s own doc says the opposite (non-focusable overlay never gets focus, so the restriction is NOT lifted). The only working background path is `LogcatCaptureService` → focusable `ClipboardFloatingActivity`, requiring signature-level `READ_LOGS` (adb-only) + `SYSTEM_ALERT_WINDOW`. Stock-Android background capture is effectively absent for ordinary users, with two implementations making opposite claims.
- why it matters: The headline feature silently does nothing in the background on stock Android 10+ without the adb ritual.
- recommended fix: Reconcile the contradictory docs with measured reality; make the Settings/onboarding status honestly reflect best-effort and emphasize the adb path; consider a WorkManager poll fallback.
- test plan: stock Android 12/13 without `READ_LOGS`: copy in another app while backgrounded; verify whether capture fires and that the status string matches reality.
- release blocker: no
- sources: AND-03 (audit-android), G-android F-3, A-architecture context
- bd-title-uk: Фонове захоплення на Android 10+ ненадійне; overlay у `ClipboardService` суперечить `ClipboardFloatingActivity`
- bd-type: bug
- bd-priority: 1

### PCA-030 — Telemetry `PiiScrubber` implemented but never wired to any caller (dead code)
- severity: P2
- status: stub
- platforms: all
- files: `crates/copypaste-telemetry/src/lib.rs` (`init()` always returns `NoopReporter`), `crates/copypaste-telemetry/src/scrubber.rs`; no `*/Cargo.toml` depends on the crate
- expected: If telemetry ships, the daemon initialises a reporter behind opt-in consent and routes errors through the scrubber; docs (`telemetry-policy.md`) imply scrubbing is active.
- actual: `init()` ignores the passed `ReportConsent` and always returns `NoopReporter`; no daemon/CLI/UI/android crate depends on `copypaste-telemetry`. Scrubber unit tests pass but the scrubber never runs in production. Current behavior is safe (Noop discards everything) but the privacy policy is misleading and a future Sentry wiring would flow PII unscrubbed unless addressed.
- why it matters: Misleading safety guarantee; abandoned-mid-integration dead-weight; opting into error reporting does nothing.
- recommended fix: Decide product intent — either wire `init(consent)` at daemon startup (default `Disabled`) and route the error path through the scrubber, or remove the crate + ARCHITECTURE/policy references until intentionally added.
- test plan: with consent=Denied assert no events emitted and scrubber strips known PII; integration test that startup constructs the reporter (if integrating); `cargo build --workspace` after removal (if dropping).
- release blocker: no
- sources: SENS-03 (audit-sensitive), LOG-01 (audit-storage-logs-release), CMP-003 (P1-completeness), B-crypto context (F-03)
- bd-title-uk: `copypaste-telemetry` (`PiiScrubber`/`init`) не підключений до жодного caller'а — мертвий код
- bd-type: bug
- bd-priority: 2

### PCA-031 — `has_sensitive_items` swallows DB errors and returns `false` → sensitive data persists past TTL
- severity: P2
- status: risky
- platforms: macOS, Android, Linux
- files: `crates/copypaste-core/src/storage/items.rs:952-960`
- expected: A DB error propagates; the TTL cleanup is skipped with a warning on a degraded DB.
- actual: Returns `false` on any DB error, so the sensitive-TTL pre-check reports "nothing to clean" and sensitive data silently persists past its TTL.
- why it matters: Silent failure of a privacy guarantee on a degraded DB.
- recommended fix: Return `Result<bool, ItemsError>`; caller logs and skips cleanup.
- test plan: force a DB error in the pre-check → assert the error is surfaced/logged, not collapsed to `false`.
- release blocker: no
- sources: R-03 (P2-reliability-ux)
- bd-title-uk: `has_sensitive_items` ковтає помилки БД і повертає `false` — чутливі дані лишаються після TTL
- bd-type: bug
- bd-priority: 2

### PCA-032 — `revoked_devices` table created outside versioned migrations
- severity: P2
- status: risky
- platforms: macOS, Android, Linux
- files: `crates/copypaste-core/src/storage/devices.rs:39-50`
- expected: The table is created via a migration step with a `SCHEMA_VERSION` bump.
- actual: Created as a startup side-effect via `CREATE TABLE IF NOT EXISTS`, not part of `apply_migrations()`. If startup order changes or the DB is opened off the normal path, `revoke_device()` fails with "no such table".
- why it matters: Schema integrity hazard; a revoke can panic on an unexpected open path.
- recommended fix: Add as a proper migration (v12).
- test plan: open a DB without the startup side-effect, call `revoke_device`, assert it succeeds (table present).
- release blocker: no
- sources: R-04 (P2-reliability-ux)
- bd-title-uk: Таблиця `revoked_devices` створюється поза версіонованими міграціями
- bd-type: bug
- bd-priority: 2

### PCA-033 — `reqwest::Client` fallback path has no timeout (cloud/IPC) → loops block forever
- severity: P2
- status: risky
- platforms: macOS, Linux
- files: `crates/copypaste-supabase/src/cloud.rs:819-822,1813-1816` (`.unwrap_or_else(|_| reqwest::Client::new())`), `crates/copypaste-daemon/src/ipc.rs:9380` (`reqwest::Client::new()` test-connection)
- expected: Every HTTP client has a bounded timeout.
- actual: On TLS/builder failure the fallback client has no timeout; push/realtime loops block indefinitely on a network stall. The IPC test-connection client likewise has no timeout and can block the IPC worker until the OS TCP timeout (minutes).
- why it matters: A single network stall hangs a sync loop or an IPC worker indefinitely.
- recommended fix: Propagate the builder error rather than using a no-timeout fallback; set an explicit timeout on the test-connection client.
- test plan: simulate a stalled endpoint → assert the call times out rather than hanging.
- release blocker: no
- sources: R-05 + R-13 (P2-reliability-ux)
- bd-title-uk: Фолбек `reqwest::Client` без таймауту (cloud/IPC) — цикли блокуються назавжди
- bd-type: bug
- bd-priority: 2

### PCA-034 — Relay push loop drops items on transient failure (no retry queue / no backoff)
- severity: P2
- status: risky
- platforms: macOS
- files: `crates/copypaste-daemon/src/relay.rs` (`push_loop` ~`:917`)
- expected: Failed pushes are queued for retry with backoff (the cloud path uses a `retry_queue: VecDeque`); transient errors trigger exponential backoff.
- actual: On any non-401 failure the item is logged and dropped from the broadcast channel — never re-queued — and the loop spins without backoff. Only items arriving after recovery get uploaded; the receive loop does use `BackoffScheduler`, the push loop does not.
- why it matters: Items captured during a transient relay outage are permanently lost to that transport; spin-retry burns CPU.
- recommended fix: Add a retry queue + `BackoffScheduler` on non-401 transient errors (keep the 401 re-register-once path).
- test plan: mock relay returning 503; assert the item is retried (not dropped) and the loop backs off.
- release blocker: no
- sources: F-03 (audit-sync), R-06 (P2-reliability-ux)
- bd-title-uk: relay push_loop губить items при тимчасовому збої — немає retry-черги та backoff
- bd-type: bug
- bd-priority: 2

### PCA-035 — Relay/P2P task JoinHandles dropped → panics silently kill sync subsystems
- severity: P2
- status: risky
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/relay.rs:1497,1508`; `crates/copypaste-p2p/src/p2p.rs:827,840,1313,1326`
- expected: Task panics are surfaced to a supervisor; the subsystem restarts or the user is notified.
- actual: `tokio::spawn(...)` JoinHandles are immediately dropped; a panic in any push/receive/connection task kills that subsystem invisibly and sync silently stops.
- why it matters: Background sync can die with no signal.
- recommended fix: Store JoinHandles; wrap spawns so panics are logged and the subsystem is restarted/notified.
- test plan: inject a panic in a spawned task → assert it is logged and the subsystem recovers/notifies.
- release blocker: no
- sources: R-07 (P2-reliability-ux)
- bd-title-uk: JoinHandle relay/P2P задач відкидаються — паніки мовчки вбивають синхронізацію
- bd-type: bug
- bd-priority: 2

### PCA-036 — Poisoned sync key-cache Mutex recovered silently (corrupt-key risk)
- severity: P2
- status: risky
- platforms: macOS, Android, Linux
- files: `crates/copypaste-sync/src/sync_orch.rs:198,217,237,254` (`unwrap_or_else(|p| p.into_inner())`)
- expected: A poisoned key cache is treated as fatal; sync halts and the user is notified.
- actual: The poisoned Mutex is recovered silently, reading potentially partial key state; subsequent sync ops may use corrupt keys and errors are dropped.
- why it matters: Using a corrupt key can fail decryption everywhere or produce undefined behavior, silently.
- recommended fix: Treat a poisoned key-cache Mutex as fatal; restart the sync subsystem.
- test plan: poison the Mutex (panic while holding) → assert sync halts/restarts rather than continuing with partial state.
- release blocker: no
- sources: R-08 (P2-reliability-ux)
- bd-title-uk: Отруєний Mutex кешу ключів синхронізації відновлюється мовчки — ризик зіпсованого ключа
- bd-type: bug
- bd-priority: 2

### PCA-037 — P2P `push_catchup` unbounded `send().await` → connector deadlock
- severity: P2
- status: risky
- platforms: macOS, Linux
- files: `crates/copypaste-p2p/src/p2p.rs:1879-1893`
- expected: Catch-up has a per-item timeout; a stalled receiver is detected and the connection dropped.
- actual: `sink.send().await` loops with no timeout; if the receiver stalls/panics before draining, the connector task deadlocks indefinitely, blocking all new P2P connections.
- why it matters: One stalled peer can wedge the entire P2P accept path.
- recommended fix: Wrap in `tokio::time::timeout`; cancel catch-up and drop the peer on timeout.
- test plan: stall the receiver during catch-up → assert the connector times out and proceeds.
- release blocker: no
- sources: R-09 (P2-reliability-ux)
- bd-title-uk: P2P `push_catchup` необмежений `send().await` — дедлок конектора
- bd-type: bug
- bd-priority: 2

### PCA-038 — `migration_v4` unbounded `i64::MAX` fetch → OOM on large databases
- severity: P2
- status: risky
- platforms: macOS, Android, Linux
- files: `crates/copypaste-core/src/storage/migration_v4.rs:695` (`fetch_kv2_blob_batch(db, i64::MAX as usize)`)
- expected: Bounded batch fetch, matching the normal sweep path.
- actual: Loads all matching rows in one shot; on large image-heavy DBs this can exhaust RAM during migration.
- why it matters: Upgrade-time OOM/crash on big databases.
- recommended fix: Apply the same batching loop used by the regular sweep.
- test plan: stage a large blob set, run the v4 migration, assert bounded memory.
- release blocker: no
- sources: R-10 (P2-reliability-ux)
- bd-title-uk: `migration_v4` необмежений `i64::MAX` fetch — ризик OOM на великих БД
- bd-type: bug
- bd-priority: 2

### PCA-039 — Relay watermark not persisted across daemon restarts (cursor gap risk)
- severity: P2
- status: open
- platforms: macOS
- files: `crates/copypaste-daemon/src/relay.rs` (`Watermark` struct + `receive_loop`, `:292`)
- expected: Watermark `(wall_time, id)` is persisted so a restart resumes from the last-seen relay item.
- actual: `Watermark` is in-memory only (`#[derive(Default)]`, reset to `(0,0)` on every start); every restart re-fetches all relay items from the beginning. Self-echo LWW dedup makes this safe but wasteful, and the `(wall_time, id)` cursor reset can interact with same-`wall_time` buckets to skip items if the relay id is not strictly monotonic.
- why it matters: Unnecessary full re-download each restart; potential gap if the relay row id is non-monotonic.
- recommended fix: Persist the watermark to DB/JSON in the app-support dir; read on startup; update atomically after each ingest page. Document the relay id-generation contract (strictly increasing).
- test plan: kill+restart the daemon; assert relay poll resumes from last-seen `(wall_time, id)`, not zero.
- release blocker: no
- sources: F-01 + F-17 (audit-sync), R-15 (P2-reliability-ux)
- bd-title-uk: relay watermark не зберігається між перезапусками — повторне завантаження + ризик пропуску
- bd-type: bug
- bd-priority: 2

### PCA-040 — Android relay ingest: image/file items bypass LWW (duplicate/stale on re-poll)
- severity: P2
- status: open
- platforms: Android
- files: `android/app/.../SyncManager.kt:501-566` (`ingestRelaySseItem`)
- expected: Image/file items from relay SSE use the same 3-tier LWW path as text (dedupe on `item_id`, compare lamport, replace only when remote wins).
- actual: Text calls `storeItemWithLww`; image/file call plain `storeItem(overrideId = …)`. The same image received twice (re-poll, self-echo, concurrent relay+Supabase) may be stored twice or the newer version may not replace the older.
- why it matters: Duplicate or stale image/file rows on Android.
- recommended fix: Add `storeImageWithLww`/`storeFileWithLww` that apply the 3-tier LWW before storing.
- test plan: push the same image twice with increasing lamport; assert one row with the higher lamport.
- release blocker: no
- sources: F-06 (audit-sync)
- bd-title-uk: Android relay ingest image/file обходить LWW — дублікати/застарілі при re-poll
- bd-type: bug
- bd-priority: 2

### PCA-041 — Android Lamport clock migration: old wall-millis values bias LWW against macOS
- severity: P2
- status: documented, no fix plan
- platforms: Android
- files: `android/app/.../LamportClock.kt:24-36`
- expected: Upgrading from old Android builds does not permanently bias LWW toward Android items.
- actual: Old builds stored wall-clock ms (~1.7e12) in `lamport_ts`; after upgrade `observe()` advances the local clock to ~1.7e12+1, so all subsequent Android items carry values far larger than the macOS daemon's logical Lamport values — Android always wins LWW over macOS until the macOS clock catches up. The comment calls this an "acceptable transitional artefact" with no migration path.
- why it matters: Cross-platform LWW convergence is biased for upgraded devices, so macOS edits can be silently overridden.
- recommended fix: On first sync from an upgraded device, reset old rows' `lamport_ts` to a sane value (schema migration) or apply a Lamport-offset correction; short term, trigger a full re-sync so macOS observes and advances past the large values.
- test plan: simulate upgrade; assert the macOS clock advances past the biased Android values within one sync cycle.
- release blocker: no
- sources: F-07 (audit-sync)
- bd-title-uk: Android Lamport clock після оновлення несе wall-millis — bias LWW проти macOS
- bd-type: bug
- bd-priority: 2

### PCA-042 — `sync_orch` auto-apply SQL `OR 1=1` may nullify the device filter
- severity: P2
- status: needs-verification
- platforms: macOS
- files: `crates/copypaste-daemon/src/sync_orch.rs` (`merge_incoming_with_crypto`, `local_latest_wt` query)
- expected: The auto-apply query selects the latest local item for the local device only, to decide whether a synced item should auto-paste.
- actual: The observed query pattern `WHERE origin_device_id = '' OR 1=1` makes the predicate trivially true, selecting all rows rather than local-only, potentially suppressing auto-apply when a remote item is newer than the latest local item. (Observed in lines 1-1222 of a 3206-line file; needs full verification of how the result is used.)
- why it matters: Auto-apply (auto-paste of synced clip) may misfire/suppress based on the wrong "freshest item" comparison.
- recommended fix: Remove `OR 1=1` or scope the filter to `origin_device_id = this_device_id`.
- test plan: seed local + remote items; assert auto-apply fires/suppresses per the corrected local-only comparison.
- release blocker: no
- sources: F-10 (audit-sync)
- bd-title-uk: sync_orch auto-apply SQL `OR 1=1` обнуляє фільтр пристрою — перевірити
- bd-type: bug
- bd-priority: 2

### PCA-043 — Cloud passphrase change does not re-encrypt already-uploaded items
- severity: P2
- status: open
- platforms: macOS, Android
- files: daemon `cloud.rs` passphrase derivation; Android `SyncManager.derivedSyncKey()`
- expected: Changing the cloud sync passphrase re-encrypts previously-uploaded items under the new key so new-passphrase peers can read them.
- actual: No re-encryption pass found. New pushes use the new key, but items uploaded under the old key remain on Supabase under the old key; a new device with only the new passphrase cannot decrypt them.
- why it matters: Passphrase rotation silently strands old history on cloud-only peers.
- recommended fix: On passphrase change, run a background re-upload pass (fetch local, re-encrypt under new key, upsert); show progress and block sync until complete.
- test plan: change passphrase on A; assert B (new passphrase only) can decrypt all items including pre-change ones.
- release blocker: no
- sources: F-18 (audit-sync)
- bd-title-uk: зміна passphrase не перешифровує вже завантажені items на Supabase
- bd-type: feature
- bd-priority: 2

### PCA-044 — Android sensitive-item sync filtering unverified (no `isSensitive` in OutboundMutationQueue / push)
- severity: P2
- status: unverified
- platforms: Android
- files: `android/app/.../OutboundMutationQueue.kt`, `SyncManager.kt`, `FgsSyncLoop.kt`, `ClipboardService.kt`, `ClipboardRepository.kt`
- expected: Android skips sensitive items when pushing to relay/Supabase/P2P (matching macOS `relay.rs:814`, `sync_orch.rs:345`, `cloud.rs` guards), and never re-uploads sensitive items ingested from peers.
- actual: `MutationRecord` carries no `isSensitive` field; no explicit `isSensitive` guard was found in the Kotlin push layer. Capture stores sensitive items (`is_sensitive=true`); Supabase poll ingest stores incoming items without re-checking sensitivity. Whether the Kotlin push paths gate on `is_sensitive` before upload is unverified (the Rust FFI `storeClipboardItem` may handle it, but no explicit Kotlin guard exists).
- why it matters: If unfiltered, a sensitive item captured on Android could be relayed to all paired devices, breaking the "sensitive items are never uploaded" guarantee. Could be P1 if confirmed unfiltered.
- recommended fix: Audit `SyncManager.pushToRelay`/`pushToSupabase` and `FgsSyncLoop` upload paths; add `if (item.isSensitive) continue`; add a test asserting zero outbound payload for a sensitive item.
- test plan: mock relay/Supabase; insert a credential item; run sync; assert zero outbound payloads.
- release blocker: no (investigate — possibly P1)
- sources: SENS-05 (audit-sensitive), F-16 (audit-sync)
- bd-title-uk: Android: перевірити фільтрацію чутливих елементів у SyncManager/FgsSyncLoop перед вивантаженням
- bd-type: bug
- bd-priority: 2

### PCA-045 — `delete_all` tombstones sequentially (N serial spawn_blocking) — slow on large history
- severity: P2
- status: open
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/ipc.rs:3705-3751`
- expected: Clearing 1000+ items completes promptly.
- actual: `delete_all` fetches all non-pinned ids, then calls `soft_delete_and_broadcast(&id).await` per id in a sequential loop — ~1000 lock acquisitions + 1000 broadcasts, ~1s wall time for a large history, with no progress indicator and risk of hitting the CLI's 5s `IO_TIMEOUT` on a slow/locked device.
- why it matters: The clear-history IPC blocks the caller O(N) seconds and can time out.
- recommended fix: Bulk tombstone SQL (`UPDATE … WHERE pinned=0 AND deleted=0`) in one `spawn_blocking`, then one batched broadcast.
- test plan: insert 500 items, `delete_all`, assert all gone in <500ms.
- release blocker: no
- sources: F-14 (D-daemon-ipc / audit-ipc-cli-relay)
- bd-title-uk: `delete_all` робить N послідовних spawn_blocking — повільно при великій історії
- bd-type: bug
- bd-priority: 2

### PCA-046 — CLI import: 64 MiB file cap exceeds the 16 MiB IPC request cap → cryptic failure
- severity: P2
- status: open
- platforms: macOS, Linux, CLI
- files: `crates/copypaste-cli/src/commands/import.rs` (`MAX_IMPORT_FILE_BYTES = 64 MiB`), `crates/copypaste-daemon/src/ipc.rs` (`MAX_REQUEST_BYTES = 16 MiB`, `MAX_IMPORT_ITEM_BYTES = 4 MiB`)
- expected: Importing a 64 MiB export either succeeds or gives a clear error.
- actual: The CLI sends the whole JSON array in one `METHOD_IMPORT` request; the daemon rejects any line >16 MiB with "request too large" and closes the connection, so a legitimate multi-thousand-item export fails with a cryptic connection-closed error.
- why it matters: Inconsistent caps; users see a socket-closed error rather than a helpful message.
- recommended fix: Reduce `MAX_IMPORT_FILE_BYTES` to ~12 MiB with a clear pre-flight error, or implement chunked N-per-request import.
- test plan: generate an import file >16 MiB; assert the CLI prints a clear error before connecting.
- release blocker: no
- sources: F-04 (audit-ipc-cli-relay)
- bd-title-uk: import файл 64 MiB перевищує `MAX_REQUEST_BYTES=16 MiB` — з'єднання закривається без пояснення
- bd-type: bug
- bd-priority: 2

### PCA-047 — No per-request IPC read timeout; a stalled client holds a slot + DB Mutex indefinitely
- severity: P2
- status: open
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/ipc.rs:3167-3231` (`read_until` at `:3172`, no timeout)
- expected: A client that dribbles bytes without a newline is timed out and disconnected.
- actual: `read_until` has no timeout; a client that connects and sends partial JSON holds 1 of 64 connection slots forever; 64 such clients stop the accept loop, and a handler holds the DB Mutex for its duration — a slow/hostile IPC client can starve clipboard capture. `MAX_REQUEST_BYTES` only fires after a full read.
- why it matters: Trivial local DoS of the IPC server / capture path.
- recommended fix: Wrap `read_until` in `tokio::time::timeout(IDLE, …)` (~30s); close on timeout.
- test plan: `socat` connect, send partial JSON without newline, repeat 64× → assert connections are timed out.
- release blocker: no
- sources: D-daemon-ipc D3.3
- bd-title-uk: Немає таймауту читання IPC — завислий клієнт тримає слот і DB Mutex назавжди
- bd-type: bug
- bd-priority: 2

### PCA-048 — `lsappinfo front` forked every poll tick and blocks signal handling
- severity: P2
- status: open
- platforms: macOS
- files: `crates/copypaste-daemon/src/daemon.rs:1722-1749`
- expected: `lsappinfo` is invoked only when needed (exclusion list non-empty or sensitive-app tracking on); a slow `launchservicesd` does not stall the tick loop.
- actual: `lsappinfo front` is `spawn_blocking`-forked unconditionally every tick (~2/s) even with an empty exclusion list, and is awaited inline in the `select!` tick arm — a multi-second `launchservicesd` stall makes the loop unresponsive to ctrl_c/SIGTERM, and the default `MissedTickBehavior::Burst` then surges deferred ticks.
- why it matters: Sustained idle overhead + signal-handling latency + tick burst after a stall.
- recommended fix: Gate the spawn on `!excluded.is_empty() || sensitive_app_tracking`; set `MissedTickBehavior::Skip`; run `lsappinfo` off the tick path or check the quit flag around the await.
- test plan: empty exclusion list → assert no `lsappinfo` fork; simulate a slow `lsappinfo` → assert SIGTERM still handled.
- release blocker: no
- sources: D-daemon-ipc D1.1 + D4.2
- bd-title-uk: `lsappinfo front` форкається щотіка і блокує обробку сигналів
- bd-type: bug
- bd-priority: 2

### PCA-049 — Self-write sentinel pre-stamp off-by-one under a 3rd-party write race
- severity: P2
- status: open
- platforms: macOS
- files: `crates/copypaste-daemon/src/ipc.rs:8405-8411` (pre-stamp `pre+2`), `:8531` (post-stamp)
- expected: The daemon's own pasteboard write is reliably recognized as self-write and not re-captured.
- actual: `write_to_pasteboard` pre-stamps the sentinel as `pre+2` assuming exactly two changeCount increments; if a third app writes between the read and `clearContents`, the real post-count is `pre+3+`, the sentinel mispredicts, and the daemon's write is recorded as an external change (duplicate). A failed Cocoa write that doesn't return `false` can also wrongly suppress the next capture.
- why it matters: Duplicate history rows (or a missed capture) under concurrent clipboard writes.
- recommended fix: Perform read→clear→write→read inside one `autoreleasepool` with no async suspension.
- test plan: rapid `pbcopy` loop while pasting from the daemon → assert no duplicate rows.
- release blocker: no
- sources: D-daemon-ipc D1.2
- bd-title-uk: Sentinel самозапису off-by-one при гонці стороннього запису — дублікати
- bd-type: bug
- bd-priority: 2

### PCA-050 — File pre-check stat and read are separate spawn_blocking calls (TOCTOU)
- severity: P2
- status: open
- platforms: macOS
- files: `crates/copypaste-daemon/src/daemon.rs:1931-1947` (stat `:1933`, read `:1947`, gate `:1950`)
- expected: Size pre-check and read are atomic w.r.t. the file.
- actual: The size pre-check and the read are separate blocking calls; the file/symlink can be swapped between them, causing a large unexpected read (the post-read gate preserves data safety; the risk is a wasted large read). No 32-bit overflow guard on `meta.len() as usize`.
- why it matters: A swapped file between stat and read wastes a large read.
- recommended fix: Open once, `metadata()`/`seek`, check, then read — all in one closure.
- test plan: swap the file between stat and read in a test harness → assert the read is bounded by the re-checked size.
- release blocker: no
- sources: D-daemon-ipc D2.3
- bd-title-uk: Перевірка розміру файлу і читання — окремі spawn_blocking (TOCTOU)
- bd-type: task
- bd-priority: 2

### PCA-051 — Broadcast `Lagged` drops to sync subscribers are unmetered
- severity: P2
- status: open
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/daemon.rs:609` (`broadcast::channel(256)`); also `:1827,1866,1909`, `:1722` mpsc; `sync_incoming/outbound_tx` capacity 64 at `:816-817`
- expected: A slow sync subscriber that lags is detected/metered, and bounded channels do not cascade-block the P2P accept loop.
- actual: The capture loop does `let _ = new_item_tx.send(item)` and ignores `RecvError::Lagged`; a slow P2P/cloud subscriber silently misses items until DB catch-up on reconnect (items not lost from storage) with no counter. Separately, a slow peer can fill the bounded `sync_incoming/outbound` mpsc(64) → `sync_orch` send blocks → P2P accept-loop send blocks → no new connections.
- why it matters: Silent lag with no observability; a slow peer can stall new connections.
- recommended fix: Add a `Lagged(n)` metric per subscriber; raise the mpsc capacity to 256 or use `try_send` with explicit drop + send timeout.
- test plan: force a lagged subscriber → assert a metric increments; fill the mpsc → assert accepts are not permanently blocked.
- release blocker: no
- sources: D-daemon-ipc D4.1 + D4.3, R-12 (P2-reliability-ux)
- bd-title-uk: Втрати `Lagged` для sync-підписників не метруються; обмежені канали можуть блокувати P2P accept
- bd-type: bug
- bd-priority: 2

### PCA-052 — Socket not cleaned up on SIGKILL/OOM/panic; no pid/lock file
- severity: P2
- status: open
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/daemon.rs:1479` (cleanup only on normal exit); `ipc.rs:8887` (`bind_with_stale_cleanup`)
- expected: A crashed daemon does not leave a stale socket that delays the next start; startup is race-safe.
- actual: `remove_file(socket)` runs only after the main loop breaks; SIGKILL/OOM/panic leaves a stale socket. `bind_with_stale_cleanup` self-heals on next start, but probe→remove→bind is not atomic (two concurrent starts could both pass `is_socket_live`; the second bind then fails `EADDRINUSE`). There is no pid/lock file at all — liveness is checked only by socket connect.
- why it matters: One-startup delay + warning after a crash; theoretical concurrent-start race.
- recommended fix: Add a flock lock file alongside the socket, or rename-before-remove for atomicity.
- test plan: `kill -9` the daemon → assert next start self-heals; concurrent starts → assert exactly one binds.
- release blocker: no
- sources: D-daemon-ipc D3.1
- bd-title-uk: Сокет не очищується при SIGKILL/OOM/panic; немає pid/lock-файлу
- bd-type: task
- bd-priority: 2

### PCA-053 — `AppLogger` Android: no redaction layer on external-storage logs
- severity: P2
- status: risky
- platforms: Android
- files: `android/app/.../AppLogger.kt:94-100`
- expected: Log output never includes clipboard content; even on external storage no sensitive substrings are written.
- actual: `AppLogger.write` has no `scrub()` helper; if any call site passes clipboard content to `d/i/w/e`, it is written in plaintext to `getExternalFilesDir/logs/` (adb-pullable without root). Spot-check shows no direct content logging, but `CrashHandler` writes full stack traces that could embed content via exception messages. (Closely related to PCA-028, which covers the storage-location fix; this entry is the missing redaction layer.)
- why it matters: A future content-logging call site, or a content-bearing exception message, would persist in plaintext in an adb-readable log.
- recommended fix: Add a regex `scrub(msg)` (like the daemon `PiiScrubber`) applied in `AppLogger.write`; combine with the internal-storage move from PCA-028.
- test plan: log a string containing a mock AWS key; assert the file does not contain it.
- release blocker: no
- sources: SENS-04 (audit-sensitive), AND-10 (audit-android)
- bd-title-uk: `AppLogger` Android не редагує чутливий контент перед записом у лог
- bd-type: bug
- bd-priority: 2

### PCA-054 — UDL zeroization contract not honored on the Kotlin side
- severity: P2
- status: risky
- platforms: Android
- files: contract `crates/copypaste-android/uniffi/copypaste_android.udl:104,190-209,331-333,362-364,411-412,631`; violations `Settings.kt:246-273,963-979,1112-1140,1262-1263,1525-1536`, `DeviceKeyStore.kt:35-41`
- expected: The UDL mandates the Kotlin caller zero secret `ByteArray`s after use (sync key, PAKE session keys, P2P key_der, derived_sync_key).
- actual: No zeroization anywhere in `Settings.kt`/`DeviceKeyStore.kt`; the master key, cloud sync key, and per-peer session keys linger on the JVM heap until GC.
- why it matters: Defense-in-depth violation on the most sensitive bytes — widens the heap-dump/memory-scrape window. (String-typed passphrase/Supabase password can't be zeroed — inherent limitation.)
- recommended fix: `java.util.Arrays.fill(raw, 0)` after `wrapKey`/persist and in `finally` blocks for `sessionKeyFor`/`cloudSyncKeyDirect`/`pairedPeerSessionKey` and the pairing-confirm/listener paths.
- test plan: code review; a test asserting a helper zeros its input array after wrapping.
- release blocker: no
- sources: AND-11 (audit-android)
- bd-title-uk: Контракт UDL щодо обнулення секретних `ByteArray` не виконується у `Settings`/`DeviceKeyStore`
- bd-type: bug
- bd-priority: 2

### PCA-055 — Android auto-apply silently overwrites the user's current clipboard
- severity: P2
- status: needs major improvement
- platforms: Android
- files: `android/app/.../ClipboardService.kt:710-715`, `FgsSyncLoop.kt:571-573,810-812`
- expected: Auto-apply of incoming clips is opt-in or non-destructive (macOS has an `auto_apply_synced_clip` knob).
- actual: On every catch-up drain / P2P batch the newest synced text clip is force-written via `setPrimaryClip`, overwriting whatever the user currently has copied; re-capture is guarded only by a single-shot `expectClip` hash (a missed guard window risks a re-capture/re-push loop). No user-facing toggle.
- why it matters: Surprising clobbering of the user's active clipboard with no control.
- recommended fix: Add an opt-in auto-apply setting (parity with macOS); only apply when the clipboard hasn't changed since last user interaction.
- test plan: sync arrives while user has unrelated text copied → assert clipboard not overwritten when auto-apply is off.
- release blocker: no
- sources: AND-13 (audit-android)
- bd-title-uk: Авто-застосування синхронізованого кліпу мовчки перезаписує поточний буфер користувача
- bd-type: feature
- bd-priority: 2

### PCA-056 — Android sync failures are invisible to the user (silent Log.w only; 401 collapsed to empty)
- severity: P2
- status: needs major improvement
- platforms: Android
- files: `android/app/.../FgsSyncLoop.kt:344,832`, `SyncManager.kt:1104-1146`, `SupabaseClient.kt:262,296,401,552`, `SupabasePollWorker.kt:135`
- expected: Persistent sync failures (bad passphrase/credentials, relay down, 401) are surfaced.
- actual: Every failure path is `Log.w`/`Log.e` only; the only user-visible sync error is native-crypto-unavailable. `pollRaw`/`poll` collapse HTTP errors (incl. 401) into `emptyList()`, so a poll-only device with a revoked JWT stays silently stalled.
- why it matters: A user with wrong credentials or a down relay gets no feedback; sync silently never happens.
- recommended fix: Surface persistent auth/credential/transport failures via notification or a Settings sync-status banner; distinguish "no rows" from "auth failure" in the poll layer.
- test plan: inject a 401 → assert a user-visible error state, not a swallowed empty list.
- release blocker: no
- sources: AND-14 (audit-android)
- bd-title-uk: Помилки синхронізації не показуються користувачу — лише `Log.w`; 401 згортається у порожній список
- bd-type: task
- bd-priority: 2

### PCA-057 — Android outbound mutation queue drain is not periodic (pin/delete can stall)
- severity: P2
- status: partial
- platforms: Android
- files: `android/app/.../OutboundMutationQueue.kt:57`; drain only at `ClipboardService.kt:1671,278` (UI mutation + startup); the 30s `FgsSyncLoop` tick only peeks the queue
- expected: Queued pin/reorder/delete mutations drain reliably even without further UI activity.
- actual: The queue persists across restarts (idempotent via LWW) but drains only on a UI mutation or startup; a mutation enqueued while offline can stall until the next UI action or a restart.
- why it matters: Pin/reorder/delete may not propagate to other devices for an extended period.
- recommended fix: Add a periodic drain on the `FgsSyncLoop` tick and on connectivity-regained.
- test plan: enqueue offline, regain connectivity with no UI action → assert drain within one loop tick.
- release blocker: no
- sources: AND-15 (audit-android)
- bd-title-uk: Черга вихідних мутацій не дренажиться періодично — pin/delete можуть зависнути
- bd-type: bug
- bd-priority: 2

### PCA-058 — Android P2P is Doze/OEM-kill fragile (no WakeLock; lifecycle tied to FGS)
- severity: P2
- status: partial
- platforms: Android
- files: `android/app/.../FgsSyncLoop.kt:65,151`, `ClipboardService.kt:400,596`
- expected: P2P listener/dialer survive Doze and aggressive OEM battery management.
- actual: No WakeLock anywhere; the inbound listener, 30s dial, and poll live and die with the FGS. Under Doze/OEM kills the FGS is suspended/killed and P2P silently stops; only swipe-away reschedules via WorkManager. The fixed 30s dial also wakes/drains when peers are offline.
- why it matters: Background P2P sync silently stops on many real-world devices.
- recommended fix: Doze-aware strategy (high-priority FCM nudge or a brief WakeLock around dial), surface battery-optimization state, adaptive dial backoff when all peers offline.
- test plan: `adb shell dumpsys deviceidle force-idle` → verify whether P2P resumes and how the user is informed.
- release blocker: no
- sources: AND-16 (audit-android)
- bd-title-uk: P2P крихкий до Doze/OEM-kill — немає WakeLock, життєвий цикл прив'язаний до FGS
- bd-type: task
- bd-priority: 2

### PCA-059 — Android P2P outbound has up to 30s latency vs daemon's near-immediate push
- severity: P2
- status: partial
- platforms: Android
- files: `android/app/.../ClipboardService.kt:1347` (inline push to Supabase+relay only), `FgsSyncLoop.kt:151` (P2P deferred to dial loop)
- expected: A fresh copy reaches a LAN-only peer promptly.
- actual: Capture pushes inline only to cloud/relay; P2P outbound waits for the next 30s dial, so a LAN-only macOS peer can lag up to 30s.
- why it matters: Noticeably worse cross-device latency on LAN-only setups.
- recommended fix: Trigger an immediate, debounced opportunistic P2P dial on capture in addition to the periodic loop.
- test plan: copy on Android with a LAN-only macOS peer → assert appears in < a few seconds.
- release blocker: no
- sources: AND-17 (audit-android)
- bd-title-uk: Вихідний P2P має затримку до 30с — кліп не доходить до LAN-пира одразу
- bd-type: task
- bd-priority: 2

### PCA-060 — No sync-key rotation / revoke-and-rotate wired on Android
- severity: P2
- status: partial
- platforms: Android
- files: FFI exists (`copypaste_android.udl:203-211`, `CopypasteBindings.kt` `revoke_device_and_rotate_key`/`rotate_sync_key`); no end-to-end UI flow; QR-provisioned `cloudSyncKeyDirect` write-once (`PairActivity.kt:657`). macOS: `ipc.rs:5158,5200`.
- expected: Revoking a peer can rotate the cloud sync key so the revoked device can no longer read cloud/relay items (parity with macOS `revoke_and_rotate`).
- actual: Android offers audit-only revoke + P2P denylist (fail-closed), but a revoked peer that still knows the passphrase keeps decrypting cloud/relay blobs; the rotation FFI is only exercised in stub-mode tests.
- why it matters: Incomplete revocation story for cloud/relay transports.
- recommended fix: Wire `revoke_device_and_rotate_key` into the Devices revoke flow with re-registration under the new key; verify old-key rejection against a live relay.
- test plan: live revoke+rotate; assert old key fails to fetch new items and new key works on both platforms.
- release blocker: no
- sources: AND-18 (audit-android), CMP-017 (P1-completeness)
- bd-title-uk: Ротація sync-ключа / revoke-and-rotate не підключена в Android UI
- bd-type: feature
- bd-priority: 2

### PCA-061 — Sensitive items remain findable via search (masked only); FTS policy undocumented
- severity: P2
- status: risky
- platforms: macOS, Android, daemon
- files: `crates/copypaste-daemon/src/ipc.rs:4632`, `crates/copypaste-ui/src/views/HistoryView.tsx:1839`, `android/.../HistoryActivity.kt:641`
- expected: A documented, deliberate policy for whether sensitive content is searchable, applied consistently.
- actual: `history_page` redacts the sensitive *preview* (no plaintext in the list), but the FTS index still holds the text, so a sensitive item can be surfaced by searching its content (shown blurred). Not a list-payload leak, but the item is discoverable by typing the secret; the behavior is undocumented and inconsistent. (The IPC-layer plaintext-leak facet is PCA-002; this entry is the policy/UX/parity decision.)
- why it matters: Privacy expectation gap — a "sensitive/private" item is still locatable by its secret content.
- recommended fix: Decide policy (exclude sensitive items from FTS, or document "searchable-but-masked") and apply consistently across macOS/Android + docs.
- test plan: assert the chosen policy (no FTS hit, or a masked hit) on both clients.
- release blocker: no
- sources: CLIP-15 (audit-clipboard), SENS-02 context, I-parity refuted-3
- bd-title-uk: Визначити та задокументувати політику пошуку чутливих елементів (FTS vs приховування)
- bd-type: task
- bd-priority: 2

### PCA-062 — Sensitive image/file TTL semantics diverge from text (`expires_at` vs `wall_time`)
- severity: P2
- status: open
- platforms: macOS, Android, Linux
- files: `crates/copypaste-daemon/src/daemon.rs:2091-2096,2224-2225,2313-2315`, `crates/copypaste-core/src/storage/items.rs:656-667,924-941,968-1001`
- expected: One coherent TTL mechanism for sensitive items; recopying a sensitive item extends its effective TTL consistently.
- actual: Text sensitive items get `expires_at = now + ttl`, but `delete_sensitive_expired` purges on `wall_time < threshold` (not `expires_at`); image/file sensitive items get no `expires_at` at all yet are still wiped by the `wall_time` predicate. After `bump_item_recency` (recopy) updates `wall_time` but not `expires_at`, the item can be deleted by `delete_expired` (general TTL) on the stale `expires_at` even though it was just recopied — an undocumented coupling between two TTL paths.
- why it matters: A just-recopied sensitive item can be deleted unexpectedly; `expires_at` on sensitive items is misleading.
- recommended fix: Align the two paths — extend `expires_at` on recency bump for sensitive items, or rely on a single TTL mechanism; at minimum document the intentional `wall_time` use.
- test plan: recopy a sensitive item near its original TTL → assert it is not deleted prematurely.
- release blocker: no
- sources: C-storage F-03 + F-05 (+ F-07)
- bd-title-uk: TTL для sensitive image/file розходиться з текстом (`expires_at` проти `wall_time`)
- bd-type: bug
- bd-priority: 2

### PCA-063 — FTS5 external-content index plaintext at-rest: design tradeoff undocumented
- severity: P2
- status: needs-improvement
- platforms: macOS, Android, Linux
- files: `crates/copypaste-core/src/storage/schema_v1.sql:7-8,19-20`, `items.rs:446-453,1484-1496`
- expected: The decision to store decrypted plaintext in `clipboard_fts.content_text` (protected only by SQLCipher at-rest, no secondary app-layer encryption) is explicitly documented in an ADR.
- actual: `clipboard_fts` stores decrypted plaintext for search; if the SQLCipher key is extracted, the FTS index reveals all indexed text verbatim alongside the ciphertext. This is a design choice, not a bug, but undocumented.
- why it matters: A second reader with the SQLCipher key gets ciphertext and plaintext side by side; the tradeoff should be a recorded decision.
- recommended fix: Document in ARCHITECTURE.md/ADR; if higher assurance is required, switch to an external-content table pointing at the encrypted `content` column with app-layer decryption before search.
- test plan: n/a (documentation); if changed, search-correctness regression test.
- release blocker: no
- sources: C-storage F-01
- bd-title-uk: FTS5 зберігає plaintext at-rest — задокументувати дизайн-компроміс в ADR
- bd-type: task
- bd-priority: 2

### PCA-064 — Sensitive-pattern keyword list misses common token forms
- severity: P2
- status: needs-improvement
- platforms: all
- files: `crates/copypaste-core/src/sensitive/patterns.rs:139-146`
- expected: Common credential forms (`client_secret`, `access_token`, `refresh_token`, `private_key`, `db_password`, bare `token`, DigitalOcean `dop_v1_…`) are detected.
- actual: `generic_password_kv` requires the key to be one of `password|passwd|secret|api_key|apikey|auth_token`; lowercase mixed forms without those exact prefixes (`db_password=`, `client_secret=`, `access_token=`) are not matched (uppercase `_SECRET`/`_KEY` forms are caught by `dotenv_secret`).
- why it matters: False negatives — real credentials are not flagged as sensitive.
- recommended fix: Expand the keyword list to include `client_secret|access_token|refresh_token|private_key|db_pass|db_password|token` and add a DigitalOcean `dop_v1_[A-Za-z0-9]{64}` pattern.
- test plan: assert `access_token=AbcDef123456` is detected.
- release blocker: no
- sources: C-storage F-09
- bd-title-uk: Список ключів `generic_password_kv` пропускає `client_secret`/`access_token`/`db_password` тощо
- bd-type: bug
- bd-priority: 2

### PCA-065 — Telemetry PII scrubber misses single-segment base64url tokens
- severity: P2
- status: needs-improvement
- platforms: all
- files: `crates/copypaste-telemetry/src/scrubber.rs:69-72`
- expected: High-entropy single-segment base64url secrets (43-char QR pairing token, base64 sync key) are redacted.
- actual: `RE_JWT` matches only three-segment tokens; a 43-char base64url string with no `.`/`:` passes through unredacted (`RE_HEX32`/`RE_UUID_HEX` don't cover it). The module comment acknowledges the miss but does not enumerate these shapes.
- why it matters: If a pairing/inbox token or raw sync key appears in an error string, it would be transmitted to a telemetry backend verbatim (if telemetry is ever wired — see PCA-030).
- recommended fix: Add a `[A-Za-z0-9_-]{40,}` pattern after `RE_JWT`; document uncovered shapes.
- test plan: assert a 43-char base64url token is redacted.
- release blocker: no
- sources: B-crypto F-03
- bd-title-uk: PII-скрабер не ловить односегментні base64url токени
- bd-type: bug
- bd-priority: 2

### PCA-066 — `derive_storage_key_v1` returns an unzeroized `[u8; 32]`
- severity: P2
- status: needs-improvement
- platforms: all
- files: `crates/copypaste-core/src/crypto/keys.rs:132-138` (cf. `derive_v2` at `:119`)
- expected: Returns `zeroize::Zeroizing<[u8; 32]>` like its v2 sibling.
- actual: Returns a plain `Copy` `[u8; 32]`; during a migration sweep/export the raw v1 storage key sits unguarded on stack/heap until the OS reclaims the page.
- why it matters: A memory dump during migration could expose the v1 key, which decrypts all pre-migration rows.
- recommended fix: Change the return type to `Zeroizing<[u8; 32]>` (callers already deref).
- test plan: compile-level check that the return type is `Zeroizing`.
- release blocker: no
- sources: B-crypto F-02
- bd-title-uk: `derive_storage_key_v1` повертає необнулюваний `[u8; 32]`
- bd-type: bug
- bd-priority: 2

### PCA-067 — `DeviceKeypair::ecdh` returns an unzeroized ECDH secret copy; stale doc reference
- severity: P2
- status: needs-improvement
- platforms: all
- files: `crates/copypaste-core/src/crypto/keys.rs:146-147,172-176,187-195`
- expected: ECDH callers use the zeroizing variant; doc comments reference only live accessors.
- actual: `ecdh` wraps in `Zeroizing` then returns `*buf` — a plain `Copy` array, leaking an unzeroized shared-secret copy each call (the comment acknowledges this). The doc at `:172` references a removed `secret_key_bytes` accessor, a doc hazard inviting reintroduction of the unsafe method.
- why it matters: Any caller using `ecdh` (vs `ecdh_zeroizing`) leaves the shared secret unguarded.
- recommended fix: `#[deprecated]` `ecdh` pointing to `ecdh_zeroizing`; fix the `secret_key_bytes` doc reference; migrate callers.
- test plan: n/a (attribute + doc); audit callers.
- release blocker: no
- sources: B-crypto F-04
- bd-title-uk: `DeviceKeypair::ecdh` повертає необнулену копію ECDH-секрету; застаріле посилання в доці
- bd-type: bug
- bd-priority: 2

### PCA-068 — Responder side shows SAS without the peer fingerprint
- severity: P2
- status: partial
- platforms: daemon, macOS, Android
- files: `crates/copypaste-daemon/src/pairing_sm.rs:59-65`, `ipc.rs:6262,2486-2500`
- expected: At SAS-confirm both endpoints display the peer's cert fingerprint (and name/IP) so the human can correlate the device.
- actual: `PeerSnapshot.fingerprint` is populated only on the initiator path (from mDNS device_id); on the responder path `pair_get_sas` returns no `peer_fingerprint`/`peer_device_name`/`peer_ip_addrs` (the TLS peer fp is known post-handshake but not surfaced).
- why it matters: A responder confirms the SAS with zero device-identity context; the displayed identity is asymmetric, weakening MitM detection.
- recommended fix: Thread the post-handshake `tls_peer_fp` into the responder's `enter_awaiting_sas` `PeerSnapshot` so `pair_get_sas` returns `peer_fingerprint` on both roles.
- test plan: drive the responder path; assert `pair_get_sas` body contains `peer_fingerprint`.
- release blocker: no
- sources: PAIR-01 (audit-pairing-devices)
- bd-title-uk: На стороні responder `pair_get_sas` не повертає `peer_fingerprint` — SAS без ідентичності
- bd-type: bug
- bd-priority: 2

### PCA-069 — Online/offline derivation diverges across platforms and can mislead
- severity: P2
- status: mostly complete
- platforms: daemon, macOS, Android
- files: `crates/copypaste-daemon/src/ipc.rs:5882-5996`, `android/.../DevicesActivity.kt:234-238,520-547`
- expected: "Online" means the same on both platforms and reflects real reachability.
- actual: macOS uses the live P2P sink table (authoritative) with a 60s `last_sync_at` fallback only when P2P is off; Android has no live-connection signal and derives online from mDNS IP-correlation + a 60s `lastSyncMs` window. So a peer that synced 50s ago then dropped shows online on Android (and on macOS only if P2P off), mDNS-advertising alone reads online on Android, and the two platforms can disagree about the same peer.
- why it matters: A user-facing reliability signal that can lie and disagree cross-platform.
- recommended fix: Document the semantics explicitly; give Android a live-connection signal (it has the FGS dialer); align both to "connected now" with a separate "last active" line; reconcile the 60s windows and the mDNS-implies-online rule.
- test plan: assert a peer with a closed sink but recent last_sync shows offline on macOS when P2P enabled; document the Android divergence.
- release blocker: no
- sources: PAIR-04 (audit-pairing-devices)
- bd-title-uk: Похідна online/offline розходиться між macOS (live sinks) та Android (mDNS+lastSync)
- bd-type: bug
- bd-priority: 2

### PCA-070 — Android image/file rows omit the source-app icon; reorder gesture diverges
- severity: P2
- status: needs-improvement
- platforms: Android, macOS
- files: `android/.../HistoryActivity.kt:3115-3165` (text-row icon), `:2892,:2994` (image/file rows lack it), `:988-998,2847` (reorder up/down); macOS `crates/copypaste-ui/src/views/HistoryView.tsx:600-609,2160` (drag-to-reorder)
- expected: Source-app provenance shown uniformly across row types (macOS shows it on all rows); equivalent reorder interaction.
- actual: Android renders the source-app icon+label chip only on text rows (image/file rows omit it). Reorder: macOS uses HTML5 drag with a drop indicator; Android uses an explicit reorder-mode with per-row up/down buttons — functionally equivalent but divergent UX.
- why it matters: Provenance hidden for image/file clips on Android; minor cross-platform UX inconsistency.
- recommended fix: Render the source-app chip on Android image/file rows; optionally add long-press drag-reorder on Android (or document the deliberate difference).
- test plan: composable test asserting the app-icon chip renders for an image item with a known source package.
- release blocker: no
- sources: CLIP-09 + CLIP-10 (audit-clipboard)
- bd-title-uk: Android image/file рядки без іконки джерела; жест перевпорядкування розходиться з macOS
- bd-type: task
- bd-priority: 2

### PCA-071 — Android image copy-back from PreviewOverlay uses a narrow SystemUI URI grant
- severity: P2
- status: risky
- platforms: Android
- files: `android/app/.../HistoryActivity.kt:1407,1427` (narrow grant) vs `:328` `grantUriToAll` and `:2277,2316` (broad grant on list-row path)
- expected: Image copy-back grants read permission to whatever app receives the paste, consistently across all copy paths (the AB-12 fix introduced `grantUriToAll`).
- actual: The list-row path was fixed to `grantUriToAll`, but the PreviewOverlay (pinned-mode) copy path still calls `grantUriPermission("com.android.systemui", uri, …)` only; on OEMs where the pasting app is not SystemUI, pasting a previewed image silently fails.
- why it matters: The exact AB-12 bug left behind on one path — partially-broken copy on a subset of devices.
- recommended fix: Replace the two narrow grants in the PreviewOverlay copy path with `grantUriToAll(ctx, uri)`.
- test plan: assert the preview copy path enumerates packages; manual paste test on a non-SystemUI OEM.
- release blocker: no
- sources: CLIP-05 (audit-clipboard)
- bd-title-uk: PreviewOverlay копіювання зображення має використовувати `grantUriToAll`, а не `com.android.systemui`
- bd-type: bug
- bd-priority: 2

### PCA-072 — `delete_all` (clear history) not reachable from the macOS history view
- severity: P2
- status: partial
- platforms: macOS
- files: `crates/copypaste-ui/src/lib/ipc.ts` (`deleteAll`), `HistoryView.tsx` (no caller), `SettingsView.tsx` (the only caller)
- expected: Clearing all history is discoverable where the user views history (Android exposes Clear-all/Clear-unpinned in the history overflow; CLI has `clear`). Note: I-parity refuted-1 confirms clear-all DOES exist in Settings → Storage (canonical), so this is a discoverability/placement gap, not a missing feature.
- actual: On macOS `delete_all` is reachable only via SettingsView; the history list has no "Clear all" affordance.
- why it matters: Discoverability/parity gap — the most natural place to clear history lacks the action on macOS.
- recommended fix: Add a confirmed "Clear all" (and optionally "Clear unpinned") entry to the macOS HistoryView toolbar/overflow.
- test plan: UI test asserting the action renders, prompts a confirm, and calls `deleteAll`.
- release blocker: no
- sources: CLIP-06 (audit-clipboard), I-parity refuted-1
- bd-title-uk: Додати дію "Clear all" у macOS HistoryView (парність з Android/CLI)
- bd-type: feature
- bd-priority: 2

### PCA-073 — macOS LogView: no offline/daemon-down state, no tests, raw error/path leakage
- severity: P2
- status: partial
- platforms: macOS
- files: `crates/copypaste-ui/src/views/LogView.tsx:36-187` (error render `:128-131`, refresh `:100-109`), subtitle path `:93-96`
- expected: When logs are unavailable, a helpful offline state with a retry action and a `RestartDaemonButton`, distinguishing "daemon offline" from "no log file yet"; never render the raw error/path.
- actual: On `readLogs`/`logDirPath` failure LogView shows a bare `<p>{error}</p>` with the raw error string (can leak a filesystem path), no retry, no RestartDaemonButton, no offline/no-file distinction, no auto-refresh, and the Refresh button shows a static "Refresh" label during load. The full `~/Library/Logs/CopyPaste` path is shown as a subtitle. Zero tests.
- why it matters: Logs are most needed when something is wrong, which is exactly when this view fails to degrade gracefully; minor path PII.
- recommended fix: Classify the error to friendly messages, add retry + RestartDaemonButton, add a 10–30s auto-refresh, show a "Refreshing…" state, tilde-collapse the path, and add ≥2 tests (error + load states). Use `ipcErrorMessage`.
- test plan: mock `read_logs` reject → assert friendly error + retry render; mock success → lines render.
- release blocker: no
- sources: UI-02 + UI-11 (audit-macos-ui-settings), F-3 + F-9 (F-macos-tauri), UX-19 (P2-reliability-ux)
- bd-title-uk: macOS LogView без офлайн-стану/тестів; сирий рядок помилки і шлях у DOM
- bd-type: bug
- bd-priority: 2

### PCA-074 — HistoryView does not refresh after a successful backup import
- severity: P2
- status: open
- platforms: macOS
- files: `crates/copypaste-ui/src/views/SettingsView.tsx:1512-1519`, `HistoryView.tsx`
- expected: After `importItems()` succeeds, HistoryView reloads to show imported items (Android reloads on import).
- actual: `handleImportFile` shows a count message but emits no signal to HistoryView; the list stays stale until the user switches tabs/reopens, making them think the import failed.
- why it matters: Stale UI after a destructive/bulk action; users distrust the import.
- recommended fix: Emit a Tauri `"history-changed"` event after import; listen in HistoryView to trigger reload (or a shared Zustand signal).
- test plan: mock import success → assert HistoryView triggers a `history_page` call.
- release blocker: no
- sources: UI-03 (audit-macos-ui-settings)
- bd-title-uk: HistoryView не оновлюється після успішного імпорту резервної копії
- bd-type: bug
- bd-priority: 2

### PCA-075 — macOS accessibility permission prompt has no completion feedback (and no test)
- severity: P2
- status: partial
- platforms: macOS
- files: `crates/copypaste-ui/src/lib/ipc.ts:1250-1261`, `crates/copypaste-ui/src/App.tsx`
- expected: After the user grants Accessibility and returns, the UI confirms the permission is granted and the feature is available.
- actual: `requestAccessibilityPermission()` opens System Settings and re-installs the CGEventTap, but there is no polling/completion callback updating the UI to "permission granted" — fire-and-forget; no test for the flow.
- why it matters: Users who grant the permission get no confirmation and may think it's broken.
- recommended fix: After requesting, poll `checkAccessibilityPermission` (every 2s ×5) and show "Permission granted — shortcut is active" on success; add a test.
- test plan: mock check returning false then true → assert UI updates.
- release blocker: no
- sources: UI-08 (audit-macos-ui-settings)
- bd-title-uk: Відсутній зворотний зв'язок після надання дозволу Accessibility (і тесту)
- bd-type: bug
- bd-priority: 2

### PCA-076 — ErrorBoundary wraps only the whole app, not individual views
- severity: P2
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/App.tsx:494-498`, `crates/copypaste-ui/src/components/ErrorBoundary.tsx`
- expected: A render crash in one view leaves the others (and the sidebar) reachable.
- actual: A single top-level `<ErrorBoundary>` wraps the entire app; a render error in e.g. HistoryView blanks the whole window, hiding Sidebar/Settings/Devices/Logs.
- why it matters: A localized render crash takes down the entire UI.
- recommended fix: Wrap each `<View />` render site in its own `<ErrorBoundary label=…>` (the component already accepts `label`).
- test plan: throw a render error from HistoryView → assert other views/sidebar remain reachable.
- release blocker: no
- sources: F-8 (F-macos-tauri)
- bd-title-uk: ErrorBoundary охоплює весь застосунок, а не окремі екрани
- bd-type: bug
- bd-priority: 2

### PCA-077 — macOS QR / SAS / daemon error surfaces render raw IPC strings (path/PII leakage)
- severity: P2
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/views/DevicesView.tsx:773-776,203-206,1436,1548`, `crates/copypaste-ui/src/lib/ipc.ts:952-955,1086-1092`, `HistoryView.tsx:2603-2604`
- expected: Friendly user-readable errors; raw detail only in `console.warn`/`<details>`.
- actual: QR error (`generateQr` catch) renders `err.message`, which can be `daemon_offline:/Users/<username>/…/daemon.sock` — exposing the macOS username. SAS `discoverError` and `ipcErrorMessage`-based toasts can render raw daemon strings (e.g. `database locked: SqliteFailure(…)`). (App.tsx `daemonError` banner is confirmed safe — generic text only.)
- why it matters: Username path PII in the UI and confusing internal error strings, inconsistent with the careful error handling in History/Devices/Settings offline states.
- recommended fix: Normalize before storing in state — detect `daemon_offline`/path prefixes and substitute friendly strings; map IPC codes to messages with raw text in a collapsible `<details>`; keep raw in `console.warn`.
- test plan: kill the daemon, open DevicesView → assert no raw socket path appears.
- release blocker: no
- sources: F-5 + F-7 (F-macos-tauri), UX-09 + UX-21 (P2-reliability-ux)
- bd-title-uk: macOS QR/SAS/daemon помилки показують сирі IPC-рядки (витік шляху/PII)
- bd-type: bug
- bd-priority: 2

### PCA-078 — Tray "Private Mode" checkmark not re-synced after a daemon restart
- severity: P2
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src-tauri/src/lib.rs:267-310,1349-1374`
- expected: After the user restarts the daemon, the tray Private Mode checkmark reflects the new daemon instance.
- actual: `spawn_tray_private_mode_resync` polls once at startup then stops; the Recent submenu poller re-syncs periodically, but the Private Mode single-shot poller does not — so after a `restart_daemon` the checkmark can show a stale value.
- why it matters: Tray shows wrong private-mode state until app relaunch.
- recommended fix: Emit a `"daemon-restarted"` Tauri event after restart and re-trigger both tray resyncs, or make the private-mode poller periodic (low frequency).
- test plan: enable Private Mode, restart daemon, check the tray checkmark reflects the reset state.
- release blocker: no
- sources: F-10 (F-macos-tauri)
- bd-title-uk: Tray "Private Mode" не ресинхронізується після перезапуску daemon
- bd-type: bug
- bd-priority: 2

### PCA-079 — SyncStatusChip can show stale "connected" for up to 10s after going offline
- severity: P2
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/components/SyncStatusChip.tsx:38`
- expected: Status reflects actual connectivity within ~1s of change.
- actual: 10s polling interval; the chip stays green for up to 10s after daemon/network loss.
- why it matters: A misleading connectivity indicator during the staleness window.
- recommended fix: Event-driven status push from the daemon, or shorten the poll to ≤2s; add a "last checked X ago" tooltip.
- test plan: drop connectivity → assert chip updates within ~2s (or shows last-checked).
- release blocker: no
- sources: UX-08 (P2-reliability-ux)
- bd-title-uk: SyncStatusChip показує застаріле "connected" до 10с після офлайн
- bd-type: bug
- bd-priority: 2

### PCA-080 — macOS private-mode active shows the wrong empty-state copy
- severity: P2
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/views/HistoryView.tsx:2647`
- expected: When private mode is on and the list is empty, show "Private mode is active — CopyPaste is not recording clipboard content."
- actual: The empty list shows "Copy something and it will appear here", telling the user to copy when the real reason is private mode is on.
- why it matters: Misleading empty-state messaging.
- recommended fix: Add a `privateMode && items.length === 0` branch with a dedicated private-mode empty state.
- test plan: enable private mode with empty history → assert the private-mode copy renders.
- release blocker: no
- sources: UX-13 (P2-reliability-ux)
- bd-title-uk: Активний private mode показує неправильний empty-state у HistoryView
- bd-type: bug
- bd-priority: 2

### PCA-081 — macOS "Revoke & rotate" / SAS confirm buttons show "..." with no accessible label
- severity: P2
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/views/DevicesView.tsx:391,636`
- expected: An accessible in-progress label (`aria-busy`, aria-label "Confirming…").
- actual: In-progress shows literal `"..."` with no ARIA attributes; screen-reader users and users who miss the ellipsis get no feedback.
- why it matters: Accessibility gap on a destructive/trust action.
- recommended fix: Replace `"..."` with `<Spinner aria-busy="true" aria-label="Confirming…">`.
- test plan: trigger the in-flight state → assert ARIA busy/label present.
- release blocker: no
- sources: UX-14 (P2-reliability-ux)
- bd-title-uk: Кнопки "Revoke & rotate"/SAS показують "..." без доступної мітки
- bd-type: task
- bd-priority: 2

### PCA-082 — Android `imageQuality` slider is dead (PNG@100 hardcoded)
- severity: P2
- status: broken
- platforms: macOS, Android
- files: Android slider `SettingsActivity.kt:985-992`, persisted `Settings.kt:849-851`, capture `ClipboardService.kt:1105` (`PNG, 100`); macOS UI `SettingsView.tsx:471,705,897,1072`; core `crates/copypaste-core/src/image.rs:181,211,237` (no quality param); UDL `image_quality`
- expected: Moving the quality slider changes image encoding (smaller files at lower quality).
- actual: Images are always re-encoded lossless PNG; `encode_as_png`/`encode_image*` take no quality param and never read `AppConfig::image_quality`. The field exists only in the config struct, IPC DTO, UDL, and the UI slider — no consumer in the encode pipeline on either platform.
- why it matters: A prominent persisted cross-platform setting is inert; users lowering it see no effect and waste storage/bandwidth.
- recommended fix: Thread `image_quality` into the encode path (JPEG/WebP when <100, PNG at 100), or remove the slider from both UIs and drop the field from `AppConfig`/UDL.
- test plan: unit test asserting encoded bytes shrink as quality decreases; round-trip test that the daemon applies the configured quality at capture.
- release blocker: no
- sources: AND-08 (audit-android), CMP-001 (P1-completeness)
- bd-title-uk: Слайдер `imageQuality` не впливає на захоплення — PNG@100 жорстко закодовано
- bd-type: bug
- bd-priority: 2

### PCA-083 — Android `pasteAsPlainText` is a structural no-op
- severity: P2
- status: stub
- platforms: Android
- files: UI `SettingsActivity.kt:680-687`, persisted `Settings.kt:808-812`, copy-back always `ClipData.newPlainText` (e.g. `HistoryActivity.kt:862`); macOS strips RTF/HTML (`ipc.rs:8492`)
- expected: Either remove the toggle on Android or document it as N/A (Android copy-back is already plain text).
- actual: Shown as a meaningful toggle with no possible effect.
- why it matters: Parity illusion — a control present with no behavior.
- recommended fix: Hide on Android or label macOS-only.
- test plan: verify the control is hidden/labeled.
- release blocker: no
- sources: AND-24 (audit-android), CMP-023 (P1-completeness)
- bd-title-uk: `pasteAsPlainText` — структурний no-op на Android
- bd-type: task
- bd-priority: 2

### PCA-084 — Android `SyncBadgeState::Syncing` never emitted; Android badge not driven by IPC `badge_state`
- severity: P2
- status: stub
- platforms: macOS, Android
- files: `crates/copypaste-ipc/src/methods.rs` (`SyncBadgeState::Syncing`, `compute_sync_badge_state`), `android/.../ui/SyncStatusBadge.kt:565-621`, `FgsSyncLoop.kt`
- expected: A "Syncing" (green-pulse/spinner) state renders while a sync round-trip is in flight; the Android badge consumes the daemon-computed canonical `badge_state`.
- actual: `compute_sync_badge_state` has no `in_flight` parameter and never returns `Syncing`; the variant is defined in Rust and `IpcSyncBadgeState.SYNCING` on Android but never produced and has no colour path. Android re-derives the badge locally from `DevicesOnlineState`/`isSupabaseConfigured` rather than reading IPC `badge_state` (no IPC socket to the daemon), so macOS and Android can disagree on auth errors.
- why it matters: The user never sees a sync-in-progress indicator; cross-platform badge disagreement.
- recommended fix: Add an `in_flight: bool` to `compute_sync_badge_state`, set from the daemon during push/receive; emit SYNCING from `FgsSyncLoop` at push/pull start with a badge animation; wire a health/badge signal into Android `DevicesOnlineState`. Or remove the variant if deferred.
- test plan: start a sync → assert the badge shows Syncing; inject an auth failure → assert Android can surface the error state.
- release blocker: no
- sources: F-19 + F-13 (audit-sync), UX-11 (P2-reliability-ux)
- bd-title-uk: `SyncBadgeState::Syncing` ніколи не виставляється; Android badge не отримує canonical `badge_state`
- bd-type: task
- bd-priority: 2

### PCA-085 — Android `maxHistoryItems` slider stored but never enforced
- severity: P2
- status: partial
- platforms: macOS, Android
- files: Android slider `SettingsActivity.kt:1069-1075` (self-documented dead at `:239,327,1067-1068`); retention is quota/age-driven (`ClipboardRepository.kt:1317,1421`); macOS UI pref `SettingsView.tsx:77,710`
- expected: A retention cap that prunes to N items, or a clear "display-only/coming-soon" label.
- actual: Stored to SharedPreferences but no Android consumer enforces it; no UI warning (the comment acknowledges `// pref-only (no daemon IPC knob yet)`). On macOS this is an intentional UI-render pref (CMP-019), but it is unlabeled as such on Android.
- why it matters: A retention slider that does nothing (low impact — quota/age caps still apply) with no user indication.
- recommended fix: Wire to the repository prune path, or add a "(coming soon)"/display-only label.
- test plan: set maxItems=10, add 20 → assert prune to 10 (once wired).
- release blocker: no
- sources: AND-25 (audit-android), UX-12 (P2-reliability-ux), CMP-019 (P1-completeness)
- bd-title-uk: Слайдер `maxHistoryItems` зберігається, але не застосовується в Android
- bd-type: task
- bd-priority: 2

### PCA-086 — Android `logcatCaptureWorking` set optimistically → misleading "WORKING" status
- severity: P2
- status: risky
- platforms: Android
- files: `android/app/.../LogcatCaptureService.kt:235-241,323`
- expected: Status reflects a confirmed non-null clipboard read.
- actual: `onDenialDetected` sets `logcatCaptureWorking = true` immediately after launching `ClipboardFloatingActivity`, before knowing if `getPrimaryClip()` returned non-null (the activity has no callback to update the flag); Settings can show WORKING on a ROM where every read returns null.
- why it matters: Misleading status — the user believes background capture works when it does not.
- recommended fix: Have `ClipboardFloatingActivity` report success/failure back (broadcast/pref); set `logcatCaptureWorking` only on a confirmed read.
- test plan: simulate a null read → assert status is not WORKING.
- release blocker: no
- sources: AND-12 (audit-android)
- bd-title-uk: `logcatCaptureWorking` встановлюється оптимістично — хибний WORKING
- bd-type: bug
- bd-priority: 2

### PCA-087 — Android dangerous-extension guard keyed on raw filename, not the sanitized name
- severity: P2
- status: risky
- platforms: Android
- files: `android/app/.../HistoryActivity.kt:1300-1346` (writes `safeName` `:1304-1307`, decides ext on `rawFileName` `:1327`)
- expected: The dangerous-extension decision uses the extension of the file actually written to disk.
- actual: `sanitizeFilename` may strip characters (e.g. zero-width), so the sanitized extension can differ from the raw one; the denylist keyed on the raw extension could miss (`evil.sh​` → raw ext `"sh​"` not matched), letting a `.sh` slip to `ACTION_VIEW`.
- why it matters: Peer-controlled filename → potential bypass of the dangerous-ext guard (narrow; still needs a handler app + OS prompt).
- recommended fix: Compute the extension from `safeName` (the written file), not `rawFileName`.
- test plan: crafted unicode filename → assert the dangerous-ext branch is taken on the sanitized name.
- release blocker: no
- sources: AND-19 (audit-android)
- bd-title-uk: Перевірка небезпечного розширення використовує raw-ім'я замість санітизованого
- bd-type: bug
- bd-priority: 2

### PCA-088 — Android notification denial silently degrades capture; no in-app warning
- severity: P2
- status: partial
- platforms: Android
- files: `android/app/.../ClipboardService.kt:238-245`, `NotificationHelper.kt:66-76,101-111`, `PermissionsSettingsActivity.kt:313-316`, `NotificationPermissionHelper.kt:56-61`
- expected: When POST_NOTIFICATIONS is denied (API 33+) or `startForeground` is rejected, the user is clearly warned that capture/controls are degraded; a pre-request rationale is shown.
- actual: The status notification (with Pause/Resume) is suppressed; nothing forces the permission and there is no in-app banner when capture is silently degraded; no pre-prompt rationale.
- why it matters: The user can be in a state where capture runs invisibly or not at all, with no signal or control.
- recommended fix: Surface a persistent in-app status banner when notifications are denied or `startForeground` fails; add a pre-permission rationale.
- test plan: deny POST_NOTIFICATIONS on API 33 → assert an in-app degraded-state banner.
- release blocker: no
- sources: AND-20 (audit-android)
- bd-title-uk: Відмова в нотифікаціях тихо погіршує захоплення без попередження в застосунку
- bd-type: task
- bd-priority: 2

### PCA-089 — `armeabi-v7a` excluded from APK abiFilters — 32-bit devices silently stub
- severity: P2
- status: open
- platforms: Android
- files: `android/app/build.gradle.kts:113` (`abiFilters += listOf("arm64-v8a")`)
- expected: 32-bit devices either get a working native lib or are excluded from the store listing.
- actual: Only arm64-v8a is packaged; a 32-bit device installs the APK but fails `System.loadLibrary("copypaste_android")` and silently falls back to stub mode (all crypto/store calls are no-ops), appearing functional but never persisting anything.
- why it matters: Silent degraded install on 32-bit devices, with no user warning.
- recommended fix: Add `armeabi-v7a` to `abiFilters` + the Rust target, or add a `<uses-feature android:name="android.hardware.type.arm64">` to exclude 32-bit from the store.
- test plan: install on a 32-bit emulator → assert either a working lib or store-level exclusion.
- release blocker: no
- sources: G-android F-4
- bd-title-uk: `armeabi-v7a` виключено з abiFilters — 32-бітні пристрої мовчки в stub-режимі
- bd-type: bug
- bd-priority: 2

### PCA-090 — Android ABI mismatch is non-fatal; ABI-gate function may be renamed by R8
- severity: P2
- status: risky
- platforms: Android
- files: `android/app/.../CopyPasteApp.kt:39`, `CopypasteBindings.kt:960-985`, `android/app/proguard-rules.pro`, `build.gradle.kts:175,179-180`
- expected: An ABI mismatch fails cleanly at startup; the ABI-gate function is protected from R8 renaming/stripping.
- actual: `checkNativeAbiCompatibility()` returns `false` on mismatch and only logs ("best-effort mode"), so a shifted FFI signature corrupts data or crashes on the next call rather than failing cleanly. In the minified build the symbol appears as `n()`; if ProGuard keep-rules are insufficient it could be stripped (silent `UnsatisfiedLinkError`).
- why it matters: For a binary ABI gate, best-effort is worse than a hard stop — the failure mode becomes invisible data corruption.
- recommended fix: Treat `false` as fatal (throw/“please update” dialog and refuse to start the service); add explicit ProGuard keep rules for the binding wrapper / ABI-check function.
- test plan: build with a bumped Rust ABI against old Kotlin → assert a graceful fatal error, not corruption; release-build smoke test confirming the ABI check is reachable.
- release blocker: no
- sources: G-android F-1 + F-2
- bd-title-uk: Невідповідність ABI не фатальна; функція ABI-гейту може бути перейменована R8
- bd-type: bug
- bd-priority: 2

### PCA-091 — Android SAS code is copyable to the clipboard during pairing
- severity: P2
- status: risky
- platforms: Android
- files: `android/app/.../DevicesActivity.kt:2326` (`setPrimaryClip(ClipData.newPlainText("SAS code", sasFull))`)
- expected: The ephemeral SAS confirmation code is not persisted to the device clipboard where another app could read it before the user confirms.
- actual: A long-press copies the 6-digit SAS to the clipboard; an app monitoring the clipboard (foreground, or with READ_LOGS) could read it before confirmation, theoretically enabling a MitM pre-confirm. (Mitigated by Android 10+ background clipboard restrictions and the short pairing window.)
- why it matters: SAS persistence in the clipboard outlives the pairing session.
- recommended fix: Remove the SAS clipboard-copy affordance, or clear the clipboard immediately after the SAS dialog closes.
- test plan: during SAS pairing, long-press the code → confirm whether it lands on the clipboard.
- release blocker: no
- sources: G-android F-7
- bd-title-uk: SAS-код можна скопіювати в буфер під час пейрингу
- bd-type: task
- bd-priority: 2

### PCA-092 — Relay `MAX_PULL_BYTES_BUDGET = 128 MiB` under a single global mutex (scale bottleneck)
- severity: P2
- status: mitigated, not eliminated
- platforms: relay
- files: `crates/copypaste-relay/src/state.rs:67,1286-1302`; SSE per-connection task at `routes/items.rs:238-330` (no per-device subscription cap); per-device rate limiter keyed on attacker-chosen `:device_id` (`routes/mod.rs:181-202`)
- expected: A single pull cannot stall all concurrent requests for more than a few ms; authenticated clients can't exhaust resources via unbounded SSE; rate limits aren't trivially bypassed.
- actual: The relay uses one global `Mutex<RelayStore>`; the 128 MiB byte-budget caps per-pull clone work (~1.3ms hold) — sufficient for small deployments but a bottleneck at high concurrency/scale. SSE subscribe spawns a long-lived task + broadcast receiver per connection with no per-device cap (authenticated self-DoS-ish). The "per-device" 60/min limiter keys on the unauthenticated path segment (bypassable by id-rotation; the per-IP 200/min limit is the real bound).
- why it matters: Scale/contention and authenticated resource-exhaustion edges; all bounded today but worth hardening before scale-out.
- recommended fix: Shard the store (hash-partitioned mutex map / DashMap) for scale; add a per-device live-subscriber cap (reuse `notifier_receiver_count`); optionally key the per-device limiter on the authenticated token.
- test plan: load test 10 concurrent 500-item pulls; open N SSE streams from one device and assert a cap; hammer rotating device_ids from one IP and assert the per-IP limit trips.
- release blocker: no
- sources: F-10 (audit-ipc-cli-relay), R1 + R2 (E-sync-relay)
- bd-title-uk: Relay: глобальний Mutex + 128 MiB budget і без ліміту SSE-підписок на пристрій
- bd-type: task
- bd-priority: 2

### PCA-093 — `WireItem::clamp_timestamps` not enforced at deserialize; negative timestamps can persist
- severity: P2
- status: open
- platforms: macOS, Android, Linux
- files: `crates/copypaste-sync/src/protocol.rs:144-164`
- expected: Negative `lamport_ts`/`wall_time` from a hostile peer cannot bias LWW; clamping is enforced at the deserialize boundary, not left to each caller.
- actual: `clamp_timestamps()` zeroes negatives but is a separate method the caller must invoke; since the production ingest path does not go through `engine.rs`, the guarantee depends on the daemon's real ingest calling clamp. `merge::resolve` tolerates negatives (total `i64::cmp`), but a negative `lamport_ts` persists and always loses LWW (or, if cast to u64 elsewhere, always wins). Whether the live daemon ingest clamps is unverifiable from the sync crate alone.
- why it matters: A malicious peer could make a tombstone/edit never win, or always win.
- recommended fix: Clamp at the deserialize boundary (`#[serde(deserialize_with)]` / custom Deserialize) or assert non-negativity in `wire_to_local`; verify the daemon clamps on every ingest path.
- test plan: negative-timestamp ingest test asserting clamping before merge.
- release blocker: no
- sources: S2 (E-sync-relay)
- bd-title-uk: `WireItem::clamp_timestamps` не примусовий при десеріалізації — негативні мітки можуть зберігатись
- bd-type: bug
- bd-priority: 2

### PCA-094 — `cloud_sign_in`/`cloud_sign_out` are undocumented wire methods; cloud controls return `not_implemented` in non-cloud builds
- severity: P2
- status: partial
- platforms: macOS, daemon
- files: `crates/copypaste-daemon/src/ipc.rs:5094,5434-5435,5444-5445`, `crates/copypaste-ipc/src/methods.rs` (no `METHOD_CLOUD_SIGN_IN/OUT`), `crates/copypaste-ui/src/lib/ipc.ts`
- expected: The UI reflects build capabilities (hides/disables cloud controls when the daemon lacks `cloud-sync`); every wire method the UI emits has a `METHOD_*` constant.
- actual: A daemon built without `cloud-sync` answers cloud UI controls with `not_implemented`, so the Sync tab can present controls that silently fail depending on the build flavour; `cloud_sign_in`/`cloud_sign_out` exist only as wire strings with no typed constant.
- why it matters: Feature presence depends on an invisible compile flag (dead controls on non-cloud builds); undocumented methods weaken the IPC contract.
- recommended fix: Add `METHOD_CLOUD_SIGN_IN/OUT` (or remove if unused); have the UI query capabilities (from `status`/`get_sync_status`) and hide/disable cloud controls when unsupported; document the build matrix.
- test plan: UI test with a no-cloud daemon → cloud controls hidden/disabled; IPC test that every emitted wire method has a constant.
- release blocker: no
- sources: CMP-004 (P1-completeness)
- bd-title-uk: `cloud_sign_in`/`cloud_sign_out` недокументовані wire-методи; cloud-контролі повертають `not_implemented` у non-cloud збірках
- bd-type: task
- bd-priority: 2

### PCA-095 — Supabase Realtime embeds `apikey` in the WS URL query string; `user_id` filter is opt-in
- severity: P2
- status: needs-improvement
- platforms: macOS, Android
- files: `crates/copypaste-supabase/src/realtime.rs:353-390,277-298,107-108`
- expected: The publishable anon key is sent as a header (not a query string); the `postgres_changes` subscription always includes a `user_id=eq.<uuid>` filter when authenticated (belt-and-suspenders with RLS).
- actual: The anon key is appended as `?apikey=<key>` (query strings are more prone to proxy/access logs than headers; logs are scrubbed and http→wss is force-upgraded, and the real secret JWT is correctly in the join body). The row filter is added only when `user_id` is `Some`; with `None` (anon scope) the join relies entirely on RLS. Whether the daemon always supplies `user_id` when signed in is unverifiable from the crate.
- why it matters: Query-string credential anti-pattern; missing `user_id` falls back to RLS-only (defense-in-depth gap on a permissive deployment).
- recommended fix: Prefer the `apikey` header if accepted, else document the RLS+TLS reliance; make `user_id` mandatory when a JWT is present.
- test plan: daemon-side test asserting the `user_id` filter is present when authenticated.
- release blocker: no
- sources: C1 + C3 (E-sync-relay)
- bd-title-uk: Supabase Realtime тримає `apikey` у query string; фільтр `user_id` опціональний
- bd-type: task
- bd-priority: 2

### PCA-096 — Backup depends on external `sqlcipher` CLI (not bundled); restore doesn't stop the daemon; no round-trip test
- severity: P2
- status: partial
- platforms: macOS, Linux
- files: `scripts/backup-db.sh:119-128,142-158`, `scripts/restore-db.sh:13,115-124`, `crates/copypaste-cli/src/commands/backup.rs:13-14,131-168,171-302`, `Casks/copypaste.rb`
- expected: Backup/restore works out-of-the-box (Homebrew install), restore is daemon-safe, and a round-trip integration test guards the critical recovery path.
- actual: Both scripts hard-fail if `sqlcipher` is not on PATH, but the Cask has no `depends_on formula: "sqlcipher"`, so a Homebrew user gets a cryptic error. `restore-db.sh` does not stop/restart the daemon (unlike `backup-db.sh`), so restoring under a live WAL connection can corrupt the restored data, and the Rust `run_restore` has no confirmation guard. The only tests are arg-wiring unit tests — no script-level backup→restore round-trip in CI.
- why it matters: The documented safe-upgrade pre-step fails for Homebrew users, can silently corrupt on restore, and the single most important data-recovery path is untested.
- recommended fix: Add `depends_on formula: "sqlcipher"` (or postflight install) and a clearer error; add daemon-stop/restart (or an active-daemon refusal) + a confirm prompt to restore; add `scripts/test_backup_restore.sh` round-trip to CI; long-term, an inline rusqlite `.backup`.
- test plan: clean Homebrew env `copypaste backup` succeeds; restore with daemon running stops it first; round-trip script asserts matching row count.
- release blocker: no
- sources: STOR-03 + STOR-04 + STOR-07 (audit-storage-logs-release), C10.6 (D-daemon-ipc), CMP-011 (P1-completeness)
- bd-title-uk: Backup залежить від зовнішнього `sqlcipher`; restore не зупиняє daemon; немає round-trip тесту
- bd-type: bug
- bd-priority: 2

### PCA-097 — macOS Release uses ad-hoc signing (no Developer ID / notarisation)
- severity: P2
- status: documented limitation
- platforms: macOS
- files: `.github/workflows/release.yml:168-173`, `Casks/copypaste.rb:28-31`, `docs/adr/ADR-010-codesigning-ad-hoc.md`
- expected: Production binaries are notarised with an Apple Developer ID so Gatekeeper accepts them without quarantine stripping.
- actual: The release pipeline uses ad-hoc signing; the Cask postflight strips quarantine via `xattr -cr`. Direct-DMG users see Gatekeeper warnings on macOS 14+; enterprise/MDM may block ad-hoc apps regardless. (Homebrew install works via the quarantine strip.)
- why it matters: A blocker for broad adoption, App Store, or enterprise deployment (acceptable for beta).
- recommended fix: Obtain a Developer ID cert; `codesign --deep --sign "Developer ID Application: …"` + `xcrun notarytool submit`; remove the Cask `xattr` strip once notarised.
- test plan: on a clean macOS 14+ machine with no Homebrew, open the DMG and verify it launches without a Gatekeeper warning.
- release blocker: no (beta) / yes (1.0)
- sources: REL-01 (audit-storage-logs-release), H-tests-ci (macOS binary assessment)
- bd-title-uk: macOS Release: відсутній Apple Developer ID та notarization — обмеження для 1.0
- bd-type: feature
- bd-priority: 2

### PCA-098 — CLI lacks `reorder_pinned`, media (`get_item_image/file/thumbnail`, `add_file_item`), and device-management commands
- severity: P3
- status: partial
- platforms: CLI
- files: `crates/copypaste-cli/src/main.rs:14-170`, `crates/copypaste-ipc/src/methods.rs:399,419-470`; daemon handlers `ipc.rs:3908,5853,6216,6368,6457,6650`
- expected: Every daemon clipboard/device method is reachable from the CLI (IPC-only first-class client).
- actual: The CLI exposes `pin`/`unpin` but no `reorder` subcommand; it cannot extract/ingest image/file media (`get_item_image/file/thumbnail`, `add_file_item`) so a user cannot save a stored image/file from the CLI; and device/peer management + SAS/password pairing (`list_peers`/`unpair`/`revoke`/`revoke_all`/discovery) is UI/Android-only.
- why it matters: CLI/GUI capability-parity gaps; scripted/headless workflows cannot reorder pins, extract binary items, or manage devices.
- recommended fix: Add `reorder`, `get <id> [--out]`, `add-file <path>`, and device-management subcommands sending the corresponding methods.
- test plan: mock-daemon tests asserting each new verb emits the right method + handles the response.
- release blocker: no
- sources: CLIP-01 + CLIP-02 (audit-clipboard), CMP-010 (P1-completeness)
- bd-title-uk: CLI без `reorder_pinned`, медіа-команд та керування пристроями
- bd-type: feature
- bd-priority: 3

### PCA-099 — Mutating storage/IPC ops and clipboard wire methods have no backend/contract tests; daemon poll/IPC tests are `#[ignore]`
- severity: P3
- status: needs-improvement
- platforms: daemon, core, all clients
- files: `crates/copypaste-core/src/storage/items.rs:1103,1311` (pin/reorder/delete fns), `crates/copypaste-daemon/tests/integration_ipc.rs` + `clipboard.rs` (all `#[ignore]`), `crates/copypaste-ipc/tests/snapshot.rs`
- expected: Every state-mutating storage + IPC path and every wire method has at least one regression/contract test that runs in CI.
- actual: `pin_item`/`unpin_item`/`reorder_pinned`/`delete_item`/`delete_all`/`soft_delete_item` have no direct test; the daemon IPC/poll integration tests are `#[ignore]` (race with the live poller / need real NSPasteboard); `snapshot.rs` covers only a few methods (no `pin_item`/`delete_all`/`reorder_pinned`/`search`/`copy_item`/media/`history_page`). Large-text payload + IPC frame-size limits are also untested.
- why it matters: Core user-facing operations and the JSON wire contract can silently regress with a green build.
- recommended fix: Add storage unit tests + a non-ignored daemon IPC harness that seeds the DB (bypassing the live poller) and inject a fake pasteboard; extend `snapshot.rs` with golden-JSON round-trips for every clipboard method; add large-text + oversized-frame tests.
- test plan: the new tests pass in CI without a desktop session and without `--ignored`.
- release blocker: no
- sources: CLIP-03 + CLIP-04 + CLIP-11 + CLIP-12 (audit-clipboard)
- bd-title-uk: Мутуючі storage/IPC операції та clipboard wire-методи без тестів; daemon poll/IPC тести `#[ignore]`
- bd-type: task
- bd-priority: 3

### PCA-100 — No search content-type filter; `search` response omits `preview`/`kind`/`pinned` (inconsistent with `list`)
- severity: P3
- status: needs-improvement
- platforms: CLI, macOS, Android, daemon
- files: `crates/copypaste-daemon/src/ipc.rs:3554-3573` (search) vs `:3421-3436` (list), `crates/copypaste-cli/src/commands/search.rs`, `crates/copypaste-ui/src/views/HistoryView.tsx:1800`, `android/.../HistoryActivity.kt:641`
- expected: Users can scope search by content type; `search` returns the same per-item fields as `list` so clients share a row renderer.
- actual: Search is full-text only with no type/kind filter on any client; the `search` arm returns `id/content_type/is_sensitive/wall_time/lamport_ts/too_large_to_sync` but no `preview`/`kind`/`pinned`/`sensitive_spans`, so the CLI prints UUID-only results and the UI needs a different renderer or a follow-up `list`.
- why it matters: Poor discoverability of non-text items; maintenance burden; CLI search output is unusable at a glance.
- recommended fix: Add a content-type/kind filter (chips / `--type`); extend the `search` arm to include `preview`/`kind`/`pinned` via `fetch_text_previews_batch`.
- test plan: assert `search` response items contain `preview`; assert the filter narrows by `content_type`.
- release blocker: no
- sources: CLIP-08 (audit-clipboard), F-03 + F-12 (audit-ipc-cli-relay)
- bd-title-uk: Немає фільтра пошуку за типом; `search` пропускає `preview`/`kind`/`pinned` (неузгодженість з `list`)
- bd-type: feature
- bd-priority: 3

### PCA-101 — CLI `copy`/`delete`/`list`/`search` use legacy IPC methods and omit fields
- severity: P3
- status: known gap
- platforms: macOS, Linux, CLI
- files: `crates/copypaste-cli/src/commands/copy.rs:4,148`, `delete.rs`, `list.rs:4`, `search.rs`; daemon arms `ipc.rs:3587,3469`
- expected: CLI uses the typed `copy_item`/`delete_item`/`history_page` verbs and surfaces their richer fields.
- actual: CLI sends legacy `copy`/`delete`/`list`/`search`; both arms are functionally equivalent (resolve by UUID), but the CLI never receives `preview` (copy_item), structured `{deleted,id}` (delete_item), or `too_large_to_sync`/`origin_device_id`/`app_bundle_id` (history_page). Future deprecation of the legacy arms would silently break the CLI.
- why it matters: Code-path divergence + test surface; missing structured output for scripts.
- recommended fix: Migrate the CLI commands to the typed methods and read the richer fields.
- test plan: CLI integration tests asserting the new method names + response shapes.
- release blocker: no
- sources: F-01 + F-02 (audit-ipc-cli-relay), P3-7 (I-parity)
- bd-title-uk: CLI `copy`/`delete`/`list`/`search` використовують legacy IPC-методи і пропускають поля
- bd-type: task
- bd-priority: 3

### PCA-102 — CLI destructive/exit hygiene: `restore` no confirm; `process::exit` skips Zeroizing drops; status output mixing
- severity: P3
- status: needs-improvement
- platforms: macOS, Linux, CLI
- files: `crates/copypaste-cli/src/commands/backup.rs:131-168`, `common.rs:90-98`, `crates/copypaste-cli/src/ipc.rs:280-286`, `status.rs:92-106,229-257`, `copy.rs:39-44`
- expected: Destructive ops confirm; secrets are zeroized before exit; machine output is clean.
- actual: `run_restore` has no Rust-layer confirmation (relies on the script). `exit_on_err`/IPC migration-retry exhaustion call `process::exit(1)` mid-command, skipping `Zeroizing<String>` password drops (~14 call sites; `cloud setup` notably). `status --json` returns exit 0 with empty stdout on serialize failure; the degraded reason is printed to stdout (not stderr) in table mode; `copy` bad-usage exits 2 (collides with `clear`'s abort code).
- why it matters: Accidental restore over the live DB; secrets linger in memory at exit; broken machine-readable contracts.
- recommended fix: Add a `restore` confirm prompt (mirror `clear`); return `Result` and let `main.rs` own the exit (drop secrets first); propagate `status --json` serialize errors; send degraded reason to stderr; use EX_USAGE (64) for bad usage.
- test plan: assert restore prompts without `--force`; assert `status --json` failure is non-zero; assert password is dropped before exit.
- release blocker: no
- sources: C10.1 + C10.2 + C10.4 + C10.5 + C10.6 + C10.7 (D-daemon-ipc), F-11 (audit-ipc-cli-relay)
- bd-title-uk: CLI: `restore` без підтвердження; `process::exit` пропускає Zeroizing drop; змішаний вивід status
- bd-type: bug
- bd-priority: 3

### PCA-103 — No CLI `reset-database` command for degraded-mode recovery
- severity: P3
- status: open
- platforms: macOS, CLI
- files: `crates/copypaste-daemon/src/ipc.rs:5529-5671` (`reset_database`, `confirm=true`), `crates/copypaste-cli/src/commands/` (no `reset`); macOS UI recovery modal exists; Android lacks any reset (P2-5 parity)
- expected: A `copypaste database reset` CLI subcommand with an interactive confirmation, so degraded-mode users have a documented recovery path; Android has an equivalent affordance.
- actual: `reset_database` exists (guarded by `confirm=true`) but no CLI command surfaces it — a degraded-mode user must craft raw IPC JSON; Android has no in-app recovery (requires OS "clear app data").
- why it matters: Degraded recovery UX is poor; raw-IPC usage bypasses the confirmation prompt; Android has no sanctioned recovery.
- recommended fix: Add `copypaste database reset` with a y/n confirm; add an Android error-state card with a confirm-gated reset (or document "clear app storage").
- test plan: `copypaste database reset` without confirmation aborts; with `--force`/`yes` invokes IPC `confirm=true`.
- release blocker: no
- sources: STOR-05 (audit-storage-logs-release), P2-5 (I-parity)
- bd-title-uk: Немає CLI-команди `reset-database` з підтвердженням для виходу з degraded режиму (+ Android recovery)
- bd-type: feature
- bd-priority: 3

### PCA-104 — `SyncEngine`/`LamportClock` struct are dead on the production path (false test confidence)
- severity: P3
- status: documented
- platforms: macOS
- files: `crates/copypaste-sync/src/engine.rs`, `clock.rs`, `lib.rs:1-28`; production uses `merge::resolve`/`remote_wins` + `copypaste_core::next_lamport_ts`
- expected: One implementation of the sync engine and Lamport clock.
- actual: `SyncEngine`/`LamportClock` in `copypaste-sync` are well-tested but NOT on the daemon production path (the daemon calls `merge::resolve` directly and stamps via `next_lamport_ts`). Tests for the unused impl give false confidence about production behavior. (E-sync-relay S1 confirms the live LWW total order in `merge.rs` is correct and exhaustively tested — the concern is the dead parallel engine, not a correctness bug.)
- why it matters: A maintainer could revive `SyncEngine` without re-validating against `merge::resolve`; misleading coverage.
- recommended fix: Either wire `SyncEngine` into production (replacing direct `merge::resolve`), or clearly mark it as test scaffold and ensure production-path tests exist separately.
- test plan: production-path merge tests independent of `engine.rs`.
- release blocker: no
- sources: F-08 (audit-sync), S1 (E-sync-relay)
- bd-title-uk: `SyncEngine`/`LamportClock` мертві на production шляху — тести дають хибну впевненість
- bd-type: task
- bd-priority: 3

### PCA-105 — No jitter in relay/P2P backoff; mDNS IP-correlation can dial the wrong peer
- severity: P3
- status: open
- platforms: macOS
- files: `crates/copypaste-sync/src/backoff.rs` (`next_delay`), `crates/copypaste-daemon/src/p2p.rs` (`DialBackoff`, `peer_connector_loop`)
- expected: Randomized jitter on backoff delays to avoid thundering-herd reconnect storms; mDNS-to-IP correlation requires a fingerprint match before dialing.
- actual: `BackoffScheduler.next_delay` and `DialBackoff` are purely deterministic (no jitter), so many devices reconnect on the same schedule after a relay/peer restart; the mDNS IP-correlation fallback can connect the wrong peer when two peers share an IP. Supabase realtime has its own separate inline backoff (consistency smell).
- why it matters: Reconnect storms; potential wrong-peer dial.
- recommended fix: Apply full/decorrelated jitter to both schedulers; require a fingerprint match in mDNS correlation; optionally unify the Supabase realtime backoff onto `BackoffScheduler`.
- test plan: assert successive `next_delay(attempt)` calls return distinct values; mDNS correlation test rejecting a non-matching fingerprint.
- release blocker: no
- sources: F-02 + F-11 (audit-sync), S3 (E-sync-relay)
- bd-title-uk: Немає джитера в relay/P2P backoff; mDNS IP-кореляція може підключити невірний пристрій
- bd-type: bug
- bd-priority: 3

### PCA-106 — Relay receive loop: 401 during burst-drain loses progress; missing `originDeviceId` on Android relay push
- severity: P3
- status: open
- platforms: macOS, Android
- files: `crates/copypaste-daemon/src/relay.rs` (`receive_loop` burst-drain), `android/.../SyncManager.kt:1082-1111` (`pushToRelay(originDeviceId = "")`)
- expected: A 401 mid-burst-drain re-registers in-loop without losing progress; relay envelopes carry `originDeviceId` for the LWW 3rd-tier tie-break.
- actual: On a full page (50 items) a 401 mid-drain clears the token and breaks the inner loop; the next outer tick re-fetches the same items (benign via LWW dedup, adds latency). Android `pushToRelay` hardcodes `originDeviceId = ""` (the `MutationRecord` has no such field), so an Android mutation could lose LWW to a stale macOS mutation on equal lamport+wall_time.
- why it matters: Re-fetch latency on the macOS side; incorrect LWW tie-break for Android mutations in a rare equal-timestamp case.
- recommended fix: Re-register in-loop on a 401 during burst drain; add `originDeviceId` to `MutationRecord`, populate from `Settings.deviceId`, forward to `pushToRelay`.
- test plan: 401 mid-burst → assert the watermark advances without a wasted re-fetch; equal-timestamp tie-break test with a non-empty `originDeviceId`.
- release blocker: no
- sources: F-09 + F-15 (audit-sync)
- bd-title-uk: relay receive_loop 401 під час burst-drain губить прогрес; Android relay push без `originDeviceId`
- bd-type: bug
- bd-priority: 3

### PCA-107 — Android `OutboundMutationQueue` Supabase tombstone push status unclear; Supabase poll limit=20 slow catch-up
- severity: P3
- status: open
- platforms: macOS, Android
- files: `android/.../SyncManager.kt:1013-1028` (`pushMutationRow`), `OutboundMutationQueue.kt`, `crates/copypaste-daemon/src/cloud.rs` (`POLL_SELECT_QS &limit=20`)
- expected: Delete/pin/unpin mutations propagate to both relay and Supabase; the Supabase poll drains a large backlog efficiently (relay uses `PULL_LIMIT=50` with burst-drain).
- actual: An in-code table comment said Supabase delete was a "gap"; CopyPaste-yaip partially wired `pushMutationRow` for deletes+pins, but `pushMutationRow` itself was not read — unclear if the PATCH path fully implements tombstones. Separately, Supabase poll `limit=20` requires 10 round-trips for a 200-item backlog (slower than relay's 50), and whether cloud.rs implements the burst-drain loop is unconfirmed.
- why it matters: Possibly incomplete Supabase tombstone propagation; slow cloud catch-up after offline.
- recommended fix: Verify `pushMutationRow` PATCHes `deleted=true`; update/remove the stale comment table; raise the Supabase poll limit to 50 and implement burst-drain.
- test plan: delete on Android → assert the Supabase row has `deleted=true`; assert cloud catch-up drains a 200-item backlog in fewer round-trips.
- release blocker: no
- sources: F-12 + F-20 (audit-sync)
- bd-title-uk: Android Supabase tombstone через `pushMutationRow` неясний; Supabase poll limit=20 повільне відновлення
- bd-type: task
- bd-priority: 3

### PCA-108 — Migration ladder integration tests end at v4 (v5–v11 untested on real files); FTS5 not rebuilt by vacuum
- severity: P3
- status: missing test coverage / latent risk
- platforms: all
- files: `crates/copypaste-core/tests/migration.rs:33-42` (v5 crash-resume is `todo!()`/`#[ignore]`), `schema.rs:57` (`SCHEMA_VERSION = 11`), `crates/copypaste-daemon/src/ipc.rs:5731-5734` (vacuum `REINDEX`), `schema_v1.sql:7-8`
- expected: At least one on-disk "real v5/v6/.../v10 file → migrate to v11" test (matching v1→v4 coverage); vacuum rebuilds the FTS5 external-content index.
- actual: Ladder tests exist for v0→v4 (+v8→v10) on disk but v5 (UNIQUE index), v6 (migration_state seeding), v7 (pinned column), and v11 (covering index) have no real-file test (only in-memory atomicity/column-presence). Vacuum runs `REINDEX`, which is a no-op for FTS5 virtual tables — if the FTS index goes stale (crash mid-insert / WAL edge), vacuum won't repair it; the correct repair is `INSERT INTO clipboard_fts(clipboard_fts) VALUES('rebuild')`.
- why it matters: A future schema bump could silently break the v5→v11 ladder; search could silently omit/return ghost results after a rare crash, and vacuum won't fix it.
- recommended fix: Add an on-disk v6 migration_state integration test (+ implement the v5 crash-resume test); append the FTS5 `rebuild` to the vacuum handler.
- test plan: stage an encrypted v6 DB, open via `Database::open`, assert `completed_at` set; insert items, delete their FTS rows, vacuum, assert search finds them again.
- release blocker: no
- sources: STOR-01 + STOR-06 (audit-storage-logs-release)
- bd-title-uk: Тести міграції закінчуються на v4 (v5–v11 без on-disk тестів); vacuum не відновлює FTS5
- bd-type: task
- bd-priority: 3

### PCA-109 — Export silently skips image/file items with no user warning
- severity: P3
- status: by-design, undocumented at call site
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/ipc.rs:8113-8118` (`if content_type != "text" { continue; }`), `crates/copypaste-cli/src/commands/export.rs:8-13`; future-hazard note `B-crypto F-09`
- expected: `export` exports all item types, or clearly warns (before export) that image/file items are excluded.
- actual: Only text items are exported; image/file items are silently dropped with no user-facing warning and no mention in `--help`. A user treating the export as a full backup gets an incomplete backup; after `reset_database` + re-import, all image/file items are permanently lost. (B-crypto also flags that a future code path bypassing the `content_type` guard would export image blobs without the text path's `Zeroizing` wrapper.)
- why it matters: Silent incomplete backup → permanent data loss on a reset+reimport.
- recommended fix: Emit `excluded_non_text_count` in the response + a stderr warning when >0 (and/or document the limitation in `--help`); assert the `content_type` filter as an invariant tied to the Zeroizing requirement.
- test plan: assert `excluded_non_text_count` is present when image items exist.
- release blocker: no
- sources: STOR-02 (audit-storage-logs-release), B-crypto F-09
- bd-title-uk: export мовчки пропускає зображення/файли без попередження користувача
- bd-type: bug
- bd-priority: 3

### PCA-110 — CI gaps: no ESLint, no `cargo test --all-features`, Android lint not on PRs, instrumented tests never run, committed/fallback keystores, non-blocking quality jobs
- severity: P3
- status: open
- platforms: all (CI)
- files: `.github/workflows/ci.yml`, `ci-matrix.yml:137`, `ci-android-build.yml`, `release.yml:414-461`, `nightly.yml:117,130-133`, `fuzz-smoke.yml:35`, `android/app/build.gradle.kts:127-133`, `android/app/debug.keystore`, `android/local.properties`, `scripts/build-android-apk.sh:124`
- expected: Frontend linting, all-features test coverage, Android lint on PRs, instrumented crypto tests run somewhere, no committed keystores/machine files, security-critical quality jobs gate merges.
- actual: No ESLint/`lint` script in the UI crate; `feature-matrix` runs `clippy --all-features` but not `cargo test --all-features` (cloud-sync untested); Android lint only in the release gate (not PRs); `connectedAndroidTest`/`CryptoConformanceTest` never run in any workflow (nightly comment is false); `debug.keystore` + `local.properties` committed and a `copypaste-beta` fallback keystore password is in `build-android-apk.sh`; `unused-deps`, `cargo-deny advisories`, and `fuzz-smoke` are `continue-on-error: true`; `nightly-android` depends on a 2× `ubuntu-latest-xlarge` runner.
- why it matters: React/Kotlin/crypto regressions and fuzz crashes can land green; committed keystores are an attack surface.
- recommended fix: Add ESLint + a `pnpm lint` CI step; add offline doubles for `cargo test --all-features` (or at least `cargo check --all-features`); run Android lint on PRs; add an emulator job for `connectedAndroidTest`; gitignore + document keystores, require `KEYSTORE_PASS`; promote security-critical fuzz targets to `continue-on-error: false`; fall back the xlarge runner.
- test plan: each CI step added is exercised on a PR.
- release blocker: no
- sources: H-tests-ci F-01..F-12, TC-11 (P3-testcoverage), REL-02 context (audit-storage-logs-release)
- bd-title-uk: Прогалини CI: немає ESLint, `cargo test --all-features`, Android lint на PR, instrumented тестів; закомічені keystore; неблокуючі quality-джоби
- bd-type: task
- bd-priority: 3

### PCA-111 — Test-coverage gaps: relay cross-device token, FTS-on-TTL, PAKE confirm, nonce/wrong-key, detector recall, ErrorCode round-trip, keychain ACL
- severity: P3
- status: needs-improvement
- platforms: all
- files: `crates/copypaste-relay/src/state.rs:970,1009`, `crates/copypaste-core/src/storage/items.rs:1971-1982`, `crates/copypaste-core/src/crypto/encrypt.rs:225-232`, `db.rs`, `tests/false_positive_corpus.rs`, `crates/copypaste-ipc/src/error.rs`, `crates/copypaste-daemon/tests/keychain_acl.rs` (all `#[ignore]`), `crates/copypaste-daemon/src/pairing_sm.rs:368`
- expected: Adversarial/contract tests for: wrong-token cross-device inbox access (401), FTS rows removed on `delete_expired`, the mandatory PAKE confirm-tag rejection, large-N nonce uniqueness, SQLCipher wrong-key rejection, a true-positive sensitive corpus (recall), exhaustive `ErrorCode` serde round-trip, and the Keychain ThisDeviceOnly attribute.
- actual: None of these exist as runnable CI tests — `verify_token` cross-device is untested; `delete_expired` asserts only item count, not FTS cleanup; the PAKE confirm path is covered only by `#[ignore]` e2e; nonce uniqueness is a 2-sample test; no wrong-key open test; only a false-positive corpus (no recall corpus); no exhaustive ErrorCode round-trip; all `keychain_acl.rs` tests are `#[ignore]`.
- why it matters: Silent regressions in auth isolation, FTS-leak-after-expiry, MitM-pairing rejection, crypto, and detector recall could land green.
- recommended fix: Add the recommended tests (cross_device_token_rejected, delete_expired_also_cleans_fts, pake_missing_confirm_rejected, 1000-sample nonce uniqueness, wrong_key_returns_error, a 50-item true-positive corpus, ErrorCode enum round-trip, and a compile-time keychain-attribute check).
- test plan: the listed tests pass in CI.
- release blocker: no
- sources: TC-01..TC-10 (P3-testcoverage)
- bd-title-uk: Прогалини покриття: relay cross-device token, FTS-on-TTL, PAKE confirm, nonce/wrong-key, recall детектора, ErrorCode round-trip, keychain ACL
- bd-type: task
- bd-priority: 3

### PCA-112 — Android parity gaps: no P2P LAN sync, single-transport SyncBackend, missing recovery/export, SAS metadata card, single-IP display
- severity: P3
- status: open
- platforms: Android
- files: `android/.../Settings.kt:22-27` (SyncBackend), `DevicesActivity.kt:1953,2306-2346`, `crates/copypaste-android/uniffi/copypaste_android.udl` (P2P FFI exists), `ClipboardRepository.kt`
- expected: Documented parity (or implementation) for: P2P/mTLS LAN sync, additive relay+cloud transports, history export/import, degraded-DB reset, the SAS peer-metadata card, and all discovered-peer IPs.
- actual: Android has no P2P LAN sync loop (core FFI is exported but unused); `SyncBackend` is mutually exclusive (macOS runs relay+cloud additively); there is no Android history export/import (no UDL function); no in-app DB-reset recovery (see PCA-103); the SAS dialog omits the peer-metadata card macOS shows; discovered-peer rows show only the first IP. The biggest user-facing one (no Android backup/export) is from P1-1 (I-parity scoped it Android-only).
- why it matters: Android users cannot back up/migrate history, get worse LAN latency, and have a weaker pairing-corroboration UX; mostly undocumented (the actual defect is the lack of documentation + the missing export).
- recommended fix: Add `export_history`/`import_history` FFI + a Settings SAF picker (highest value); document P2P/single-transport limitations in `docs/known-issues.md` or build the Android P2P loop on the existing FFI; render the SAS peer-metadata card; join all discovered IPs.
- test plan: Android export→import round-trip (sensitive excluded by default); SAS dialog renders name/IP/fingerprint; multi-IP discovered peer shows all IPs.
- release blocker: no
- sources: P1-1 + P2-1 + P2-2 + P2-6 + P2-7 (I-parity)
- bd-title-uk: Android parity: немає P2P LAN sync, single-transport SyncBackend, немає recovery/export, SAS metadata, лише перший IP
- bd-type: feature
- bd-priority: 3

### PCA-113 — GUI maintenance/parity gaps: no DB-level backup/restore, no vacuum/stats UI; CLI backup bypasses the daemon
- severity: P3
- status: open
- platforms: macOS, Android, CLI
- files: `crates/copypaste-ui/src/views/SettingsView.tsx`, `crates/copypaste-ui/src/lib/ipc.ts`, `crates/copypaste-daemon/src/ipc.rs:5677` (vacuum); `crates/copypaste-cli/src/commands/backup.rs` (shells to scripts)
- expected: GUI parity with CLI maintenance ops (encrypted DB backup/restore, vacuum, stats); a coherent backup data path.
- actual: Neither GUI exposes encrypted `.db.enc` backup/restore (Settings has plaintext JSON export/import only); `vacuum`/`stats` have IPC support but no macOS-UI surface; CLI backup/restore bypass IPC and shell out to scripts (parallel data path, no daemon coordination).
- why it matters: Maintenance-op discoverability gap; architectural boundary violation in CLI backup.
- recommended fix: Add a "Compact database" (vacuum) button + a stats readout to Settings → Storage; document encrypted DB backup as CLI-only or add a GUI affordance.
- test plan: UI test mocking `vacuum`/`stats` asserting the call + result render.
- release blocker: no
- sources: P2-3 + P2-4 (I-parity), CMP-011 (P1-completeness)
- bd-title-uk: GUI без backup/restore рівня БД та vacuum/stats; CLI backup обходить daemon
- bd-type: feature
- bd-priority: 3

### PCA-114 — Theme-default doc/spec drift: PARITY-SPEC says light-first, both platforms default dark
- severity: P3
- status: open
- platforms: macOS, Android, docs
- files: `docs/PARITY-SPEC.md` §0, `crates/copypaste-ui/src/App.tsx:272-283`, `android/.../ui/theme/Theme.kt:33`, `Settings.kt:44,55,562,576`, `Color.kt:17`
- expected: One documented default theme matching the source-of-truth PARITY-SPEC §0 (currently "light-first").
- actual: Both platforms now default to dark (`52mz`/`c48e`), but PARITY-SPEC §0 and the in-file comments (`Theme.kt:33`, `Settings.kt:562`) still claim light-first — stale and self-contradicted two lines below in `Settings.kt`. Behavior is consistent cross-platform (both dark); the defect is doc/spec drift. (Also: stale token-legend comments in `Color.kt`/`Theme.kt` listing wrong accent/faint hex though the actual tokens are correct.)
- why it matters: A source-of-truth design contract silently reversed; contradictory comments mislead contributors.
- recommended fix: Decide one default; if dark stays, update PARITY-SPEC §0 + the stale comments; if light is required, revert the `52mz`/`c48e` fallbacks; fix the `Color.kt`/`Theme.kt` legend.
- test plan: assert the default-theme constant matches the spec; parity test that web and Android defaults agree.
- release blocker: no
- sources: P1-3 + P3-4 (I-parity)
- bd-title-uk: Дрейф дефолтної теми: PARITY-SPEC каже light-first, обидві платформи дефолтять dark
- bd-type: bug
- bd-priority: 3

### PCA-115 — Android STUN public-IP response not transaction-ID-validated (spoofable, informational value only)
- severity: P3
- status: mostly complete
- platforms: Android
- files: `android/.../StunUtils.kt` (`parseXorMappedAddress` matches any 0x0101 response)
- expected: A STUN client verifies the response transaction ID matches the request (RFC 5389) so an off-path UDP responder cannot inject a forged mapped address (the daemon's `public_ip.rs` does this).
- actual: The Android parser ignores the transaction ID and accepts the first 0x0101 response; a LAN attacker racing a UDP reply could set a bogus public IP. Low impact — `public_ip` is informational and never used for auth/trust — but it is a cross-platform inconsistency and a displayed value.
- why it matters: A spoofable, user-visible value; the daemon already does it right.
- recommended fix: Copy the request transaction ID and require an exact match; drop non-matching datagrams.
- test plan: response with a mismatched tx-id → returns null; matching tx-id → parses.
- release blocker: no
- sources: PAIR-06 (audit-pairing-devices)
- bd-title-uk: Android STUN не перевіряє transaction ID відповіді — підроблюваний public IP
- bd-type: bug
- bd-priority: 3

### PCA-116 — Pairing UX/dead-code gaps: CLI `pair-qr --raw` scroll-back; `poll_peer_events` wiring unverified; Android Devices states/dead helpers; empty pairing-notification name
- severity: P3
- status: partial / unverified
- platforms: CLI, daemon, Android, macOS
- files: `crates/copypaste-cli/src/commands/pair_qr.rs:34-45`, `crates/copypaste-ipc/src/methods.rs` (`poll_peer_events`), `android/.../DevicesActivity.kt` (no initial-load/offline state `:621`, non-copyable own fingerprint `:1923-1925`, dead `DeviceField` `:2602-2618`), `ClipboardService.kt:560-563,1533-1534` (empty peer name)
- expected: Minimal lingering pairing-token exposure; verified peer-event delivery; Android Devices has loading/offline states + tap-to-copy fingerprint + no dead code; the pairing notification shows the real peer name.
- actual: `pair-qr --raw` prints the (single-use, 120s) token to stdout with only a stderr warning (acceptable, but the only QR surface with no privacy gating); `poll_peer_events` producer/consumer wiring was not confirmed end-to-end; Android Devices lacks initial-load/offline states, the own fingerprint is non-copyable, and `DeviceField` is unused; the incoming-pair notification always uses an empty peer name (dead non-blank branch), with duplicated size-cap constants.
- why it matters: Minor token scroll-back exposure; possible missed/late incoming-pairing prompts; Android Devices polish/consistency gaps.
- recommended fix: Optionally gate `--raw`; trace and document/implement `poll_peer_events`; add Android Devices loading/offline states, make the own fingerprint tap-to-copy, remove `DeviceField`, pass the real peer name, centralize cap constants.
- test plan: trace `poll_peer_events` producers/consumers; UI review of Android Devices states; incoming pair → notification shows the peer name.
- release blocker: no
- sources: PAIR-07 + PAIR-08 (audit-pairing-devices), AND-26 + AND-27 (audit-android)
- bd-title-uk: Прогалини пейринг-UX: `pair-qr --raw` scroll-back; `poll_peer_events` неперевірений; стани/мертвий код Android Devices; порожнє ім'я в нотифікації
- bd-type: task
- bd-priority: 3

### PCA-117 — `ClipboardFloatingActivity` duplicates `dispatchClipData`; `FgsSyncLoop` dies if dial-setup throws before the per-peer loop
- severity: P3
- status: risky
- platforms: Android
- files: `android/.../ClipboardService.kt:830-844` (canonical dispatcher), `ClipboardFloatingActivity.kt:183-236` (inline reimpl), `FgsSyncLoop.kt:372,384,634` (`dialPairedPeer` outside `poll()` try/catch)
- expected: Both capture call sites delegate to `dispatchClipData`; a throw in dial setup is caught and the loop continues.
- actual: The background-capture path (the one that matters most on Android 10+) reimplements the three-phase image/file/text dispatch inline instead of calling `dispatchClipData` — the exact duplication the "BUG 1 fix" was meant to remove. Separately, `dialPairedPeer()` is invoked outside the `poll()` try/catch, so a throw in dial setup (`localItemsForSync`, `encryptionKey`, `deviceKeyStore.peek()`) kills the whole `while(isActive)` coroutine, stopping both catch-up polling and P2P dialing until service restart.
- why it matters: Future MIME fixes must be applied twice or the background path diverges; a single transient error silently terminates all background sync.
- recommended fix: Have `ClipboardFloatingActivity.onFocusedLayout` call `ClipboardService.dispatchClipData(...)`; wrap `dialPairedPeer()` in try/catch that logs and continues.
- test plan: assert both paths produce identical capture; inject a throw in dial setup → assert the loop survives.
- release blocker: no
- sources: AND-21 + AND-22 (audit-android)
- bd-title-uk: `ClipboardFloatingActivity` дублює `dispatchClipData`; `FgsSyncLoop` гине, якщо dial-setup кидає до циклу по пирам
- bd-type: bug
- bd-priority: 3

### PCA-118 — Android critical paths (capture/key/restart) + loaded-`.so` FFI errors untested; many tests cosmetic/copy-of-logic
- severity: P3
- status: partial
- platforms: Android
- files: `android/.../ClipboardService.kt`, `DeviceKeyStore.kt`, `ServiceRestartWorker.kt`, `CopypasteBindings.kt` (Err mapping), `TextKind.kt:40-49`; in-test reimplementations (`AuthoritativePinStateTest`, `TombstoneCatchUpTest`, `HistoryParityTest`), `LiveP2pSyncTest.kt:74-78` (assumeTrue-skipped)
- expected: Critical runtime paths and the Rust-`Err`→graceful-Kotlin mapping are unit-tested; the `TextKind` Kotlin fallback matches the Rust classifier; `fallbackDefaultConfig()` literals match Rust defaults.
- actual: Clipboard capture, key storage, service restart, and real FFI error propagation have no JVM coverage; ~45% of test methods are cosmetic (skin/glass/color) and several "behavioral" tests reimplement the logic they assert (tautologies); `LiveP2pSyncTest` silently skips without orchestrator args. The `TextKind` Kotlin fallback algorithm can diverge from Rust (stub-mode vs prod), and `fallbackDefaultConfig()` hardcodes values that can drift from `copypaste-core` defaults.
- why it matters: The riskiest Android paths (capture, master key, FFI errors) can regress with a green build; stub/prod classification divergence makes coverage misleading.
- recommended fix: Extract capture/key/restart logic into pure helpers and unit-test them; add a loaded-`.so` FFI error-mapping test; add a JVM test that the `TextKind` fallback matches the canonical classification; add a CI assertion that `default_config()` FFI matches the Kotlin fallback constants; replace in-test reimplementations with calls to shipped functions; make `LiveP2pSyncTest` skips visible.
- test plan: a Rust `Err` surfaces as the correct `CopypasteException` subtype (not a crash); fallback classification matches; default-config parity holds.
- release blocker: no
- sources: AND-23 (audit-android), G-android F-5 + F-9, TC-12 (P3-testcoverage)
- bd-title-uk: Критичні Android-шляхи (capture/ключ/restart) та loaded-`.so` помилки без тестів; багато тестів косметичні/копії логіки
- bd-type: task
- bd-priority: 3

### PCA-119 — Architecture/docs drift, dead constants, and `todo!()` test hazards
- severity: P3
- status: open
- platforms: all (docs/CI)
- files: `ARCHITECTURE.md:8-16,149-150`, `crates/copypaste-ipc/src/lib.rs:78` + `crates/copypaste-p2p/src/bootstrap.rs:143` (duplicated `QR_PAIRING_TTL_SECS`), `crates/copypaste-relay/src/middleware/rate_limit.rs:28-36` (unwired constants), `crates/copypaste-daemon/tests/lifecycle.rs:124,142` + `core_integration.rs:16` + `migration.rs:41` (`todo!()`)
- expected: ARCHITECTURE.md reflects the real dep graph and current pins; shared constants have a single source; `todo!()` test bodies are `#[ignore]`-guarded; relay rate-limit constants are wired into the governor.
- actual: ARCHITECTURE.md hides the `sync→core` edge, understates `copypaste-android` deps (omits p2p/sync), and lists removed pins; `QR_PAIRING_TTL_SECS=120` is hand-duplicated across ipc and p2p (drift silently breaks pairing); four relay rate-limit constants carry `#[allow(dead_code)]` and are not read by the governor; several `todo!()` test bodies risk panicking if un-ignored in CI.
- why it matters: Doc drift misleads contributors; duplicated/unwired constants are silent-break hazards; a live `todo!()` panics rather than skips.
- recommended fix: Update ARCHITECTURE.md (dep edges + pins); centralize the QR TTL constant (move to core + re-export, or add a drift compile-assert); wire the relay rate-limit constants into the `GovernorConfigBuilder`; confirm every `todo!()` test is `#[ignore]`-annotated.
- test plan: a constant-drift assertion test; verify `cargo test` does not panic on the lifecycle/migration stubs.
- release blocker: no
- sources: A-architecture F-2 + F-3 + F-8 + F-9 + F-10 + F-14, H-tests-ci F-08
- bd-title-uk: Дрейф ARCHITECTURE.md, дубльовані/непідключені константи та `todo!()` у тестах
- bd-type: task
- bd-priority: 3

### PCA-120 — `#[allow(dead_code)]`/blanket-allow cleanup and unzeroized-on-takeover dead-code review
- severity: P3
- status: open
- platforms: macOS, Linux
- files: `crates/copypaste-daemon/src/ipc.rs:1177,1677,7767,8956`, `crates/copypaste-daemon/src/cloud.rs:57`, `crates/copypaste-relay/src/error.rs:13-16,39-41` (`DeviceConflict`, `HistoryQuotaExceeded`), `crates/copypaste-core/src/storage/db.rs:520-537` (`open_in_memory` pub)
- expected: Dead-code allows have a concrete future plan or are removed; test-only/unencrypted helpers are gated; blanket lint suppressions are scoped to tests.
- actual: `device_public_key` (`IpcServer`) carries a "future use" allow with no read path; `cloud.rs:57` uses a blanket `#[allow(unused_imports)]` broader than needed; relay `DeviceConflict`/`HistoryQuotaExceeded` variants are dead-but-retained (documented); `Database::open_in_memory` (unencrypted) is `pub` and could be misused outside tests.
- why it matters: Cognitive overhead and small footgun risk (e.g. an accidental `open_in_memory` runs storage without encryption).
- recommended fix: Remove `device_public_key` if no concrete plan (or file a bd issue); scope the `cloud.rs` import suppression to `#[cfg(test)]`; gate `open_in_memory` with `#[cfg(test)]`/a `_test` suffix; convert the dead relay variants to `#[cfg(test)]` or remove once R1a is stable.
- test plan: `cargo build --workspace` after each removal; verify no production caller of `open_in_memory`.
- release blocker: no
- sources: A-architecture F-12 + F-13, C-storage F-04, F-07 + F-13 (audit-ipc-cli-relay)
- bd-title-uk: Прибирання `#[allow(dead_code)]`/blanket-allow та небезпечний `pub open_in_memory`
- bd-type: task
- bd-priority: 3

### PCA-121 — Relay observability/precision: no version in `/health`, `registered_at` via `Instant::elapsed()`, silent-prune backpressure, 500 echoes internal strings
- severity: P3
- status: open
- platforms: relay
- files: `crates/copypaste-relay/src/routes/health.rs:13`, `routes/devices.rs:166-168`, `quota.rs:113`, `state.rs:1152-1181`, `error.rs:45-60,87-95`
- expected: Operators can read the running relay version; `GET /devices/:id` returns the true `registered_at`; inbox overflow has some backpressure signal; 500 bodies don't echo internal strings.
- actual: `/health` returns only `{status:"ok"}` (no `build_version` at any route); `registered_at` is approximated as `now_unix - record.registered_at.elapsed()` (drifts with clock/NTP/DST) instead of reading the persisted SQLite Unix timestamp; inbox overflow silently prunes oldest with a 201 success (no sender backpressure; the `HistoryQuotaExceeded` 413 path is dead); `Storage`/`Internal` 500 bodies echo `format!`-ed SQL/IO strings (no plaintext, but leaks schema/path detail).
- why it matters: Deploy/canary checks lack a version; reported registration time can be wrong; receivers may silently miss items; minor info disclosure on 500s.
- recommended fix: Add `build_version` to `/health` (or an authed `/info`); read `registered_at_unix` from SQLite; add an "inbox size" field to `GET /devices/:id` (or document the silent-prune); return a generic 500 body, keep detail in `tracing`.
- test plan: `/health` contains `build_version`; register + advance mock clock → `GET /devices/:id` still returns the original time; force a 500 → generic body.
- release blocker: no
- sources: F-06 + F-08 + F-15 (audit-ipc-cli-relay), R3 + R4 (E-sync-relay)
- bd-title-uk: Relay observability/точність: `/health` без версії, `registered_at` через `elapsed()`, тихий prune, 500 з внутрішніми рядками
- bd-type: task
- bd-priority: 3

### PCA-122 — macOS UI polish: double stale-daemon banner, thin behavioral test coverage, protocol-mismatch message direction, sensitive-export safeguard
- severity: P3
- status: partial
- platforms: macOS
- files: `crates/copypaste-ui/src/App.tsx`, `SettingsView.tsx:2648-2658`, `src/lib/ipc.ts:176-203`, `SettingsView.test.tsx:167-183` (stale `get_limits` mock), various `*.skin.test.*`
- expected: One canonical stale-daemon banner; behavioral state-mapping tests per view; a protocol-mismatch message that distinguishes "upgrade app" vs "restart daemon"; clean test fixtures.
- actual: App.tsx and SettingsView can both show a stale-daemon banner (possible double banner); a large fraction of UI tests assert CSS skin tokens rather than behavior (LogView untested, AboutView degraded state, wifi-only/lan-visibility save/revert, SyncStatusChip fallback, history reload after import all untested); the protocol-mismatch banner shows one generic "upgrade" message that is wrong when the daemon is behind (should be "restart daemon"); a stale `get_limits` mock in `SettingsView.test.tsx` references a non-existent method.
- why it matters: UI feels unpolished; skin tests don't catch behavior regressions; users may take the wrong remediation; stale fixtures reduce test clarity.
- recommended fix: Centralize stale-daemon detection; add per-view loading/empty/error/offline behavioral tests; branch the protocol-mismatch message on `daemonVersion <> CURRENT_PROTOCOL_VERSION`; remove the `get_limits` mock.
- test plan: assert one stale-daemon banner at a time; assert the correct mismatch message per direction; add the missing behavioral tests.
- release blocker: no
- sources: UI-07 + UI-10 + UI-13 + UI-14 (audit-macos-ui-settings)
- bd-title-uk: Поліш macOS UI: подвійний банер застарілого daemon, тонке behavioral-покриття, напрямок повідомлення protocol-mismatch, safeguard експорту чутливих
- bd-type: task
- bd-priority: 3

### PCA-123 — `paste_to_frontmost` 80ms focus-delay race; QR/visibility edge UX; `ToastProvider` mounting unverified
- severity: P3
- status: partial
- platforms: macOS
- files: `crates/copypaste-ui/src-tauri/src/lib.rs:631-680`, `crates/copypaste-ui/src/views/DevicesView.tsx:841-891`, `crates/copypaste-ui/src/components/Toast.tsx:129-167`, `src/App.tsx`
- expected: Reliable paste-to-frontmost; QR drain bar doesn't show a stale 0 on tab restore; `ToastProvider` is mounted so `useToast().show()` is visible.
- actual: `paste_to_frontmost` sleeps a fixed 80ms before synthesizing Cmd+V — on a slow machine the paste can land in the wrong app (known OS limitation, `NSApplicationActivateIgnoringOtherApps` deprecated). On tab restore after a long hide, the QR drain bar can briefly show 0/negative before regen. `Toast.tsx` defines a full `ToastProvider`/`useToast` with a graceful no-op fallback, but App.tsx mounting was not confirmed — if unmounted, `useToast().show()` is silently swallowed everywhere except HistoryView's separate inline Toast.
- why it matters: Mis-paste into the wrong app; brief stale QR; potentially lost toast feedback in non-History views.
- recommended fix: Make the focus delay a tunable constant (consider 120ms); call `tick()` synchronously on visibility restore; verify/add `<ToastProvider>` in App.tsx and consolidate the inline HistoryView Toast.
- test plan: paste into a slow app; tab away >2min then return (no stale 0); render the full app and assert a `useToast().show()` produces a visible toast.
- release blocker: no
- sources: F-11 + F-17 (F-macos-tauri), UI-12 (audit-macos-ui-settings)
- bd-title-uk: `paste_to_frontmost` 80мс гонка фокусу; крайові випадки QR/visibility; монтування `ToastProvider` не підтверджено
- bd-type: bug
- bd-priority: 3

### PCA-124 — Remaining UX P3s: silent load-more/search-fallback failures, history error-flash, no scanning state, import warning, app-bundle-id exposure, Android scroll/i18n
- severity: P3
- status: needs-improvement
- platforms: macOS, Android
- files: `crates/copypaste-ui/src/views/HistoryView.tsx:1250-1252,1637,1720,1804`, `SettingsView.tsx:2539`, `DevicesView.tsx:1397`, `android/.../SettingsActivity.kt:505-618,870-872`
- expected: Load-more/search-fallback failures are surfaced; no error-flash before degraded state resolves; a "Scanning…" state during initial mDNS; a pre-action restore warning; app-bundle-id not redundantly exposed; Android density labels translatable + tab switch resets scroll.
- actual: Load-more failures and silent FTS→client-side-fallback are not signalled (user assumes no more history / complete results); a brief error state flashes before the degraded state resolves; "No devices found" shows instantly with no scanning state; restore opens the file picker with no pre-warning; `app_bundle_id` is shown verbatim in the detail-modal footer (already in the row chip); Android density labels are hardcoded (not in strings.xml) and tab switches don't reset scroll.
- why it matters: Misleading completeness signals and minor polish/i18n/consistency gaps.
- recommended fix: Surface load-more/search-fallback failures (toast/notice); suppress the error-flash; add a "Scanning…" state; add a restore pre-warning; redact/omit the footer bundle id; move density labels to strings.xml; reset scroll on tab switch.
- test plan: induce a load-more failure → assert a notice; assert "Scanning…" appears during initial discovery; assert restore shows a warning before the picker.
- release blocker: no
- sources: UX-16 + UX-17 + UX-18 + UX-22 + UX-24 + UX-27 + UX-28 (P2-reliability-ux), F-12 (F-macos-tauri)
- bd-title-uk: Залишкові UX P3: тихі збої load-more/пошуку, error-flash, відсутній стан сканування, попередження імпорту, показ app-bundle-id, Android scroll/i18n
- bd-type: task
- bd-priority: 3

### PCA-125 — `ip_with_port` 0.70-confidence pattern auto-wipes private IP:port pastes (false-positive data loss)
- severity: P3
- status: known/by design, not communicated
- platforms: all
- files: `crates/copypaste-core/src/sensitive/patterns.rs:192-197`, `crates/copypaste-core/src/sensitive/detector.rs` (`AUTOWIPE_CONFIDENCE_FLOOR = 0.70`)
- expected: A private IP:port (e.g. `192.168.1.1:8080`) in a config/log paste is not treated as a credential and auto-wiped; the blurred-vs-autowiped distinction is communicated.
- actual: `ip_with_port` has confidence 0.70 — exactly at the autowipe floor — so `kubectl port-forward`/nginx/SSH-tunnel pastes get `is_sensitive=true` and are silently TTL-wiped (~30s). Conversely, several 0.65 patterns (SSN, IBAN, email, Discord/Twilio/bearer) are below the floor, so they are flagged/blurred but never autowiped — and the UI never explains which items are autowiped vs only blurred.
- why it matters: False-positive silent data loss on common IP:port pastes; and a user who sees a blurred SSN/IBAN wrongly assumes it will auto-wipe.
- recommended fix: Lower `ip_with_port` to 0.65 (below the floor) or exclude RFC-1918 ranges; clarify in Settings which items are autowiped vs only blurred (or add a secondary TTL).
- test plan: assert `192.168.1.1:8080` does not trigger autowipe; verify the Settings copy explains the confidence-floor distinction.
- release blocker: no
- sources: C-storage F-10, SENS-06 (audit-sensitive)
- bd-title-uk: `ip_with_port` (0.70) авто-видаляє приватні IP:port — false-positive; межа autowipe не пояснена в UX
- bd-type: bug
- bd-priority: 4

### PCA-126 — Orphaned/dead UI prefs and stubs: `previewSize`, Advanced tab, `supabase_account_id`, duplicated default shortcut
- severity: P4
- status: partial / intentional-but-unlabeled
- platforms: macOS
- files: `crates/copypaste-ui/src/store.ts:39-40` (`previewSize` not exposed), `SettingsView.tsx:370-379` (hidden "Advanced" tab stub), `src/lib/ipc.ts:433` (`supabase_account_id` TODO), `SettingsView.tsx:44` + `src-tauri/src/lib.rs:14` (duplicated `DEFAULT_POPUP_SHORTCUT`)
- expected: No orphaned pref fields/stubs; the default-shortcut constant has a single source.
- actual: `previewSize` (UIPrefs) has no Settings control (programmatic-only); the "Advanced" tab `renderAdvanced()` exists but is hidden from the tab bar (coming-soon stub); `supabase_account_id` is not exposed from daemon status (TODO); `DEFAULT_POPUP_SHORTCUT` is hand-duplicated in TS and Rust (drift risk for the "reset to default" button).
- why it matters: Dead/duplicated UI surface increases cognitive load and drift risk (low impact).
- recommended fix: Expose or remove `previewSize`; remove the Advanced stub or ship it; expose `supabase_account_id`; expose the default shortcut over IPC (or add a drift test).
- test plan: n/a (refactor) / a test asserting the TS and Rust default shortcuts match.
- release blocker: no
- sources: UI-05 (audit-macos-ui-settings), CMP-018 + CMP-021 + CMP-022 (P1-completeness), F-14 (F-macos-tauri)
- bd-title-uk: Осиротілі/мертві UI-параметри і стаби: `previewSize`, Advanced tab, `supabase_account_id`, дубльований default shortcut
- bd-type: task
- bd-priority: 4

### PCA-127 — FileChip list-row renders a hardcoded MIME (`application/octet-stream`)
- severity: P4
- status: needs-improvement
- platforms: macOS
- files: `crates/copypaste-ui/src/views/HistoryView.tsx:693-694`, `crates/copypaste-daemon/src/ipc.rs:4639` (`history_page` file branch returns no MIME)
- expected: The list row shows the file's real MIME/type.
- actual: `history_page` does not include the file MIME, so the UI passes `mime="application/octet-stream"` to FileChip until the user triggers Save (which calls `get_item_file` returning the true MIME). Cosmetic.
- why it matters: The type label/icon may be generic until the file is opened/saved.
- recommended fix: Include the file MIME (from `blob_ref` `FileMeta.mime`) in the `history_page` file-row JSON.
- test plan: daemon test asserting a file row in `history_page` carries the stored MIME.
- release blocker: no
- sources: CLIP-14 (audit-clipboard)
- bd-title-uk: FileChip у рядку macOS показує hardcoded MIME (`application/octet-stream`)
- bd-type: task
- bd-priority: 4

### PCA-128 — Low-impact platform/cosmetic items: rekey O(n) key scan, image dedup hash truncation, ad-hoc Keychain ThisDeviceOnly skip, launchd plist USERNAME, Linux log-path docs, tauri.conf version sed, Linux CI, db_key backup docs, Android `Log.d` insert-id
- severity: P4
- status: open / informational
- platforms: macOS, Android, Linux, docs
- files: `crates/copypaste-daemon/src/sync_orch.rs` (`rekey_inbound` O(n) scan), `crates/copypaste-daemon/src/clipboard.rs:94-99` (16-byte image dedup hash), `crates/copypaste-daemon/src/keychain/mod.rs:217-228` (ad-hoc skip), `packaging/macos/com.copypaste.daemon.plist:47-48` (`USERNAME`), `crates/copypaste-daemon/src/logging.rs:151-160` (Linux log path undocumented), `.github/workflows/release.yml:112-133` (`sed` version patch), `.github/workflows/ci.yml:35-37` (cargo test macOS-only), `docs/ops/backup-restore.md` (db_key backup steps), `android/.../ClipboardService.kt:961` (`Log.d` native insert id)
- expected: O(1) pairwise-key lookup; documented/de-risked cosmetic items.
- actual: A grab-bag of low-impact items: inbound rekey does an O(n) linear scan over peer keys (negligible at <10 peers); image dedup hash is truncated to 16 bytes (sound, no DB space saving vs full digest); ad-hoc builds silently skip the `ThisDeviceOnly` Keychain hardening (dev-only iCloud-sync risk); the legacy launchd plist hardcodes literal `USERNAME` in log paths (logs lost if installed verbatim); the Linux log path is coded but undocumented; `tauri.conf.json` version is `sed`-patched in CI (stale local-build version, no single source); `cargo test` runs only on macOS (Linux paths untested beyond `cargo check`); `docs/ops/backup-restore.md` lacks db_key export/recovery steps (esp. from the macOS Keychain); Android logs the native insert UUID at `Log.d` (debug-only noise).
- why it matters: Individually minor; grouped here to avoid dropping them. The db_key backup-docs gap is the most user-relevant (disaster-recovery clarity).
- recommended fix: Pass the sender fingerprint through to enable O(1) key lookup; document/de-risk each item (fix the plist `USERNAME`, document the Linux log path, move tauri version to build-time injection, add `cargo test` on Linux or accept MSRV-check coverage, add a db_key backup section incl. `security find-generic-password -s com.copypaste.daemon -w`, gate the Android `Log.d` behind `BuildConfig.DEBUG`).
- test plan: per-item (e.g. a sender-fingerprint key-lookup test; a doc review for db_key/Linux-log/launchd).
- release blocker: no
- sources: F-14 (audit-sync), D-daemon-ipc D2.4 + D3.4, B-crypto F-06, LOG-03 + REL-03 + REL-04 + REL-05 (audit-storage-logs-release), G-android F-10
- bd-title-uk: Низькоimpact платформні/косметичні пункти: O(n) rekey scan, усічений image-hash, ad-hoc Keychain skip, `USERNAME` у launchd, Linux log-path docs, tauri.conf sed, Linux CI, db_key backup docs, Android `Log.d`
- bd-type: task
- bd-priority: 4

---

## Truncation note

All 20 source fragments were read in full and every distinct finding was carried forward — nothing was silently dropped. Where a single PCA entry merges several source findings (notably the grouped P3/P4 entries PCA-110, PCA-111, PCA-116, PCA-119, PCA-120, PCA-121, PCA-122, PCA-124, PCA-126, PCA-128), the contributing source IDs are listed in that entry's `sources:` field so the merge is fully traceable. Pure PASS / "no issue" / positive-finding notes (e.g. A-architecture F-1/F-4/F-5/F-6/F-7/F-11/F-15, B-crypto F-05/F-07/F-08 + the 13 positive findings, D-daemon-ipc I5.2/I7.1/C10.8/C10.9/C10.10, E-sync-relay D1/P1-1/P1-2/P3-1/P3-2/C2/C4 + cross-cutting, F-macos-tauri F-1/F-2/F-6/F-13/F-15/F-16, G-android positives, the I-parity "Refuted" section, and H-tests-ci version/cask/secrets PASS rows) were intentionally excluded as non-gaps, not truncated.
