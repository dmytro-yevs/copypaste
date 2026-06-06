# Android App Code Review — v0.6.1-integration

**Reviewer:** Senior Android / Kotlin engineer (read-only)
**Scope:** `android/app/src/main/java/com/copypaste/android/`
**Branch:** `feat/android-parity-v0.5.3` (review baseline)
**Date:** 2026-06-04

---

## 1. Code Duplication

### DUP-1 — Inline store-dispatch block copied verbatim to three callers (P1)

`SupabasePollWorker.doWork` (lines 104–147), `FgsSyncLoop.poll` (lines 366–438), and `SupabaseRealtimeClient.ingestWsRow` (lines 525–591) all contain the same three-branch `if (isImage) … else if (isFile) … else` dispatch that calls `repository.storeItem("[image]")`, `repository.storeImageBytes`, `SyncThumbnailHelper.generateAndStore`, `repository.storeFileBytes`, and `repository.storeItemWithLww`. The function `storeDecryptedItem` at `SupabasePollWorker.kt:276` was written to unify exactly this pattern, but the inline copies were not updated to use it.

Three concrete divergences visible right now:

- `SupabasePollWorker.doWork` **has no file branch** (lines 104–147) — `isFile` is never tested; file payloads fall into the `else` branch and are UTF-8-decoded as garbage text. The `storeDecryptedItem` helper below it (line 276) does include a file branch but is never called from `doWork`.
- `SupabasePollWorker.doWork` image path does **not generate a thumbnail** (lines 122–126) — `SyncThumbnailHelper.generateAndStore` is absent; the `storeDecryptedItem` helper and `FgsSyncLoop.poll` both call it.
- `SupabaseRealtimeClient.ingestWsRow` uses **inline** `val isImage = item.contentType == "image" || item.contentType.startsWith("image/")` (line 525) instead of the canonical `contentTypeIsImage()` helper from `ContentType.kt:23`.

**Recommendation:** Make `SupabasePollWorker.doWork` delegate entirely to `storeDecryptedItem`, then inline that helper into `FgsSyncLoop.storeSyncedItem` (P2P) with the same signature, and have `SupabaseRealtimeClient.ingestWsRow` call the same helper. Delete the three inline copies.

---

### DUP-2 — `clearAll` and `clearUnpinned` are byte-for-byte identical (P2)

`ClipboardRepository.clearAll` (lines 414–448) and `clearUnpinned` (lines 454–484) share **all** body logic: iterate pinned set, remove unpinned items with the same six `editor.remove(…)` calls, update `KEY_ITEM_IDS`, remove `KEY_SYNCED_SOURCE_IDS`, reset dedup state, evict parse cache, call `ClipboardService.onItemsDeleted`. The only observable difference is the log message. The methods are separate at the ViewModel layer for semantic reasons, but the shared body should be a private `clearUnpinnedInternal()` called by both.

---

### DUP-3 — `saveFile` MediaStore block duplicated across two lambdas in `HistoryActivity` (P2)

The MediaStore download flow (`ContentValues` construction, `resolver.insert`, `resolver.openOutputStream`, `IS_PENDING` flip, `resolver.update`) appears in full at `HistoryActivity.kt:1044–1075` (the `onSaveFile` lambda of `HistoryList`) and again at `HistoryActivity.kt:1176–1201` (the `onSaveFile` lambda of `PreviewOverlay`). A private top-level `suspend fun saveFileToDownloads(context, fileBytes, fileName, mime): Boolean` would eliminate both copies.

---

### DUP-4 — Image copy-back URI write duplicated in `copyItemById` and `PreviewOverlay.onCopy` (P2)

`HistoryActivity.kt:1568–1607` (inside `copyItemById`) and `HistoryActivity.kt:1113–1129` (inside `PreviewOverlay.onCopy`) both perform: decode image bytes → write to `cacheDir/image_copy/<id>.png` → `FileProvider.getUriForFile` → `grantUriToAll` / hardcoded `"com.android.systemui"` grant → `expectImageUri` → `setPrimaryClip`. The file copy-back path is similarly duplicated at lines 1610–1644 and 1132–1149. Extract a private `suspend fun copyBinaryItemToClipboard(context, repository, item, cm)`.

---

### DUP-5 — Backoff and reconnect logic copy-pasted between `SupabaseRealtimeClient` and `RelaySubscriptionClient` (P3)

`SupabaseRealtimeClient.reconnectDelayMs` (line 225) and `RelaySubscriptionClient.reconnectDelayMs` (line 77) are identical implementations of `1s * 2^(attempt-1)` with ±20% jitter clamped to 60 s. Same constants, same formula. Extract to a shared `fun exponentialBackoffMs(attempt: Int, base: Long, max: Long): Long` in a common file.

---

## 2. Dead / Unused Code

### DEAD-1 — `storeDecryptedItem` is never called (P1)

`SupabasePollWorker.kt:276–366` defines `internal suspend fun storeDecryptedItem(…)`. A search of the entire package finds exactly **zero call sites** — only its own definition. `SupabasePollWorker.doWork` does not call it; `FgsSyncLoop` does not call it; nothing calls it. The function was written to unify the three inline copies but was never wired. It should either be wired (and the inline copies deleted, fixing DUP-1) or deleted if the P2P path already supersedes it.

---

### DEAD-2 — `NotificationHelper.notifySensitiveDetected` is never called (P2)

`NotificationHelper.kt:44` defines `fun notifySensitiveDetected(context, id)`. A grep of the entire package shows zero call sites outside the class itself. The sensitive-content path in `ClipboardService.captureClip` (line 813) silently returns without notifying. Either wire it or delete it.

---

### DEAD-3 — `NotificationHelper.createChannels` registers two channels that no code uses (P2)

`NotificationHelper.createChannels` (line 18, called from `CopyPasteApp.onCreate:28`) registers `"copypaste_sensitive"` and `"copypaste_sync"`. No builder in the codebase uses either channel ID. The four live channels (`CHANNEL_ID`, `CHANNEL_COPY_EVENT`, `CHANNEL_PAIR_REQUEST`, created by `ClipboardService.ensureChannel`) are separate. `NotificationHelper.createChannels` (and `NotificationHelper` itself, since `notifySensitiveDetected` is also unused) is vestigial scaffolding.

---

### DEAD-4 — `ClipboardRepository.syncItems` is explicitly dead (P3)

`ClipboardRepository.kt:1806` has `@Deprecated(level = ERROR)` on `syncItems`. The function body throws `UnsupportedOperationException`. The `@Suppress("UnusedParameter")` annotation acknowledges it. No caller exists. Delete it.

---

### DEAD-5 — `SupabasePollWorker` hardcodes `SyncBackend.SUPABASE` gating but is scheduled regardless of `syncBackend` (P3)

`SupabasePollWorker.schedule(context, enabled)` is called from `SupabasePollWorker.syncWithSettings`, which checks `settings.syncBackend == SyncBackend.SUPABASE`. However the worker checks `settings.syncBackend != SyncBackend.SUPABASE` **inside** `doWork` and early-returns. If `syncBackend` is changed to RELAY after the worker is scheduled, WorkManager will continue to fire it every 15 minutes; it will no-op but wastes a wake-up. Minor, but documented as a latent resource waste.

---

## 3. Competing / Duplicate State

### STATE-1 — Notification ID collision: `ClipboardService.NOTIFICATION_ID = 1001` vs `NotificationHelper.notifySensitiveDetected(id = 1001)` (P1)

`ClipboardService.kt:656`: `const val NOTIFICATION_ID = 1001` — the live persistent FGS notification.
`NotificationHelper.kt:44`: `fun notifySensitiveDetected(context, id: Int = 1001)` — a sensitive-data alert.

If `notifySensitiveDetected` were ever called with the default ID, it would silently **overwrite the FGS persistent notification**. On Android 8+ the system would interpret the new notification as an update to the service notification and the FGS styling/buttons would be replaced by the alert. Assign `NOTIF_ID_SENSITIVE = 1005` (distinct from 1001/1003/1004).

---

### STATE-2 — `SupabaseRealtimeClient.ingestWsRow` uses **inline** content-type predicates instead of canonical helpers (P1)

`SupabaseRealtimeClient.kt:525`:
```kotlin
val isImage = item.contentType == "image" || item.contentType.startsWith("image/")
val isFile = item.contentType == "file"
```
`ContentType.kt:23` defines `contentTypeIsImage` / `contentTypeIsFile` for exactly this purpose. The WS path is the only receive path not using them. If the canonical definition ever gains a new type (e.g. `"image/webp"` short form), the WS path silently diverges.

---

### STATE-3 — Blob field-index arithmetic scattered, not centralised (P1)

The v4 blob format `wallTimeMs|contentType|payloadBytes|nonceB64|ciphertextB64|lamportTs|deleted|originDeviceId` is parsed by at least six separate `parts[N]` reads:

- `ClipboardRepository.bumpToTop:559` — `parts.size < 6` guard
- `ClipboardRepository.deleteItem:335` — `parts[5].toLongOrNull()` for lamport
- `ClipboardRepository.isDeletedBlob:1211` — `parts[6] == "1"`
- `ClipboardRepository.parseItem:1221` — `parts[0..4,7]`
- `ClipboardRepository.storedLamportTs:1292` — `parts[5]`
- `ClipboardRepository.applyInboundTombstoneWithLww:1765` — `parts[5]`
- `ClipboardRepository.localItemsForSync:1664` — `parts[0,1,3,4]`

No `BlobFields` object/constants (`FIELD_WALL_TIME = 0`, `FIELD_LAMPORT_TS = 5`, etc.) exist. If a field is ever inserted or reordered, all sites must be updated manually. A `parse()` / `encode()` pair with named field constants is the minimal fix.

---

### STATE-4 — Three `ClipboardRepository` instances are created inside `HistoryScreen` / `HistoryList` composables, bypassing the ViewModel (P2)

`HistoryActivity.kt:403` (file-picker lambda, recreated on every recomposition entry), `HistoryActivity.kt:537` (`searchRepository = remember { ClipboardRepository(ctx) }`), `HistoryActivity.kt:1098` (`previewRepository = remember { … }`), `HistoryActivity.kt:1176` (preview `onSaveFile` lambda — not memoised, recreated on each recomposition call).

`ClipboardViewModel` already owns `repository` (line 31). Passing it down to composables (or exposing required operations through the ViewModel) avoids multiple SharedPreferences handles, multiple parse-cache lookups through the same singleton map, and confusion about which instance called `observe`.

---

### STATE-5 — `SupabasePollWorker` shares the same `(lastSupabasePollWallTime, lastSupabasePollId)` cursor as `FgsSyncLoop` and `SupabaseRealtimeClient` without a write-level lock (P2)

All three independently read and write `settings.lastSupabasePollWallTime` and `settings.lastSupabasePollId` via `SharedPreferences.apply()` (asynchronous). When two of them race on the same batch (e.g. WS reconnect triggers a catch-up poll at the same moment the 15-minute WorkManager tick fires), one write may be lost. SharedPreferences writes to the same file are serialised by the Android framework's `QueuedWork` thread, so a lost write here can leave the cursor pointing at an old watermark and cause re-delivery of a batch of items. The LWW `storeItemWithLww` guard suppresses duplicates, but the cursor is still stale until the next successful write.

---

## 4. Weird / Buggy Behaviour

### BUG-1 — `runBlocking` on the FGS main thread in `startInboundP2pListener` (P0)

`ClipboardService.kt:345`:
```kotlin
val localItems = runBlocking {
    repository.localItemsForSync(key)
}
```
`startInboundP2pListener` is called from `onStartCommand`, which runs on the **main thread** (the Service lifecycle is main-thread-bound). `runBlocking` on the main thread suspends the main thread for the duration of `localItemsForSync`, which is an `IO`-dispatched function that reads and decrypts every locally stored clipboard item. On a large history (hundreds of encrypted items) this is easily tens to hundreds of milliseconds — a classic ANR hazard. The correct fix is to launch a coroutine on `scope` and call `localItemsForSync` inside it before calling `startP2pListener`.

---

### BUG-2 — `SupabasePollWorker.doWork` has no file branch: file payloads corrupted (P0)

Confirmed from DUP-1: `SupabasePollWorker.doWork` (lines 104–147) tests `isImage` but never tests `isFile`. A `contentType == "file"` row falls into the `else` branch at line 130, which calls `item.plaintext.toString(Charsets.UTF_8)` on **raw binary file bytes**. The result is garbled text stored as a text clip. `storeDecryptedItem` at line 276 (which would have handled it correctly) is never called. WorkManager fires every 15 minutes; every file synced via Supabase is silently corrupted on that path.

---

### BUG-3 — Coroutine scope leak in `ClipboardFloatingActivity.cleanupAndFinish` (P1)

`ClipboardFloatingActivity.kt:231–257`: after `cleanupAndFinish` calls `finish()`, the Activity's `onDestroy` at line 266 calls `scope.cancel()`. However, `cleanupAndFinish` itself deliberately does **not** cancel the scope (comment: "Do NOT cancel scope here: launched capture coroutines must drain"). This means: (a) If `cleanupAndFinish` is called but `onDestroy` is never reached (process death, system kill before destroy), the scope and all running coroutines are abandoned. (b) More critically, if the process is killed while the child coroutines have not yet completed their SharedPreferences writes, the write is lost silently. The comment's reasoning ("coroutines are typically < 50 ms") is a race. The correct approach is to use `scope.coroutineContext[Job]?.join()` or `runBlocking { scope.coroutineContext[Job]?.children?.forEach { it.join() } }` before `finish()`.

---

### BUG-4 — `expectClip` / `shouldSkipExpectedClip` are not single-shot: a 5-second window blocks re-copy (P1)

`ClipboardRepository.kt:1476`: `shouldSkipExpectedClip` explicitly does NOT clear `expectedClipHasValue` on a match. This means that if the user copies the same text twice within 5 seconds (e.g. rapid re-copy after a mistake), the **second genuine copy** is silently dropped. The comment justifies this to allow multiple concurrent listeners to suppress one echo, but a correct design would use a counter or per-listener latch rather than a flat boolean window that suppresses legitimate user actions.

Additionally, the same risk exists for `shouldSkipExpectedImageUri` — though image re-copy within 5 s is less common.

---

### BUG-5 — `grantUriPermission("com.android.systemui", …)` hardcoded in `PreviewOverlay.onCopy` (P1)

`HistoryActivity.kt:1127` and `1147`: the `PreviewOverlay.onCopy` path grants the URI only to `"com.android.systemui"`. The same issue was already fixed on the `HistoryList.copyItemById` path (line 1599 uses `grantUriToAll`). The overlay copy path was not updated, so image and file copy-back from the preview card fails on OEM devices where the pasting app is not SystemUI (Samsung clipboard, Gboard, etc.).

---

### BUG-6 — `SupabaseRealtimeClient.triggerCatchUpPoll` fetches but does not store (P1)

`SupabaseRealtimeClient.kt:603–619`: `triggerCatchUpPoll` calls `syncManager.pollFromSupabase(…)` and obtains a batch, then logs its size and returns. The rows in the batch are **never stored** — no `storeDecryptedItem` / `storeItemWithLww` call. The intent was to trigger the FgsSyncLoop to do a catch-up poll, but instead it just issues a network fetch and discards the result. The correct fix is to let `FgsSyncLoop.poll()` handle it (e.g. expose a `triggerImmediatePoll()` method on `FgsSyncLoop`) instead of duplicating the fetch.

---

### BUG-7 — `HistoryList` creates a new `Settings` instance on every recomposition (P2)

`HistoryActivity.kt:1537`: `val settings = remember { Settings(ctx) }`. `remember` without a key means this is only created once per composition, which is fine. However, `HistoryScreen` at line 386 also does `val settings = remember { Settings(ctx) }`. There are now **two** `Settings` instances (plus the ViewModel's), and both compute `settings.encryptionKey` independently. Each `Settings.encryptionKey` call opens the `KeyStore` / derives the key; having multiple independent instances is unnecessary overhead but not strictly a bug.

---

## 5. Architecture Smells

### ARCH-1 — `ClipboardService.Companion` is a 600-line God-object (P1)

`ClipboardService.kt` companion object spans lines 654–1527 — roughly 870 lines. It contains:
- `dispatchClipData` — MIME routing
- `captureClip` — full capture pipeline with native insert, dedup, store, push
- `captureImageClip` — ~140 lines with bitmap decode + thumbnail generation
- `captureFileClip` — ~120 lines with cursor query + byte read
- `notifySyncManager` — dual-backend push routing
- `postCopyNotification`, `playCopySound`, `bumpTodayCounter`, `refreshNotification`, `buildNotification`, `ensureChannel`, `postIncomingPairNotification`

These are static business-logic functions with no instance state. They live in the companion because they need to be callable from `ClipboardFloatingActivity` and `LogcatCaptureService` as well. The correct model is a standalone `ClipboardCaptureManager` (or similar) injected into all three callers, with `ClipboardService` as a thin orchestrator.

---

### ARCH-2 — `HistoryActivity.kt` is 2472 lines — a Compose God-screen (P1)

The file hosts: `HistoryActivity` (entry point), `HistoryScreen` (~600 lines of state), `HistoryList` (~300 lines), `HistoryRow` (~400 lines, not fully read), plus 20+ private composables, LRU caches, URI grant helpers, and file-save logic. Compose encourages splitting, but this file violates the rule that a single file should contain one primary abstraction. Minimum split: `HistoryScreen.kt`, `HistoryRow.kt`, `HistoryImageCache.kt`, and `HistoryFileSave.kt`.

---

### ARCH-3 — Business logic in `HistoryScreen` composable, bypassing ViewModel (P2)

`HistoryScreen` contains: full-content search logic (debounced `LaunchedEffect` with `searchRepository.searchIds`), file download to MediaStore, file picker capture via `ClipboardService.captureFileClip`, and the copy-back pipeline (image/file/text branching with FileProvider). None of these belong in a `@Composable`. They should be `viewModel.searchFull(query)`, `viewModel.saveFile(id)`, `viewModel.captureFileFromUri(uri)`, etc. The composable should observe state, not own IO.

---

### ARCH-4 — Three concurrent sync-receive paths with no coordination token (P2)

`FgsSyncLoop` (catch-up poll), `SupabaseRealtimeClient` (WS push), and `RelaySubscriptionClient` (SSE push) run concurrently inside the FGS scope. All three advance the **same** `(lastSupabasePollWallTime, lastSupabasePollId)` cursor, but:
- The WS client advances the cursor per-item from a WebSocketListener callback (background OkHttp thread).
- The poll loop advances the cursor at the end of a drain batch (IO coroutine).
- The relay client has its own cursor (relay-specific), but calls `storeItemWithLww` with the same item IDs.

No mutex or `AtomicReference` guards the cursor. The LWW `storeItemWithLww` correctly suppresses duplicate item storage, but cursor staleness means future polls may re-fetch already-processed rows. Coordinate with a `Mutex` or a single `actor` coroutine.

---

### ARCH-5 — `ClipboardRepository` exceeds 1800 lines and violates SRP (P2)

`ClipboardRepository.kt` handles: AEAD encryption/decryption, soft-delete tombstones, LWW semantics, pagination, parse caching, cross-listener dedup state, size/TTL pruning, image/file/thumbnail byte storage, P2P/cloud sync item preparation, and inbound tombstone application. The companion object alone is ~300 lines of static crypto + dedup guards. The rule of thumb is that a repository should transform between storage format and domain model; everything else (encryption, dedup, pruning policy) should be in dedicated classes.

---

## Top 10 Issues to Fix First

| Rank | ID | Severity | Summary |
|------|----|----------|---------|
| 1 | BUG-1 | P0 | `runBlocking` on FGS main thread (`startInboundP2pListener`) — ANR risk on large histories |
| 2 | BUG-2 | P0 | `SupabasePollWorker.doWork` has no file branch — file payloads UTF-8-corrupted on 15-min WorkManager path |
| 3 | STATE-1 | P1 | Notification ID 1001 shared by FGS and `NotificationHelper.notifySensitiveDetected` — FGS notification silently overwritten |
| 4 | DUP-1 / DEAD-1 | P1 | `storeDecryptedItem` is never called; three inline store-dispatch copies diverge (missing file branch + thumbnail in `SupabasePollWorker`, inline type checks in `SupabaseRealtimeClient`) |
| 5 | BUG-6 | P1 | `triggerCatchUpPoll` fetches rows but never stores them — WS reconnect catch-up is a no-op |
| 6 | BUG-5 | P1 | `grantUriPermission("com.android.systemui")` hardcoded in preview `onCopy` — copy-back broken on OEM devices |
| 7 | STATE-2 | P1 | `SupabaseRealtimeClient.ingestWsRow` uses inline content-type checks instead of canonical helpers — will silently diverge on future type additions |
| 8 | BUG-4 | P1 | `shouldSkipExpectedClip` window not single-shot — genuine re-copy within 5 s is silently dropped |
| 9 | BUG-3 | P1 | `ClipboardFloatingActivity` scope not joined before `finish()` — capture writes may be lost on system kill |
| 10 | STATE-3 | P1 | Blob field indices (`parts[5]`, `parts[6]`, `parts[7]`) scattered without named constants — silent break on any future field insertion |

---

## Summary (6 bullets)

- **DUP:** The three-path sync store dispatch (`FgsSyncLoop`, `SupabasePollWorker`, `SupabaseRealtimeClient`) is copy-pasted; `SupabasePollWorker.doWork` is missing the file branch, and `SupabaseRealtimeClient` skips the canonical `contentTypeIsImage()` helper.
- **DEAD:** `storeDecryptedItem` (the intended unifier), `NotificationHelper.notifySensitiveDetected`, and `ClipboardRepository.syncItems` are all unreachable dead code; the first two leave real functionality gaps.
- **STATE:** Notification ID 1001 is shared between the live FGS notification and the unused sensitive-alert helper, and blob field indices are raw integer literals scattered across six call sites with no constants.
- **BUG:** `runBlocking` on the FGS main thread (ANR risk), missing file branch in `SupabasePollWorker` (data corruption), and `triggerCatchUpPoll` discards the batch it fetches (WS reconnect catch-up is a silent no-op).
- **ARCH:** `ClipboardService.Companion` (~870 lines) and `HistoryActivity.kt` (2472 lines) are God-objects; business logic in composables (`HistoryScreen` owns file I/O, file-picker capture, full-content search) should move into the ViewModel.
- **RACE:** Three concurrent receive paths (FgsSyncLoop + WS + relay) share the poll cursor without a write-level lock; the `expectClip` window is not single-shot, silently dropping re-copy within 5 s.
