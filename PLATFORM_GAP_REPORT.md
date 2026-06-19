# CopyPaste — Cross-Platform Gap Report

Date: 2026-06-19 · Tracking: CopyPaste-t5qm · Source: 7 parallel read-only parity streams (daemon ↔ IPC ↔ CLI ↔ macOS-Tauri ↔ Android).
Severity policy: security/privacy drift ≥P1 · pairing/QR/device-identity drift ≥P1 · misleading sync-status drift ≥P1 · feature missing on one platform without documented reason ≥P2 · visual-only P2/P3. Static reading only (Rust compile gates blocked: rustc 1.95 < MSRV 1.96).

**Key architectural fact:** Android has **no daemon process**. It calls `copypaste-android` UniFFI in-process and stores state in `ClipboardRepository` (SharedPreferences + FFI crypto). macOS/Linux/CLI all speak the Unix-socket IPC to `copypaste-daemon`. "Parity" for Android means functional equivalence through FFI, not the same wire protocol. This is correct by design, but it is the root cause of most drift below: **Android re-implements logic the daemon owns**, so the two can diverge silently.

Counts: **P1 = 15**, **P2 = 28**, **P3 = 18** (+ cross-refs to already-filed audit issues).

---

## P1 — High (fix first)

### PG-1 · Android inbound P2P listener silently drops control frames (Unpair ignored)
- expected: macOS `ControlMsg::Unpair` over an established P2P link removes the peer on Android.
- macOS: reads `PeerFrame` (Data|Control); handles Unpair.
- Android: `p2p_listener.rs:324-340` `run_connection` deserializes `WireItem` only; any `PeerFrame::Control` (Unpair/Ping/Pong) is discarded. (Outbound `sync_with_peer` handles it — only the inbound listener is broken.)
- CLI/daemon: n/a.
- files: `crates/copypaste-android/src/p2p_listener.rs:324-340`.
- fix: deserialize as `PeerFrame`; on `Control(Unpair)` remove peer from allowlist + revoked set and close.
- test: inject a serialized `ControlMsg::Unpair` frame → assert peer session key removed.
- status: open.

### PG-2 · Relay registration PoP derivation missing from Android FFI
- expected: Android registers with HMAC-SHA256(sync_key, "relay-registration-pop-v1:"||inbox_id), same as daemon.
- macOS: `relay.rs:392-410` computes + sends `pop_b64`.
- Android: `relay_inbox_id`/`relay_public_key_b64` exported (`lib.rs:469,492`) but `derive_relay_registration_pop` is NOT exported → Kotlin can't compute PoP → registration fails or skips PoP.
- files: `crates/copypaste-android/src/lib.rs:469-502`, `uniffi/copypaste_android.udl`, `crates/copypaste-daemon/src/relay.rs:392-410`.
- fix: export `relay_registration_pop(sync_key, inbox_id) -> String`; never log it; Kotlin passes it at registration.
- test: derive in Rust, assert Kotlin call byte-matches; mock-relay integration validating PoP.
- status: open.

### PG-3 · Android drops sensitive items at capture (AB-6b incomplete)
- expected: sensitive captured items stored + marked `is_sensitive=true` (the documented AB-6b intent at `ClipboardRepository.kt:689` "PARITY with macOS: do NOT drop").
- macOS: `daemon.rs:1924` stores + marks + stamps `expires_at`.
- Android: `ClipboardService.kt:892-896` (text), `:993` (image), `:1169` (file) early-return when sensitive → never stored, never in history/UI. Contradicts its own AB-6b comment.
- files: `android/.../ClipboardService.kt:892-896,993,1169`, `daemon.rs:1924-2107`.
- fix: remove the sensitive early-return; call `storeItem(..., isSensitive=true)` + set TTL — OR, if drop-at-capture is the real product intent, update the AB-6b comment and document the asymmetry. **Decide the contract** (interacts with PG-4/PG-15 and CopyPaste-jbao).
- test: inject sensitive text → assert stored with `isSensitive=true` and expiry within TTL.
- status: open.

### PG-4 · Sensitive-span masking missing on Android (secret shown as plaintext)
- expected: items not fully `is_sensitive` but containing a matched secret substring get that span bullet-masked (`•••`).
- macOS: daemon emits `sensitive_spans` (`ipc.rs:4428-4455`); `masking.ts` + `HistoryView.tsx:427` + `Popup.tsx` apply it.
- Android: `ClipboardItem.kt` has no `sensitiveSpans`; whole-item blur only → a credit-card/IBAN buried in a longer item shows **unmasked** on Android.
- files: `crates/copypaste-daemon/src/ipc.rs:4428-4455`, `crates/copypaste-ui/src/lib/masking.ts:36-53`, `android/.../ClipboardItem.kt`, `HistoryActivity.kt`.
- fix: add `sensitiveSpans` to `ClipboardItem`; compute via FFI `detect()` at parse-time or carry from sync; bullet-mask spans when `!isSensitive && spans.isNotEmpty()`.
- test: store text with an embedded IBAN → assert masked range in the row.
- status: open.

### PG-5 · Export emits sensitive plaintext with no filter/warning
- expected: export excludes sensitive items (or requires explicit `--include-sensitive` + warns).
- macOS/CLI: `ipc.rs:7638-7795` decrypts ALL text rows incl `is_sensitive=true` into `content_bytes_b64`; `export.rs` adds no filter.
- Android: export not available.
- files: `crates/copypaste-daemon/src/ipc.rs:7638-7795`, `crates/copypaste-cli/src/commands/export.rs`.
- fix: add `exclude_sensitive: bool` (default true) to the export IPC; CLI `--include-sensitive` opt-in; audit-log every export (cross-ref CopyPaste-tj9s).
- test: export with one sensitive + one normal → only normal present by default.
- status: open (cross-ref CopyPaste-tj9s).

### PG-6 · Tauri bridge drops `protocol_version` → UI version-mismatch handler is dead code
- expected: UI detects protocol mismatch and prompts to upgrade.
- macOS: `IpcReply` (`src-tauri/src/ipc.rs:57-62`) omits `protocol_version`; TS `protocolMismatchHandler` never fires (field always `undefined`); also the request never sends `protocol_version` (daemon sees 0).
- CLI: handles `version_mismatch` correctly.
- files: `crates/copypaste-ui/src-tauri/src/ipc.rs:57-62,83,99`, `crates/copypaste-ui/src/lib/ipc.ts:75-128`.
- fix: add `protocol_version: Option<u32>` to `IpcReply`, populate from daemon JSON; send `protocol_version` in the request. (Pairs with CopyPaste-ptb8: daemon emits `invalid_argument` not `version_mismatch`.)
- test: Tauri bridge test that `IpcReply.protocol_version` is populated.
- status: open.

### PG-7 · `ipc_not_ready` handled only in HistoryView (other views fail generically)
- expected: every DB-touching view degrades gracefully on `ipc_not_ready`.
- macOS: `HistoryView.tsx:1545-1547` branches (incl legacy uppercase); DevicesView/Popup/SettingsView show a generic error instead.
- files: `crates/copypaste-ui/src/views/{HistoryView,DevicesView,SettingsView}.tsx`, `popup/Popup.tsx`.
- fix: extract a shared `handleIpcNotReady` into `lib/ipc.ts`; apply in all views.
- test: mocked `ipc_not_ready` in DevicesView/Popup → degraded UI, not raw error.
- status: open (overlaps CopyPaste-8u2b).

### PG-8 · PairActivity re-blurs the QR on every auto-refresh (privacy-hostile)
- expected: blur state is user-owned and persists across QR regeneration (spec §10; DevicesActivity does this correctly).
- macOS: `handleQrRegenerate` keeps `qrBlur` untouched.
- Android: `PairActivity.kt:427` sets `qrRevealed=false` inside `generateQr()` → fires on every 120 s TTL auto-refresh, re-blurring without user action; `DevicesActivity` OwnQrSection does NOT do this (reference impl).
- files: `android/.../PairActivity.kt:427`.
- fix: remove `qrRevealed=false` from `generateQr()`; reset blur only at initial launch.
- test: reveal → trigger `generateQr()` → assert still revealed.
- status: open.

### PG-9 · Own-device fingerprint shown on Android but absent on macOS
- expected: both platforms show own mTLS cert fingerprint for identity verification.
- macOS: `ThisDeviceCard` (`DeviceCard.tsx:125-148`) shows Model/OS/IP — no fingerprint row; `get_own_fingerprint` IPC exists, unused here.
- Android: `DevicesActivity.kt:1559` shows full fingerprint.
- files: `crates/copypaste-ui/src/components/DeviceCard.tsx`, `crates/copypaste-daemon/src/ipc.rs` (`get_own_fingerprint`).
- fix: add fingerprint row to `ThisDeviceCard` (truncated + tap-to-copy full).
- test: UI snapshot; assert fingerprint matches daemon output.
- status: open.

### PG-10 · Sync badge "offline" uses different signals across platforms
- expected: "Offline" means the same thing on both.
- macOS: offline = IPC/daemon unreachable; no network check.
- Android: offline = OS `NET_CAPABILITY_VALIDATED` false (`SyncStatusBadge.kt:172-178`); no IPC-health awareness.
- impact: Android daemon-equivalent failure → "Idle"; macOS no-network → "Idle". Same condition, opposite badge.
- files: `crates/copypaste-ui/src/components/SyncStatusChip.tsx`, `android/.../SyncStatusBadge.kt`.
- fix: compute badge state daemon-side; distinct `DaemonUnreachable` vs `NetworkOffline` states (see Shared Status Enum below).
- status: open.

### PG-11 · Sync badge "connected" recency gate exists only on macOS
- expected: both agree on connected vs idle for a live-but-stale peer.
- macOS: `RECENT_SYNC_MS=300_000` → stale link shows "idle".
- Android: `count>0` only (`SyncStatusBadge.kt:111`) → any live peer shows "connected".
- impact: after 5 min idle, macOS="idle", Android="connected" simultaneously.
- files: `SyncStatusChip.tsx`, `SyncStatusBadge.kt`.
- fix: add a shared `Stale` state with `STALE_THRESHOLD_MS=300_000` computed daemon-side.
- status: open.

### PG-12 · Revoke peer + sync-key rotation missing on Android
- expected: revoking a peer offers optional in-place sync-key rotation on both.
- macOS: revoke dialog has "Revoke & rotate" with passphrase field.
- Android: `DevicesActivity.kt:577` confirms revoke only; no rotation.
- files: `android/.../DevicesActivity.kt:577`, daemon `rotate_sync_key`.
- fix: add passphrase-rotation step to Android revoke dialog → `rotate_sync_key`.
- status: open.

### PG-13 · Supabase email + password fields missing from macOS UI
- expected: macOS Sync tab has email+password sign-in like Android.
- macOS: only a read-only "Signed in" row; `set_config` accepts the fields (`ipc.rs:141-149`) but no UI inputs → users must use `copypaste cloud setup` CLI.
- Android: `SettingsActivity.kt:1126-1141` has both fields.
- files: `crates/copypaste-ui/src/views/SettingsView.tsx:1772-1910`, `ipc.ts:279-289`.
- fix: add a Supabase sign-in section (email + `type=password`) wired through `set_config`.
- status: open.

### PG-14 · Android private mode not restored on degraded daemon startup (macOS daemon)
*(daemon-side privacy gap surfaced by the parity sweep)*
- expected: private mode survives restarts incl degraded boot.
- macOS: normal path (`daemon.rs:399`) calls `load_private_mode()`; degraded path (`daemon.rs:1506`) inits `AtomicBool::new(false)` without loading → a user in private mode silently resumes capture after a degraded boot.
- files: `crates/copypaste-daemon/src/daemon.rs:1506` vs `:399`.
- fix: `AtomicBool::new(load_private_mode())` in the degraded path.
- test: degraded boot with persisted private_mode=1 → `get_private_mode` returns true.
- status: open.

### PG-15 · Sensitive-item sync asymmetry between platforms
- expected: one consistent, documented contract for whether sensitive items sync.
- macOS: stores sensitive locally (TTL) AND uploads (no `is_sensitive` filter in relay/cloud/P2P push) → CopyPaste-jbao.
- Android: drops at capture (PG-3) so never uploads its own; but DOES store inbound sensitive items from macOS (sync-in path re-derives `is_sensitive`, `ClipboardRepository.kt:1417`). Net: macOS→Android sensitive flows; Android→macOS does not.
- files: `relay.rs:564`, `cloud.rs:993`, `ClipboardService.kt:892`, `ClipboardRepository.kt:1417`.
- fix: pick one model end-to-end (recommend: never upload sensitive on either platform → add `is_sensitive` filter to daemon push paths AND keep/justify Android drop). Resolve jointly with PG-3, PG-5, CopyPaste-jbao.
- status: open (cross-ref CopyPaste-jbao).

---

## P2 — Medium (missing feature / inconsistent behavior, no documented reason)

### Architecture (Android re-implements daemon-owned logic — silent-divergence risk)
- **PG-16** Android content-type **classification** re-implemented in `TextKind.kt` vs core `text_kind.rs` — priority order matches but code-signal logic differs (`{;` vs `contains(';') && contains('{')`). files: `android/.../TextKind.kt`, `crates/copypaste-core/src/text_kind.rs`. fix: drive Android from the daemon/FFI-emitted `kind` or share one classifier via FFI.
- **PG-17** Android **search** re-implements FTS5 as an O(N) full-content decrypt scan (different ranking/recall). files: `ClipboardRepository.searchIds`. fix: expose an FFI search over the same index or document the divergence.
- **PG-18** Android **copy-back** decrypts + writes clipboard locally and bumps recency locally (not synced) — `HistoryActivity` bypasses daemon-style broadcast. files: `ClipboardRepository.bumpToTop`.
- **PG-19** Android **ordering** sorts unpinned by `wallTimeMs`; daemon orders by `lamport_ts` → cross-device list order can differ. files: `ClipboardRepository` sort, `get_page_pinned_first`.

### IPC / contract
- **PG-20** `migration_in_progress` has no client backoff/retry on CLI or UI (error docs say "back off and retry"). files: `error.rs:59-61`, `ipc.ts`, `cli/commands/common.rs`.
- **PG-21** `ipc.ts` JSDoc marks `supabase_email`/`supabase_password` as read-only/ignored, but daemon `set_config` accepts+stores them — misleading for future UI authors. files: `ipc.ts:279-289`, `ipc.rs:141-149`.

### Sensitive / privacy
- **PG-22** `is_sensitive_app` (password-manager source attribution) is **dead code on both platforms** — items from 1Password/Bitwarden are not auto-marked unless content matches a pattern. files: `core/src/sensitive/detector.rs:274-313` (zero callers), `daemon.rs:1924`, `ClipboardService.kt`.
- **PG-23** Android `sensitive_kind` (any-confidence) vs `is_sensitive` (≥0.70) threshold mismatch — `sensitive_kind` can be non-null while `is_sensitive` is false. files: `android/src/lib.rs:289-298`.
- **PG-24** Android lacks per-item `expires_at`; relies on `pruneByAge` called only from `getItems()` → a suspended app keeps expired sensitive items. files: `ClipboardRepository.kt:1128-1177,146`. fix: WorkManager periodic prune.
- **PG-25** macOS history window has **no screenshot guard**; Android forces `FLAG_SECURE`. fix: wire Tauri `setContentProtected(true)` to `maskSensitive`. files: `HistoryActivity.kt:224-227`, `HistoryView.tsx`.
- **PG-26** `import` trusts caller-supplied `is_sensitive` (TTL-evasion via crafted export). fix: recompute via `is_sensitive_for_autowipe` on decrypt. files: `ipc.rs:7346-7440`.

### Sync transports
- **PG-27** Android relay/cloud poll latency 60 s (active) / 15 min (Doze) vs macOS 5 s relay + Supabase WebSocket (<1 s). files: `relay.rs:95`, `docs/android-background.md`. (documented design; flagged for parity.)
- **PG-28** Android does not collect its own STUN public IP (`build_android_peer_meta: public_ip=None`) → WAN P2P to Android harder. files: `android/src/lib.rs:849`, daemon `public_ip.rs`.

### Settings
- **PG-29** `lan_visibility` (mDNS visibility) toggle missing on Android. files: `SettingsActivity.kt`, `Settings.kt` (no key).
- **PG-30** `sync_enabled` master kill-switch missing on macOS. files: `SettingsView.tsx`, vs `SettingsActivity.kt:588`.
- **PG-31** `auto_apply_synced_clip` has no UI on either platform and is absent from the IPC `AppConfig` struct (exists in core config `mod.rs:103`). files: `ipc.rs` AppConfig.
- **PG-32** macOS history-limit slider not persisted (`setPrefs` never called; no `maxItems` in `UIPrefs`). files: `SettingsView.tsx:2106-2120`, `store.ts`. (extends CopyPaste-2b1g)
- **PG-33** Density default mismatch: macOS `compact` (`store.ts:97`) vs Android `comfortable` (`Settings.kt:33`).
- **PG-34** `show_sensitive_warnings` toggle missing on macOS (Android `SettingsActivity.kt:806`).
- **PG-35** Android `private_mode` is SharedPrefs-only (not daemon-backed) — architecturally fine but verify the FFI reads it before capture; document. files: `SettingsActivity.kt:580`, `Settings.kt:795`.
- **PG-36** Liquid-Blue `:root` fallback still `#3D8BFF` on macOS (`index.css:120`) vs canonical `#4D8DFF` — parity-check.mjs tests the palette block, not `:root`. files: `index.css:120`. fix: set `--ide-accent-rgb: 77 141 255`; extend parity-check to assert `:root`.

### Devices / sync status
- **PG-37** Offline peer dot color: macOS red (`bg-ide-danger`) vs Android grey (`c.faint`). files: `DeviceCard.tsx:66`, `DevicesActivity.kt:2089`.
- **PG-38** Own-device name: macOS user-set ComputerName vs Android `Build.MODEL` (hardware string). files: `get_own_device_info`, Android own-device row.
- **PG-39** Peer Local IP: macOS falls back to `extractIp(peer.address)`, Android omits when `peerLocalIp` null. files: `DeviceCard.tsx:215`, `DevicesActivity.kt:1193`.
- **PG-40** Last-sync display: macOS absolute date, Android relative elapsed. files: `DeviceCard.tsx`, `DevicesActivity.kt`.
- **PG-41** Sync-badge device count uses a binary fallback on Android before DevicesScreen is opened. files: `DevicesActivity.kt:460`.
- **PG-42** Sync-badge metadata (last-sync + masked email) shown via macOS tooltip; absent on Android (no hover). fix: tap-to-expand bottom sheet. files: `SyncStatusChip.tsx`, `SyncStatusBadge.kt`.
- **PG-43** Discovered-peer "Pair disabled (no bport)" hint shown on Android, silent on macOS. files: `DevicesView.tsx`.
- **PG-44** Cloud-misconfig surfaced in macOS tooltip only vs Android badge chip.
- **PG-45** Peer fingerprint not shown in the peer row on either platform (only own device) → inline identity unverifiable.
- **PG-46** Android no UI action for "clear all" (only `clearUnpinned`; `clearAll()` is dead ViewModel code). files: `ClipboardViewModel`, `HistoryActivity.kt:249`.

### Pairing
- **PG-47** Android truncates peer fingerprint (`take16+…+last8`) in the SAS confirm modal; macOS shows full 64-char → weaker verification at the security-critical step. files: `DevicesActivity.kt formatPeerFingerprint`, `PairActivity.kt`.

---

## P3 — Low (visual / UX / minor)

- **PG-48** Settings tab placement mismatches: mask-sensitive (macOS General / Android Display), sound+notify (General / Notifications), image-quality (Storage / Display), excluded-apps (General / Storage).
- **PG-49** TEXT chip color: macOS accent-blue vs Android grey (`izio` comment, no spec entry).
- **PG-50** Content icons: TEXT (Type vs ContentCopy), PATH (FolderOpen vs AttachFile), NUMBER (Hash vs Tag).
- **PG-51** Timestamp format: macOS absolute locale, Android relative, CLI UTC.
- **PG-52** URL row: macOS hostname-only bold vs Android `scheme://host`.
- **PG-53** Sensitive chip: macOS italic-dim + icon vs Android explicit "PRIVATE" text chip.
- **PG-54** Tap-after-reveal: macOS auto-copies, Android shows a toast (no copy).
- **PG-55** macOS toast missing the leading semantic dot (Android has it). files: `Toast.tsx`, `GlassToast.kt`.
- **PG-56** QR auto-refresh margin: macOS refreshes at ≤15 s (no dead QR), Android refreshes at 0 s (briefly shows expired). files: `DevicesView.tsx QR_REFRESH_MARGIN_SECS`, `PairActivity.kt`, `DevicesActivity.kt`.
- **PG-57** Card radius 12 px (macOS) vs 14 dp (Android, cites oha3/5686) — reconcile PARITY-SPEC §4.
- **PG-58** Relay URL field: macOS always visible, Android hidden unless relay mode selected.
- **PG-59** `previewLinesPopup` exists on macOS, single `previewLines` on Android (deferred until Android popup exists).
- **PG-60** Discovered-poll cadence: macOS 3 s, Android 2 s; SAS poll macOS ~1 s, Android 500 ms (battery vs latency).
- **PG-61** Blur radius 6 px (macOS) vs 5 dp (Android); API<31 Android uses bullet substitution (no GPU blur).
- **PG-62** 13+ methods used by the UI via string literals are absent from `methods.rs` constants (string-drift risk) — cross-ref CopyPaste-x2c6.
- **PG-63** CLI `list` response lacks `preview`/`kind`/`sensitive_spans`/`pinned` (uses legacy `list`, not `history_page`).
- **PG-64** Icon pulse animation present on Android, removed on macOS (`s7ia`).
- **PG-65** relay-api.md wire format stale (also CopyPaste-17lj).

---

## Proposed shared model — `SyncBadgeState` (resolves PG-10/PG-11)

Compute daemon-side, deliver over IPC; both platforms render identically:
```
enum SyncBadgeState {
  DaemonUnreachable,  // IPC socket missing/refused
  NetworkOffline,     // OS has no validated internet
  Idle,               // reachable, 0 online peers
  Connected,          // reachable, ≥1 online peer, last_sync ≤ threshold
  Stale,              // reachable, ≥1 online peer, last_sync > threshold (NEW)
  Syncing,            // active transfer
  Error(String),      // relay/cloud/crypto error
}
```
Same principle (shared definitions over reinventing per platform) applies to PG-16 (classification), PG-4 (sensitive spans), and the device-metadata field set.

---

## Documented platform limitations (intentional, not gaps)

- **Android has no daemon** — SharedPreferences + FFI is the authority; daemon-config settings are not daemon-authoritative on Android.
- **Android background**: clipboard capture needs an AccessibilityService (Android 10+); FGS dies under Doze/OEM killers → sync latency degrades to 15 min; inbound P2P only while FGS alive. (docs/android-background.md)
- **Android API<31**: no GPU blur → bullet-substitution fallback.
- **macOS has no QR camera scan** (Android scans macOS's QR); **macOS has no always-on FGS** (uses launch agent / app-owned daemon, ADR-014).
- **macOS no system FLAG_SECURE** — addressable via Tauri `setContentProtected` (PG-25).
- **CLI** is display/scripting only: no blur, countdown, SAS, or devices/sync UI; `watch` is terminal-only; `backup`/`restore` shell out to scripts (daemon stop/start).
- **Windows** frozen (ADR-012) — excluded from parity.
