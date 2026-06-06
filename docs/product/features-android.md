# Android App — Complete Feature Inventory

> Branch: `v0.6.1-integration` | Audited: 2026-06-04  
> Source root: `android/app/src/main/java/com/copypaste/android/`

---

## 1. Shell & Navigation (`MainActivity.kt`)

### 1.1 Three-Tab Bottom Navigation
- **Purpose:** Root activity hosting the persistent three-tab shell.
- **Tabs:** Clips (clipboard history), Devices, Settings — `NavTab` enum with Material3 `NavigationBar`.
- **Selected-tab indicator:** `IdeAccent` tint + 15 % alpha indicator pill.
- **Screen instantiation:** each tab composable is re-created in-place; state survives rotation via `rememberSaveable`.
- **Sync-status strip:** a `SyncStatusBadge` footer sits *below* the tab content area showing app label left + coloured online-device dot + count right (parity with macOS sidebar chip). Published by `DevicesOnlineState` singleton.
- **FLAG_SECURE:** `WindowManager.LayoutParams.FLAG_SECURE` is set on this window before `setContent`, blocking screenshots and recents thumbnails for the entire app lifetime (`MainActivity.kt:116–119`).
- **Settings unsaved-changes guard:** tapping a navbar tab while Settings has pending edits routes through a `settingsNavGuard` lambda; a Discard/Keep-editing `AlertDialog` intercepts the navigation.
- **Limitations:** no drag-to-reorder tabs; no badge count on the Clips tab; tab bar is always visible (no full-screen immersive mode).

### 1.2 Onboarding (`OnboardingActivity.kt`)
- **Purpose:** First-run and permission-missing screen.
- **Permissions covered:** POST_NOTIFICATIONS (runtime), Overlay / SYSTEM_ALERT_WINDOW, Battery Optimization exemption, OEM autostart (`OemAutoStartHelper`).
- **Re-entry:** `MainActivity.onResume` calls `OnboardingActivity.allCriticalGranted()` and re-launches onboarding if any critical permission is absent (once per Activity lifetime, tracked by `onboardingShownThisSession`).
- **Limitations:** no deep-link back to the specific system settings pane for every OEM; ADB-grant commands shown as tap-to-copy text rather than automated.

---

## 2. Clipboard Capture

### 2.1 Always-On Foreground Service (`ClipboardService.kt`)
- **Purpose:** Persistent `specialUse` foreground service (declared per Google Play policy) that owns all clipboard capture, sync channels, P2P listener, and the notification.
- **Service type:** `FOREGROUND_SERVICE_TYPE_SPECIAL_USE` on API 34+; legacy type 0 on older APIs (`ClipboardService.kt:212–221`).
- **Capture path:** registers `ClipboardManager.OnPrimaryClipChangedListener` on the main thread; fires `dispatchClipData()` shared helper which resolves image/file/text by MIME type in that priority order.
- **Background clipboard token:** adds a 1×1 px invisible `TYPE_APPLICATION_OVERLAY` window (`captureOverlayView`) so `getPrimaryClip()` returns non-null from background on API 29+ (the "ClipCascade" technique). Gated on `Settings.canDrawOverlays`; no-op if permission absent.
- **Restart on swipe-away:** `onTaskRemoved` schedules a one-time expedited WorkManager job (`ServiceRestartWorker`) — does NOT call `startForegroundService` directly to avoid `ForegroundServiceStartNotAllowedException` on API 31+.
- **Boot persistence:** `BootReceiver` handles `ACTION_BOOT_COMPLETED` and four OEM quick-boot actions (HTC, Xiaomi, generic) to restart the service after device reboot.
- **Limitations:** on some OEM ROMs (ColorOS, MIUI) the overlay token approach does not survive aggressive process killing; fallback is the logcat+overlay path (§2.3).

### 2.2 Foreground In-Activity Listener (`MainActivity.kt:84–107`)
- **Purpose:** Captures clipboard while `MainActivity` is the foreground window (all API levels), closing the gap where the FGS may not yet hold its overlay token.
- **Behaviour:** same `dispatchClipData` → `captureClip` / `captureImageClip` pipeline as the service; uses `lifecycleScope` (cancelled in `onDestroy`).
- **Dedup guard:** `ClipboardRepository.shouldSkipExpectedClip()` / `shouldSkipExpectedImageUri()` prevent re-capturing items the user just copied from history.

### 2.3 Background Logcat+Overlay Capture (`LogcatCaptureService.kt`, `ClipboardFloatingActivity.kt`)
- **Purpose:** Works around the Android 10+ clipboard restriction on OEMs where the FGS overlay token approach is insufficient.
- **Mechanism:** `LogcatCaptureService` tails logcat for the clipboard-access denial log line (which names the app package); on match it launches `ClipboardFloatingActivity`.
- **`ClipboardFloatingActivity`:** a transparent, recents-excluded `Activity` that adds a `TYPE_APPLICATION_OVERLAY` view, clears `FLAG_NOT_FOCUSABLE` to acquire input focus, and reads `getPrimaryClip()` inside a `OnGlobalLayoutListener` callback (the ONLY moment the restriction is lifted). Result routed through the shared capture pipeline.
- **Prerequisites:** `READ_LOGS` permission (granted via `adb shell pm grant`), `SYSTEM_ALERT_WINDOW`.
- **Settings UI:** General tab shows live status (WORKING / NOT_GRANTED / GRANTED_NOT_WORKING), toggle to disable, and three tap-to-copy ADB commands.
- **Limitations:** requires adb access to grant `READ_LOGS`; may not work if logcat output format differs across OEM Android forks; transparent overlay is invisible but does momentarily claim focus.

### 2.4 Sensitive-Content Suppression
- **At capture:** `isSensitive(text/uri)` UniFFI call (falls back to `false` if `.so` absent). Sensitive items are **not stored** (dropped silently at capture in `captureClip`, `captureImageClip`, `captureFileClip`).
- **At sync-in:** sensitive items arriving over cloud/P2P are stored but flagged; UI masks them.
- **Private mode:** `settings.privateMode == true` → all capture is suppressed regardless of content; toggled in Settings General tab.
- **Capture-pause:** notification Pause/Resume action toggles `settings.captureEnabled`; the service keeps its listener registered so resuming is instant.

### 2.5 Image Capture (`ClipboardService.captureImageClip`)
- Reads full-res Bitmap from the content:// URI via `contentResolver.openInputStream`.
- Re-encodes as PNG (lossless full-res) + generates a thumbnail via `ImageThumbnailUtils.generateThumbnail` (WebP lossy 80 on API 30+, PNG fallback; max ~680 px on the long edge).
- Stores full-res bytes under `item_<id>` and thumbnail under `item_thumb_<id>` in `ClipboardRepository`.
- Copy-back guard: `ClipboardRepository.expectImageUri(uri)` registered before `setPrimaryClip` so the capture listener recognises history-copy echoes.
- Limitations: image sensitivity check uses the URI string as a proxy (not pixel content); OOM caught and silently skips the item.

### 2.6 File Capture (`ClipboardService.captureFileClip`)
- Reads raw bytes via `contentResolver.openInputStream`; filename from `OpenableColumns.DISPLAY_NAME` (falls back to last URI path segment).
- Stores bytes + metadata (`storeFileBytes` / `storeFileMeta`); label in history is `[file: <name>]`.
- Cloud payload encoded with `SyncManager.encodeCloudFilePayload(name, mime, bytes)` so receiver recovers original name/MIME.
- Size gated by `ClipboardRepository.storeFileBytes` internal cap (configurable, default 8 MiB).

### 2.7 Share-Target (`ShareReceiverActivity.kt`)
- **Purpose:** Lets any app's share sheet send text/images/files directly into CopyPaste history.
- Registered for `ACTION_SEND` + `ACTION_SEND_MULTIPLE` over `*/*`.
- Routes `EXTRA_TEXT` → `captureClip`; image/* streams → `captureImageClip`; other streams → `captureFileClip`.
- `ACTION_SEND_MULTIPLE`: each URI captured independently; `finish()` called only after all IO coroutines drain.
- Invisible Activity (translucent, no-history, excluded from recents).

---

## 3. Clipboard History Screen (`HistoryActivity.kt`)

### 3.1 Item List
- **Composable:** `HistoryScreen` → `HistoryList` → `HistoryRow` (Jetpack Compose `LazyColumn`).
- **Sort order:** pinned items first (by `pinnedSortIndex`), then unpinned by `wallTimeMs` descending. De-dups by `id` before rendering to prevent LazyColumn `IllegalArgumentException` from duplicate keys.
- **Infinite scroll:** `derivedStateOf` triggers `viewModel.loadMore()` when within 10 items of the end; footer shows `CircularProgressIndicator` while loading; `hasMore` signal from `ClipboardRepository`.
- **Header count badge:** `totalCount` (from `ClipboardViewModel.totalCount`) shown as a rounded-rect badge in the top bar title; reads all stored IDs without decrypting.
- **Animations:** list-item entrance: `AnimatedVisibility` fade + slide-up per item with staggered delay (capped at 10× `Motion.Fast`); only plays once per unique id (tracked in `mountedIds`).
- **Row anatomy (left→right):** checkbox 16 dp | pin-badge (bookmark icon, amber) | content-type chip | preview text / image / file label | source-app icon+label | relative timestamp (tabular-nums) | origin-device badge | icon-action buttons (pin/delete or up/down in reorder mode).
- **Row height:** minimum 40 dp for text and file rows; image rows sized by `imageMaxHeightDp` setting (10–200 dp).
- **Press-scale:** `animateFloatAsState` 0.98 on press, out-expo spring; applied to row and action buttons.
- **Row background:** selection = `IdeSelection`; expanded = `IdeElevated`; sensitive = `IdeDanger@7%`; pinned = `IdeWarning@16%`; 2 dp amber left-accent bar when pinned (normal state).

### 3.2 Copy Action
- **Single-tap:** copies immediately (no explicit copy button required). In selection mode, taps toggle selection instead.
- **Text copy:** loads full plaintext via `ClipboardRepository.loadFullPlaintext` (decrypts full content, not just 140-char snippet).
- **Image copy:** writes full-res bytes to `cacheDir/image_copy/<id>.png`, exposes via `FileProvider`; broadcasts URI-read grant to all installed packages via `grantUriToAll` (broad grant fix for OEM clipboard managers).
- **File copy:** writes bytes to `cacheDir/file_copy/<name>`, exposes via `FileProvider`; same broad URI grant.
- **Echo guard:** `ClipboardRepository.expectClip` / `expectImageUri` registered before `setPrimaryClip` to suppress re-capture.
- **Recency bump:** `viewModel.copyItem(id)` bumps `wallTimeMs` so copied item moves to top of the unpinned section.
- **Sensitive items:** tap shows a snackbar hint ("masked") rather than copying.

### 3.3 Search
- **Toggle:** search icon in top bar; expands inline animated `AnimatedVisibility` full-width `TextField` (pushes list down via `innerPadding`, not a popup).
- **Instant snippet match:** `item.snippet.contains(query, ignoreCase)` applied synchronously on the sorted list.
- **Full-content async match:** `ClipboardRepository.searchIds` decrypts full text; debounced 250 ms; `LaunchedEffect(searchQuery, idListHash)` triggers re-scan when list changes while query is active. Results unioned with snippet matches once available.
- **Recent searches:** last 5 queries persisted in `Settings.recentSearches` (SharedPreferences); shown inline when field is empty; "Clear" button removes all; tapping re-fills the query.
- **Empty states:** dedicated hero-icon + title + subtitle composables for empty history and empty search results.

### 3.4 Device Filter
- **Strip:** horizontal `LazyRow` of chip buttons shown only when items from > 1 origin device are present (mirrors macOS).
- **Chips:** "All" first, then own device (accent-dim bg, accent text), then peers (elevated bg, dim text) — sorted own-first, then alphabetically.
- **Auto-reset:** filter resets to "all" when selected device disappears from the list (e.g. after clearing that device's items).
- **Origin-device badge:** per-row `OriginDeviceBadge` — "This device" (accent) for own items; peer display name (faint) for received items.

### 3.5 Content-Type Chips
- Per-row `ContentTypeChip` (9 sp semibold, 4 dp corner radius).
- **PRIVATE** (danger/red): `isSensitive == true`.
- **IMAGE** (violet): `contentType.startsWith("image/")`.
- **TEXT / URL / EMAIL / CODE** (accent, info, or violet): classified by `TextKind.classify(snippet)`.
- **FILE** (dim/elevated): `contentType == "file"`.
- **Too-large-to-sync badge:** `CloudOff` icon (warning amber 12 dp) when item exceeds `SYNC_MAX_BLOB_BYTES` (8 MiB).

### 3.6 Long-Press Preview Overlay (`PreviewOverlay.kt`)
- **Phases:** `Idle` → `Peeking` (long-press hold) → `Pinned` (drag up past threshold) → `Idle` (dismiss).
- **Trigger:** `previewPeekGesture` custom `Modifier` using `detectDragGesturesAfterLongPress`; no-op when `selectionMode == true`.
- **Content:** full-text `SelectionContainer` (selectable for copy); image rendered at `heightIn(max=340.dp)`; file shows name + SaveAlt button.
- **Actions (preview overlay):** Copy, Pin/Unpin, Delete, Save (file only).
- **Survives rotation:** `previewItemId` and `previewPhase` stored in `rememberSaveable`.
- **Auto-dismiss:** when previewed item is deleted or no longer in list.
- **Haptic feedback:** `HapticFeedbackType.LongPress` on peek start.

### 3.7 Selection Mode
- **Enter:** long-press on row (outside preview mode) or checkbox tap.
- **Top bar swap:** `SelectionTopBar` replaces normal top bar — shows selected count, select-all toggle (checkbox icon), pin, unpin, delete action buttons.
- **Bulk delete:** confirmation `AlertDialog` before deletion; count shown in message.
- **Bulk pin/unpin:** acts on all selected items, exits selection mode after.
- **Select-all toggle:** checks all when not all selected; unchecks all when all selected.
- **Survives rotation:** `selectedIds` persisted via `rememberSaveable` with `listSaver`.
- **Dedup:** selected ids intersected against live item list on each list change so stale selections do not show wrong counts.

### 3.8 Pin / Unpin
- **Per-row:** bookmark icon (filled=pinned amber / outlined=unpinned) — tap toggles.
- **Pinned items:** always appear first in list, sorted by `pinnedSortIndex` (copying a pinned item does NOT move it — `HW-A15` fix).
- **State sync:** `ClipboardViewModel.setPinned` → `ClipboardRepository.setPinned`.

### 3.9 Reorder Pinned Items
- **Activation:** `SwapVert` icon in top bar; visible only when ≥ 2 pinned items present.
- **Mechanism:** up/down arrow buttons replace pin/delete buttons on each pinned row; arrows dim when at first/last position.
- **Persistence:** `viewModel.reorderPinned(List<String>)` swaps `pinnedSortIndex` values in the repository.
- **Exit:** back gesture or tapping the reorder icon again.

### 3.10 Delete & Clear
- **Single delete:** trash icon on row; immediate, no confirmation.
- **Selected delete:** bulk confirmation dialog.
- **Clear unpinned:** overflow `⋮` menu → "Clear unpinned" → confirmation dialog. Pinned items are preserved.
- **Counter adjustment:** `ClipboardService.onItemsDeleted(context, count)` decrements the "captured today" counter and refreshes the notification.

### 3.11 Image Display & Caching
- **Two-level LRU:** `imageByteCache` (16 MiB, raw bytes) + `bitmapCache` (8 MiB, decoded `Bitmap`).
- **Decoding:** `BitmapFactory.Options.inSampleSize` proportional to `targetPx=340`; thumbnail preferred (`getDisplayImageBytes`), full-res fallback.
- **App-icon LRU:** `appIconBitmapCache` (2 MiB) for source-app icons decoded from `AppIconHelper.getAppIconBase64`.
- **Cache eviction:** `evictImageCaches(id)` called on item delete.

### 3.12 File Save
- **In-list:** `SaveAlt` icon on file rows → writes bytes to `MediaStore.Downloads` (API 29+, no `WRITE_EXTERNAL_STORAGE` needed); snackbar on success/failure.
- **From preview overlay:** same path.

### 3.13 In-App File Picker
- **Source:** `AttachFile` icon in history top bar opens `ACTION_OPEN_DOCUMENT` (`*/*`).
- **Behaviour:** selected file routed through `ClipboardService.captureFileClip`, snackbar confirms capture, list refreshes.

---

## 4. Devices Screen (`DevicesActivity.kt`)

### 4.1 This-Device Card (`OwnDeviceCard`)
- Shows model, OS version, app version, local LAN IP (refreshed every ~5 s via `remember(nowMs / 5000)`), fingerprint (truncated to 16+8).
- Always-online dot (green).

### 4.2 Own QR Code (Devices screen inline, `OwnQrSection`)
- Pairing QR shown at the top of the Devices list without navigating away; 200 dp, 512 px bitmap.
- **Blurred by default** (16 dp blur + "Tap to reveal" overlay); first tap reveals; second tap regenerates.
- **Auto-refresh:** 2-minute TTL countdown (same as `PairActivity`); auto-regenerates on expiry and stays visible.
- **Countdown:** label turns danger-red below 15 s.

### 4.3 Paired Peer Cards (`PeerCard`)
- Two-column metadata table: fingerprint (truncated), model, OS, app version, local IP, public IP, sync address, paired-at timestamp, last-sync relative time.
- **Online dot:** green when `lastSyncMs` within 60 s (ONLINE_WINDOW_MS) OR when peer's IP appears in live mDNS discovery set (IP-correlation via `discoveredIps`).
- **Actions:** Unpair (forgets peer locally, confirmation dialog) and Revoke (forgets + writes `revokeDeviceAudit` record to encrypted DB, confirmation dialog with sync-key rotation warning).
- Online/offline state computed once per second in DevicesScreen and threaded to all cards + `SyncStatusBadge` footer (single source of truth via `DevicesOnlineState`).

### 4.4 Discovered (Unpaired) LAN Peers (`DiscoveredPeerCard`)
- Shown only when P2P sync is enabled.
- Refreshed every 2 s via `listDiscovered` UniFFI call; paired devices filtered out by IP-correlation.
- **Pair button:** disabled when `bport == null` (v1 peer without SAS bootstrap port) or another pairing is in flight.
- On tap: calls `pairWithDiscovered` (initiator role) then opens `SasPairingDialog`.

### 4.5 Direct QR Scan (Devices screen)
- "Scan QR" `OutlinedButton` opens ZXing camera (`PortraitCaptureActivity`, portrait-locked).
- Scan result forwarded to `PairActivity` via `cppair://pair?p=<payload>` deep link so the full PAKE + provisioning flow runs there.

### 4.6 SAS Pairing Dialog (`SasPairingDialog`)
- **Opened from:** discovery-initiated pair, OR auto-opened on screen entry when `pairGetSas()` returns `awaiting_sas` (triggered by tapping the incoming-pair notification or on first composition if already in that state).
- **States polled every 1 s:** `initiating` (spinner), `awaiting_sas` (6-digit SAS code shown + Match / Doesn't Match buttons), `awaiting_sas` without code ("Waiting for the other device…"), `confirmed`, `rejected`, `aborted`, `timed_out`, trailing `idle`.
- **On confirmed:** KEK-wraps session key, upserts peer roster entry (fingerprint, sync address, device metadata), applies fill-missing provisioning (supabaseUrl, supabaseAnonKey, relayUrl, derivedSyncKey).
- **On close before terminal:** calls `pairAbort`.
- **Security:** SAS code never logged; session-key bytes zeroized after wrapping.

---

## 5. Pair Activity (`PairActivity.kt`)

### 5.1 Display Own QR
- Auto-generates `CPPAIR1.*` pairing QR on first composition; 240 dp, 512 px bitmap. `FLAG_SECURE` on this window.
- **Blurred by default**; first tap reveals; second tap regenerates and stays visible (HW-A5).
- 2-minute TTL countdown, auto-refresh on expiry.
- Parallel QR-display and scan-ready state.

### 5.2 Scan Peer QR
- "Scan QR" `OutlinedButton` → ZXing `ScanContract` → `PortraitCaptureActivity`.
- Parsed via UniFFI `parsePairing` → shows `ScannedPairing` confirmation card (device name, address, fingerprint).
- **"Pair & sync" button:** runs `bootstrapPairInitiator` (PAKE exchange), syncs items immediately, persists `PairedPeer` with KEK-wrapped session key and device metadata.
- **Deep-link support:** `cppair://pair?p=<CPPAIR1.…>` intent (from Google Lens or DevicesActivity scan) processed in `onNewIntent`; feeds the same `parsePairing` path.
- **Post-pair sync:** bidirectional item exchange at pairing time; item-type routing by `contentType` (text/image/file); skip-reason counters logged (`itemsSkippedLegacy`, `itemsSkippedDecryptFail`, etc.).
- **Provisioning fill-missing:** Supabase URL, anon key, relay URL, derived sync key applied only if not already configured locally.

---

## 6. Sync Engine

### 6.1 Foreground-Service Sync Loop (`FgsSyncLoop.kt`)
- **Supabase catch-up poll:** compound `(wall_time, id)` cursor; self-echo skipped; LWW replace on item_id when incoming `lamportTs` is strictly newer.
- **Poll intervals:** WS connected → 120 s; WS disconnected → 60 s; ≥3 consecutive empty polls → 300 s idle.
- **Exponential backoff on failure:** 30 s base, doubles per consecutive failure, cap 8 min.
- **Drain loop:** re-polls immediately when batch is full (≥ `POLL_LIMIT`) to catch backlogs; breaks on short batch.
- **Auto-apply newest text:** after full drain, applies the highest-`wallTime` text clip to the system clipboard once (prevents per-item clipboard spam).
- **P2P dial cadence:** 3-second fixed interval (`P2P_DIAL_INTERVAL_MS`) independent of poll interval; sleeps poll interval in 3-s chunks, dialing each chunk.
- **Per-peer mDNS IP-correlation:** before and after each dial attempt, `resolveAddrByIp` refreshes stale port from live `listDiscovered` by matching LAN IP host.
- **Denylist:** `listRevokedFingerprints` loaded per dial pass; revoked peers skipped.
- **`storeSyncedItem`:** shared by dialer and inbound listener; routes image/file/text by content type; applies inbound tombstone (ABI 15); applies inbound `pinned` state.

### 6.2 Supabase Realtime WebSocket (`SupabaseRealtimeClient.kt`)
- **Protocol:** `wss://<project>.supabase.co/realtime/v1/websocket?apikey=<anon>&vsn=1.0.0`.
- **Topic:** `realtime:clipboard_items`; PostgreSQL changes filter `user_id=eq.<UUID>`.
- **Frame format:** 5-element JSON array `[join_ref, ref, topic, event, payload]`.
- **Heartbeat:** every 30 s.
- **Reconnect:** exponential backoff 1 s → 60 s + jitter on `phx_error` / `phx_close` / disconnect.
- **`isConnected` flag:** read by `FgsSyncLoop.pollIntervalMs` to switch catch-up poll intervals.
- **Auto-apply:** `onSyncedTextClip` callback for WS-pushed text clips (applied immediately, not batched).
- **Security:** access token and payload content never logged.
- **Lifetime:** started in `ClipboardService.onStartCommand`, closed gracefully in `onDestroy`.

### 6.3 Supabase Poll WorkManager (`SupabasePollWorker.kt`)
- Fallback 15-min periodic worker when the FGS is not running (battery-optimised devices, process death). Configured by `SupabasePollWorker.schedule(ctx, enabled)` called from Settings save and boot receiver.

### 6.4 Relay SSE Subscription (`RelaySubscriptionClient.kt`)
- Third independent transport (alongside P2P and Supabase).
- Server-Sent Events stream; `(wall_time, id)` cursor persisted in `Settings.lastRelaySubscribeWallTime/Id`.
- Gated on `settings.relayUrl` being configured and non-localhost.
- Owned by `ClipboardService`; reconnects on `onFailure`.

### 6.5 P2P mTLS Inbound Listener (`ClipboardService.startInboundP2pListener`)
- Binds an OS-assigned port via UniFFI `startP2pListener`; port published in `ClipboardService.activeListenerPort`.
- Drain loop runs every `P2P_DIAL_INTERVAL_MS` (3 s); stores received items via `FgsSyncLoop.storeSyncedItem`.
- Peer roster / denylist / session keys refreshed each tick.
- **mDNS discovery:** `startDiscovery` advertises this device (model, OS, app version, LAN IP, sync port, bootstrap port) for the lifetime of the FGS (`ClipboardService.startFgsDiscovery`). Stopped with `stopDiscovery` on service destroy.
- **Incoming-pair responder poll:** `pairResponderPollJob` polls `pairGetSas` every 1 s; when `state == awaiting_sas && role == responder`, posts a HIGH-importance notification (`NOTIF_ID_PAIR_REQUEST`) with a "Confirm" action that deep-links to `DevicesActivity`.

### 6.6 Sync Constraints
- **Wi-Fi only:** `settings.syncOnWifiOnly`; checked in `FgsSyncLoop` before dial.
- **Sync backend switch:** `SyncBackend.SUPABASE` or `SyncBackend.RELAY` (exclusive); relay upload path currently marked as no-op in `notifySyncManager` for local-capture (Supabase is the active cloud path).

---

## 7. Settings Screen (`SettingsActivity.kt`)

Settings are organised into five scrollable tabs. All edits buffer in Compose state and are written only on "Save" (single synchronous `commit()` to survive force-kill).

### 7.1 General Tab
| Setting | Type | Notes |
|---------|------|-------|
| Private mode | Toggle | Suppresses all new captures on this device; stored as `settings.privateMode` |
| Sync enabled | Toggle | Master switch for all cloud/P2P sync |
| Discover public IP | Toggle | Allows one-off STUN to learn public IP for device-info card |
| Paste as plain text | Toggle | Strips RTF/HTML on paste (parity with macOS) |
| Permissions | Nav row | Opens `PermissionsSettingsActivity` |
| Devices | Nav row | Opens `DevicesActivity` |
| Log Viewer | Nav row | Opens `LogViewerActivity` |
| Export Logs | Button | `LogExportHelper.shareLogsZip(ctx)` — shares ZIP of persistent app logs |
| Background capture (ADB) section | Status + toggle + 3 tap-to-copy ADB commands | Live `LogcatCaptureStatus` badge; toggle enables/disables logcat capture |
| About | Nav row | Opens `AboutActivity` |

### 7.2 Display Tab
| Setting | Type | Range |
|---------|------|-------|
| Show sensitive warnings | Toggle | Highlights sensitive items in history |
| Mask sensitive content | Toggle | Replaces sensitive snippet with `•••` mask string |
| Translucency | Toggle | UI translucency effect |
| Image max height | Continuous slider | 10–200 dp |
| Preview / auto-close delay | Continuous slider | 200–30 000 ms |
| Image quality | Continuous slider | 1–100 % |

### 7.3 Storage Tab
| Setting | Type | Steps |
|---------|------|-------|
| Max text clip size | Stepped slider | Steps from `TEXT_SIZE_STEP_VALUES` |
| Max image size | Stepped slider | Steps from `IMAGE_SIZE_STEP_VALUES` |
| Max file size | Stepped slider | 8 / 16 / 25 / 50 / 100 MiB |
| Storage quota | Stepped slider | Steps from `QUOTA_STEP_VALUES` |
| Sensitive auto-clear TTL | Stepped slider | Off / 10 s / 30 s / 1 min / 5 min / 15 min / 1 h |
| Excluded apps | Editable chip list | Package IDs; text input + Add button + removable chips |
| Background capture setup | Nav row | Opens `BackgroundCaptureSetupActivity` |

### 7.4 Sync Tab
| Setting | Type | Notes |
|---------|------|-------|
| P2P sync | Toggle | LAN direct mTLS sync |
| Wi-Fi only | Toggle | Gates all sync on Wi-Fi connectivity |
| Use Supabase | Toggle | Switches `syncBackend` between `SUPABASE` and `RELAY` |
| Supabase URL | Text field | Shown only when `syncBackend == SUPABASE` |
| Supabase anon key | Password field | |
| Sync passphrase | Password field | Cloud AEAD key derivation |
| Supabase email | Text field | Optional sign-in |
| Supabase password | Password field | |
| Relay URL | Text field | Shown only when `syncBackend == RELAY` |

### 7.5 Notifications Tab
| Setting | Type |
|---------|------|
| Notify on copy | Toggle — IMPORTANCE_MIN silent badge, 2 s auto-dismiss, debounced 500 ms |
| Sound on copy | Toggle — `AudioManager.playSoundEffect(CLICK)` |

### 7.6 Unsaved-Changes Guard
- `isDirty` (`derivedStateOf`) tracks all 27 fields against a `SettingsSnapshot`.
- Back-press and navbar tab switches intercepted; `AlertDialog` offers Discard / Keep editing.
- `onRegisterNavGuard` callback wires the guard to `MainShell`'s navbar.

---

## 8. Notifications

| Channel | Importance | Purpose |
|---------|-----------|---------|
| `copypaste_service` | LOW (no sound, no heads-up) | Persistent FGS notification: title "Active"/"Paused"; body "N items captured today" / "Capture paused…"; actions: Pause/Resume, Open |
| `copypaste_copy_event` | MIN (badge only) | Per-copy event toast; auto-cancels after 2 s; debounced 500 ms |
| `copypaste_pair_request` | HIGH (sound + heads-up) | Incoming P2P pairing request; "Confirm" action deep-links to DevicesActivity with `EXTRA_AUTO_OPEN_SAS=true` |
| `copypaste_sensitive` | HIGH | Sensitive-data detected (from `NotificationHelper`) |
| `copypaste_sync` | LOW | Relay sync status (from `NotificationHelper`) |

The persistent FGS notification shows the captured-today count; `Pause/Resume` broadcasts to `CaptureControlReceiver` which toggles `settings.captureEnabled` and refreshes the notification in-place.

---

## 9. Storage & Retention (`ClipboardRepository.kt`)

- **Backend:** `SharedPreferences` (`copypaste_items`); items stored as pipe-delimited blobs under `item_<uuid>`; ordered index under `item_ids` (comma-separated, newest-first after reversal).
- **Encryption:** UniFFI `encryptText`/`decryptText` (XChaCha20-Poly1305 via Rust core); AES-256-GCM Android KeyStore fallback when `.so` unavailable.
- **Pagination:** pinned items always returned first; unpinned items paged (`PAGE_SIZE`); `getItems(key, limit, offset)`.
- **Retention:** byte-only quota (`settings.storageQuotaBytes`); oldest unpinned items pruned until under quota. Pinned items exempt from eviction. No item-count cap.
- **Sensitive TTL:** `pruneByAge` runs on each `getItems` call; items older than `settings.sensitiveTtlSecs` and flagged sensitive are wiped.
- **Dedup window:** process-wide in-memory guard in companion object; identical content within `DEDUP_WINDOW_MS` stored only once (prevents duplicate rows from concurrent `ClipboardManager` listeners).
- **LWW:** `storeItemWithLww` for cloud/P2P items; replaces local row only when `incomingLamportTs` is strictly newer.
- **Image/file attachment:** separate `item_img_<id>`, `item_thumb_<id>`, `item_file_<id>`, `item_filemeta_<id>` keys.
- **Full-text search:** `searchIds` decrypts all items and filters by substring; debounced in the UI.

---

## 10. Diagnostics (`AppLogger.kt`, `LogViewerActivity.kt`, `LogExportHelper.kt`, `LogcatCaptureService.kt`)

- **Persistent crash log:** `AppLogger` writes to a rolling log file in app-private storage (accessible via adb pull); `CrashHandler` installs an `UncaughtExceptionHandler` to persist crash stacks.
- **Log Viewer:** `LogViewerActivity` — in-app scrollable view of the persistent log.
- **Log export:** `LogExportHelper.shareLogsZip(ctx)` — zips the log file and shares via Android intent.
- `LogcatCaptureService` also reads its own status via `LogcatCaptureStatus` (WORKING / NOT_GRANTED / GRANTED_NOT_WORKING / DISABLED).

---

## 11. App Icon / Alternate Icons (`AppIconHelper.kt`)

- Retrieves the launcher icon of any installed package as a Base64 PNG for display in the history source-app chip.
- No alternate-icon switching functionality in `AppIconHelper`; the class is a read-only helper.

---

## Notable Gaps Observed

1. **Relay upload path disabled:** `notifySyncManager` in `ClipboardService` only pushes to Supabase; the `SyncBackend.RELAY` branch calls `syncManager.pushToRelay(…)` but the calling path from `captureClip` was commented "no-op" in earlier versions and the relay SSE receive path is only wired as a subscriber. The relay bearer token is never persisted (`Settings` has no `relayToken` from `registerDevice` flow — `MainActivity.kt:138`).

2. **macOS→Android P2P receives items but Android→macOS (dialer) uses wall_time not lamport_ts as the P2P LWW basis** because `SyncedItem` from the frozen P2P ABI carries no `lamportTs` field; `storeSyncedItem` uses `wallTimeMs` as a proxy.

3. **File items from Supabase poll lose name/MIME:** `FgsSyncLoop.poll()` stores file rows with `storeFileBytes` + `storeFileMeta(null, null)` because `DecryptedItem` from the Supabase SELECT does not carry `file_name`/`mime` columns — filename shows as `[file]` not `[file: report.pdf]` for cloud-received files.

4. **No image-sync in Supabase realtime WS path:** `SupabaseRealtimeClient` only processes text clips from the WS push; image/file rows arriving via WS are ignored and caught up by the poll loop.

5. **`SyncBackend.RELAY` Supabase-account config displayed incorrectly:** the Sync tab shows a hardcoded warning "All your devices must use THIS SAME Supabase account" even when `syncBackend == RELAY` (the account section is nested inside the `if (syncBackend == SUPABASE)` block, so it does not show — but the warning text inside that block conflates relay and Supabase).

6. **No push-notification channel for sync failures:** there is no user-visible indication when sync is consistently failing (only silent log entries); the persistent FGS notification does not surface sync errors.

7. **OwnDeviceCard shows no public IP:** public IP (`peerPublicIp`) is absent from the own-device card because the STUN request is never performed locally at display time; the card only shows local LAN IP.

8. **Clipboard history limited to `SharedPreferences` backend:** no SQL-based full-text search or indexed query; `searchIds` decrypts the full plaintext of every item per search call (O(n) decryption), which may be slow with hundreds of items.

9. **No delete/pin/reorder sync operations (tombstones) over cloud/relay:** these operations are local-only; a pin or delete on Android does not propagate to macOS or other devices until ABI 15 op-propagation (partially wired in `storeSyncedItem` for inbound tombstones, but no outbound path for local operations).

10. **`FLAG_SECURE` absent on `DevicesActivity`:** the own-QR section on the Devices screen is blurred by default, but the window lacks `FLAG_SECURE`, so a screen-recording app could capture the QR after the user taps to reveal.
