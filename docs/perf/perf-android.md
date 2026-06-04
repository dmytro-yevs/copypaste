# Android Performance Audit — CopyPaste v0.6.1

**Scope:** `android/app/src/main/java/com/copypaste/android/`  
**Date:** 2026-06-04  
**Branch:** feat/android-parity-v0.5.3  
**Auditor:** read-only static analysis (no builds, no instrumentation)

---

## 1. List / UI Jank

### 1.1 `parseCache` is an unbounded `HashMap` — no eviction ceiling
**File:** `ClipboardRepository.kt:1555`  
**Impact: High** | **Effort: S**

```kotlin
private val parseCache = HashMap<String, ParsedEntry>()
```

The cache holds `(rawBlob, ClipboardItem)` per item and is process-wide (companion object). It grows monotonically — every item ever stored accumulates here until the process dies, a `clearAll`, or an explicit `evictParseCache`. At 1 000 items with an average 300-char blob each entry is ~700 bytes; at default `storageQuotaBytes` (10 GiB cap, though unlikely to reach) there is no practical ceiling on this map. A long-running process with many stored items will accumulate hundreds of KB here that GC can never collect. Replace with an `LruCache<String, ParsedEntry>` sized to a reasonable multiple of `PAGE_SIZE` (e.g., 200 entries).

---

### 1.2 `unpinnedItemCount()` and `totalItemCount()` each call `storedIds()` — O(N) comma-split on every page boundary
**Files:** `ClipboardRepository.kt:195–198, 827` / `ClipboardViewModel.kt:110, 147`

```kotlin
fun unpinnedItemCount(): Int {
    val pinnedSet = storedPinnedIds()          // prefs read + split
    return storedIds().count { it !in pinnedSet }  // another prefs read + split
}
fun totalItemCount(): Int = storedIds().size   // yet another prefs read + split
```

`storedIds()` reads `item_ids` from SharedPreferences and splits the comma-joined string every call. `ClipboardViewModel.loadItems()` and `loadMore()` each call **both** of these after the `getItems` call (lines 109–110, 146–147), so every page load does 4–6 separate comma-split passes over a potentially multi-KB string. With 500 items the `item_ids` blob is ~18 KB; each `split(",")` allocates a fresh `List<String>`. Cache the count in-process (invalidated on write) or, cheaper, cache the split list inside `getItems` and pass it to the counting functions.

**Impact: Medium** | **Effort: S**

---

### 1.3 `relativeTime()` called unconditionally every recomposition with `System.currentTimeMillis()`
**File:** `HistoryActivity.kt:213–222` (called inline at lines 1990, 2098, 2242)

`relativeTime(item.wallTimeMs)` computes `System.currentTimeMillis() - ms` on each frame. With 50 visible rows and 60 Hz this is 3 000 `currentTimeMillis()` calls/second even when the list is idle. `ClipboardItem` is `@Immutable` so Compose's skip logic should prevent redundant recompositions, but the value itself changes once per minute — meaning all rows with a "Nm ago" stamp recompose simultaneously every minute. Wrap in a `derivedStateOf` that updates on a 30s timer, or use `remember(item.wallTimeMs, tickMinute)` keyed on a minute-granularity clock tick.

**Impact: Medium** | **Effort: S**

---

### 1.4 `ScaleIconButton` spawns a `MutableInteractionSource` + `animateFloatAsState` per action button per row
**File:** `HistoryActivity.kt:2426–2447`

Each `ScaleIconButton` allocates: one `MutableInteractionSource`, one `collectIsPressedAsState()` flow subscription, and one `animateFloatAsState` coroutine. A text row has 2 action buttons, an image row has 2, a file row has 3. With 50 visible rows, this is up to 150 `MutableInteractionSource` allocations and 150 active animation coroutines. The 0.98→1.0 scale range is also trivially indistinguishable from pressing. Either move the press-scale to the parent row's existing `interactionSource` (already tracked at the row level at line 1845) or use a simpler `pointerInput` approach with no animation.

**Impact: Medium** | **Effort: M**

---

### 1.5 `produceState` for app-icon bitmap inside the text row branch — allocated per recomposition
**File:** `HistoryActivity.kt:2183–2207`

```kotlin
val iconBitmap by produceState<ImageBitmap?>(initialValue = null, key1 = item.sourceApp) { ... }
```

This `produceState` is defined **inside the `sourceAppLabel(...)?.let` block**, which means the composable scope for it can shift on recomposition if `item.sourceApp` becomes null or non-null. Compose will create and tear down the coroutine on those transitions. More importantly, this is a nested `@Composable`-style call inside a `let` lambda in what is not a directly annotated `@Composable` function — it relies on the outer composable's scope. If the icon is already in `appIconBitmapCache` (LRU), the coroutine still launches and immediately reads from the cache, leaving a small but redundant coroutine per visible text row per composition. Extract the icon loading into a stable side-effect or a dedicated `@Composable` helper.

**Impact: Low** | **Effort: S**

---

### 1.6 `AnimatedVisibility` with `slideInVertically` on every initial item mount — staggered by `index * Motion.Fast`
**File:** `HistoryActivity.kt:1706–1722`

```kotlin
val mountDelay = if (isNewMount) (index * Motion.Fast).coerceAtMost(10 * Motion.Fast) else 0
AnimatedVisibility(visible = true, enter = fadeIn(...) + slideInVertically(...)) { ... }
```

On first load of a 50-item page, items 0–9 each start a staggered `slideInVertically` animation. All 10 animations run concurrently on the main thread via Compose's animation system. This is intentional for polish but the 10-item cap (`coerceAtMost(10 * Motion.Fast)`) means the 10th item still has a 150ms+ delay on screen while animating up. On older mid-range devices this can cause dropped frames. Consider reducing the stagger cap to 5 items or removing the slide component (keep only `fadeIn`) on devices with `Build.VERSION.SDK_INT < 31` or with reduced-motion accessibility enabled.

**Impact: Low** | **Effort: S**

---

### 1.7 `PreviewOverlay` loads full-res image bytes from SharedPreferences into a `produceState` on BOTH `Peeking` and `Pinned` phase transitions
**File:** `PreviewOverlay.kt:248–272`

```kotlin
val fullBitmapState by produceState(..., key1 = item.id, key2 = phase) {
    if (item.isImage && phase != PreviewPhase.Idle) {
        val bytes = repository.getImageBytes(item.id) ...  // full-res SP read
```

The `key2 = phase` causes the effect to re-run on **every** phase transition (Idle→Peeking AND Peeking→Pinned). On the Peeking→Pinned transition the full-res bytes are loaded **again** even though they were loaded during Peeking. Additionally, `getImageBytes` reads the raw Base64 string from SharedPreferences and decodes it each call — no caching layer (unlike `cachedThumbnailBitmap`). For a 4 MB image this is a 4 MB Base64 decode on the UI-thread-adjacent IO dispatcher every time the preview transitions. Key the effect only on `item.id` (not phase) and accept `null` → decoded-bitmap latency on first show.

**Impact: Medium** | **Effort: S**

---

## 2. Storage (SharedPreferences as Clipboard Store)

### 2.1 `item_ids` is a comma-joined string of all item UUIDs — O(N) on every write
**File:** `ClipboardRepository.kt:1101–1106, 668–675`

Every store, delete, prune, and bumpToTop operation reads the full `item_ids` CSV string, splits it into a list, mutates it, rejoins, and writes it back. At 1 000 items (each UUID = 36 chars) the string is ~37 KB. Each `joinToString(",")` in a tight loop (e.g. bulk delete of 100 items) allocates a fresh 37 KB string. This pattern also means `storedIds()` deserializes the entire index for O(1) lookup operations. For the default PAGE_SIZE=50 on a 500-item store, a single `getItems()` call deserializes 500 UUIDs, filters them, and never caches the result.

At 10 000 items (aggressive user, pinned items + long history) the string approaches 370 KB and each mutation round-trips 740 KB (read + write). SharedPreferences XML-serialises its values; the XML overhead is additional.

**Recommendation:** Replace the CSV index with either (a) an ordered `StringSet` in SharedPreferences (no join/split overhead, O(1) presence tests via set membership) or (b) migrate to a Room/SQLite store for O(log N) index operations.

**Impact: High** | **Effort: L**

---

### 2.2 `pruneByAge()` called on every `getItems()` — synchronous O(N) scan inside the IO dispatcher
**File:** `ClipboardRepository.kt:133, 973`

```kotlin
pruneByAge(key)  // called at the top of every getItems()
```

`pruneByAge` acquires `idsWriteLock`, reads ALL item blobs from SharedPreferences (one `prefs.getString` per item), parses the wallTime from each, and conditionally decrypts+classifies items older than the sensitive TTL. At 500 items this is 500 SharedPreferences reads inside a synchronized block. The general-TTL fast path (wallTime comparison only, no decrypt) is cheap per item but still scans all items. This runs on every `getItems()` call — on scroll events that trigger `loadMore()`, on the ViewModel's `storeListener` debounce (300ms after any prefs write), and on every page load.

**Recommendation:** Run `pruneByAge` on a timer or once per FGS tick, not on every read. A sticky `lastPruneAtMs` watermark (persisted in SharedPreferences) prevents double-runs within the same minute.

**Impact: High** | **Effort: S**

---

### 2.3 `pruneToLimits()` O(N²) inner loop: `blobSizes.values.sumOf{}` recalculated after **each individual eviction**
**File:** `ClipboardRepository.kt:894–948`

```kotlin
val blobSizes: Map<String, Int> = ids.associate { id ->
    // 4 SharedPreferences reads per item
    val textBytes = prefs.getString("item_$id", null)?.toByteArray(Charsets.UTF_8)?.size ?: 0
    val imgBytes = ...
    val thumbBytes = ...
    val fileBytes = ...
    id to (textBytes + imgBytes + thumbBytes + fileBytes)
}
var totalBytes = blobSizes.values.sumOf { it.toLong() }
...
while (...) {
    val evictId = unpinned.removeAt(0)
    totalBytes -= sz   // ← this part is fine — O(1) per eviction
```

The `blobSizes` map construction reads **4 SharedPreferences keys per item** (text blob, image bytes, thumbnail bytes, file bytes). At 500 items with images this is 2 000 SharedPreferences reads in a single synchronized block on every store. This runs after every `storeItem` call. For a capture storm (rapid paste) every capture triggers a full N×4 SharedPreferences read pass.

**Recommendation:** Track total byte usage in a single persisted counter (updated atomically on each write/delete). `pruneToLimits` reads the counter, evicts oldest-first only while it exceeds quota, and decrements counter per eviction — reducing this from O(N×4 reads) to O(1 read + K deletes).

**Impact: High** | **Effort: M**

---

### 2.4 Binary payloads stored as Base64 in SharedPreferences — 1.33× storage inflation and mandatory full-decode on every read
**Files:** `ClipboardRepository.kt:206–210, 220–227, 257–264`

Image bytes (`item_img_<id>`), thumbnail bytes (`item_thumb_<id>`), and file bytes (`item_file_<id>`) are stored as Base64 NO_WRAP strings. A 1 MB PNG becomes a 1.37 MB string in SharedPreferences. Every `getImageBytes()` / `getThumbnailBytes()` call allocates a full `ByteArray` decode via `Base64.decode`. The LRU `imageByteCache` mitigates repeated decodes for visible rows, but the first access and every cache-miss requires a full Base64 decode into a fresh allocation.

SharedPreferences is not designed for binary blobs. Values are persisted to an XML file and loaded into RAM on first access; storing multiple 1–8 MB Base64 strings means the entire `copypaste_items.xml` file is parsed and held in memory.

**Recommendation:** Write binary items to files in `context.getDir("clipboard_blobs", MODE_PRIVATE)` using a content-addressed filename (`<id>_img`, `<id>_thumb`, `<id>_file`). The SharedPreferences entry stores only the filename/path. This eliminates the Base64 overhead, keeps the SharedPreferences XML small, and allows mmap-backed file reads.

**Impact: High** | **Effort: L**

---

### 2.5 `storedSourceIds()` parses the full `synced_source_ids` CSV (up to 1 000 entries) on every sync-inbound item
**File:** `ClipboardRepository.kt:1130–1136`

```kotlin
private fun storedSourceIds(): LinkedHashSet<String> =
    LinkedHashSet(
        prefs.getString(KEY_SYNCED_SOURCE_IDS, "")?.split(",")?.filter { it.isNotBlank() } ?: emptyList()
    )
```

This is called under `seenSourceIdsLock` for every row in every Supabase poll batch. A batch of 20 rows parses the 1 000-entry CSV 20 times, allocating 20 `LinkedHashSet` instances. Cache the seen-set in memory (process-local `@Volatile` in companion object), invalidated on `clearAll`/`clearUnpinned`.

**Impact: Medium** | **Effort: S**

---

## 3. Battery / Background

### 3.1 P2P dialer fires every 3 seconds unconditionally — 20 wakeups/minute in the FGS
**File:** `FgsSyncLoop.kt:140`

```kotlin
const val P2P_DIAL_INTERVAL_MS = 3_000L
```

The P2P dial loop runs at 3 s cadence regardless of whether the peer is currently reachable. Each iteration: reads `settings.pairedPeers` (SharedPreferences JSON parse), calls `listDiscovered()` (native FFI), reads `settings.encryptionKey` (possibly an AndroidKeyStore unwrap), and calls the native `syncWithPeer()` FFI. At 20 wakeups/minute this keeps the CPU busy even when the device is on the other side of the world from its peer. The 3 s interval was chosen for "link establishment speed" but after a successful sync there is no reason to retry for several minutes.

**Recommendation:** Implement adaptive dial cadence: 3 s until first successful connection in the current session, then 30 s during active use, then 5 min when idle (matching the Supabase idle interval). Gate on `NetworkCapabilities.hasTransport(TRANSPORT_WIFI)` as a prerequisite.

**Impact: High** | **Effort: M**

---

### 3.2 `pairResponderPoller` fires `pairGetSas()` (native FFI) every 1 second for the FGS lifetime
**File:** `ClipboardService.kt:679`

```kotlin
private const val PAIR_RESPONDER_POLL_MS = 1_000L
```

`startPairResponderPoller()` runs forever in the FGS scope, calling `pairGetSas()` via Rust FFI at 1 Hz. This is 3 600 native FFI calls per hour even when the user has already paired and is not actively scanning a QR code. The poll is needed to detect incoming pair requests, but 1 Hz is aggressive. Increase to 5 s; pair QR codes are scanned deliberately and a 5 s notification delay is imperceptible.

**Impact: Medium** | **Effort: S**

---

### 3.3 Three independent sync transports all active simultaneously — triple keep-alive overhead
**File:** `ClipboardService.kt:120–128, 241–245`

The FGS runs three transport clients concurrently:
1. `SupabaseRealtimeClient` — WebSocket with 30 s heartbeat + OkHttp 25 s ping interval  
2. `RelaySubscriptionClient` — SSE connection with its own reconnect loop  
3. `FgsSyncLoop` / `P2pDialerGate` — P2P mTLS dial every 3 s

All three are active whenever `syncEnabled` is true regardless of whether the user is on cellular data, in Doze mode, or has not synced anything for days. At minimum the WS heartbeat (30 s) and the P2P dial (3 s) together prevent the radio from entering its deep sleep state.

**Recommendation:** Gate Supabase WS and relay on `syncEnabled && syncBackend == SUPABASE` (relay is a no-op when Supabase is the backend, but the SSE connection still starts). Pause P2P dials when `NetworkCapabilities` reports cellular or metered network (unless user opts in).

**Impact: Medium** | **Effort: M**

---

### 3.4 `SupabasePollWorker` at 15-minute WorkManager cadence duplicates the FGS poll — double-poll overhead
**File:** `SupabasePollWorker.kt:194, 237–238`

```kotlin
private const val POLL_INTERVAL_MINUTES = 15L
val request = PeriodicWorkRequestBuilder<SupabasePollWorker>(POLL_INTERVAL_MINUTES, ...)
```

When the FGS is alive (normal operation), the Supabase poll loop runs every 60–300 s. The WorkManager 15-min periodic worker is designed as a fallback when the FGS is killed, but it runs regardless. Because WorkManager de-duplicates by unique work name (`scheduleOnce`/`REPLACE`), the worker fires even while the FGS is active, causing a redundant poll. The LWW dedup in `storeItemWithLww` makes this safe but wasteful. The worker should check if the FGS is running before issuing a network request.

**Impact: Low** | **Effort: S**

---

### 3.5 WebSocket heartbeat (30 s) + OkHttp TCP ping (25 s) — two independent keepalive timers per connection
**File:** `SupabaseRealtimeClient.kt:74–77`

```kotlin
private const val HEARTBEAT_INTERVAL_MS = 30_000L
private const val PING_INTERVAL_S = 25L
```

The OkHttp-level ping (`pingIntervalSeconds`) fires every 25 s to keep the TCP connection alive, and the Phoenix-protocol heartbeat fires every 30 s to keep the Supabase channel joined. These overlap and both prevent radio sleep every ~25 s. If the OkHttp ping is kept, the Phoenix heartbeat period can be extended to match — or the OkHttp ping can be removed (relying only on the Phoenix heartbeat which the server already accepts as a keep-alive signal).

**Impact: Low** | **Effort: S**

---

## 4. Memory

### 4.1 `grantUriToAll()` enumerates ALL installed packages on every image/file copy-back
**File:** `HistoryActivity.kt:239–258`

```kotlin
val packages = pm.getInstalledPackages(0)
for (pkg in packages) {
    ctx.grantUriPermission(pkg.packageName, uri, FLAG_GRANT_READ_URI_PERMISSION)
}
```

`getInstalledPackages(0)` returns a `List<PackageInfo>` for every installed app — typically 100–300 items on a mid-range device. The list is allocated, iterated, and discarded on every image copy. With `FLAG_GRANT_READ_URI_PERMISSION` this is also called from the PreviewOverlay copy path (line 1127). Each `grantUriPermission` call crosses an IPC boundary to ActivityManagerService. This function is O(installed-packages) on a hot copy path.

**Recommendation:** Grant only to the currently-focused IME package and `com.android.systemui`, falling back to a targeted intent query for apps declaring `android.intent.action.PASTE`. The broad-grant comment acknowledges OEM variance but `getInstalledPackages(0)` is far too broad.

**Impact: Medium** | **Effort: M**

---

### 4.2 `parseCache` is a `HashMap` — no size bound, stores rawBlob string for every item
**File:** `ClipboardRepository.kt:1545, 1555` (see also §1.1)

Each `ParsedEntry` holds both the raw pipe-delimited blob string (including Base64-encoded ciphertext, typically 100–500 bytes for text items) and the decoded `ClipboardItem`. At 1 000 items the cache holds ~1 000 `ParsedEntry` objects totalling ~500 KB–1 MB of strings, unbounded. This is in addition to the `ClipboardItem` objects held by the ViewModel's `items: LiveData<List<ClipboardItem>>`.

---

### 4.3 Full-res image stored in SharedPreferences XML AND decoded into `imageByteCache` simultaneously
**File:** `ClipboardRepository.kt:304` / `HistoryActivity.kt:274`

A captured 4 MB PNG is stored Base64-encoded (~5.3 MB string) in the SharedPreferences XML. When loaded, the Base64 string is decoded to a 4 MB `ByteArray` held in `imageByteCache` (16 MiB cap). Then `cachedThumbnailBitmap` allocates a `Bitmap` (e.g. 340×240px ARGB_8888 = ~326 KB) held in `bitmapCache` (8 MiB cap). So for a single image the heap holds: the Base64 XML string in SharedPreferences RAM (~5.3 MB), the decoded ByteArray in `imageByteCache` (4 MB), and the decoded Bitmap in `bitmapCache` (~326 KB) — potentially >9 MB per image. With 5 images visible, this can approach 45 MB heap pressure from images alone.

**Impact: High** | **Effort: L** (requires migrating binary blobs off SharedPreferences, see §2.4)

---

## 5. Capture Overhead

### 5.1 `dispatchClipData` calls `isSensitive(uriStr)` for image and file URIs — native FFI call on every capture
**File:** `ClipboardService.kt:912–916, 1067–1069`

```kotlin
val uriStr = uri.toString()
val sensitive = try { isSensitive(uriStr) } catch (_: UnsatisfiedLinkError) { false }
```

Calling the native `isSensitive()` with a `content://` URI string (e.g. `content://media/external/images/media/42`) will never produce a true sensitivity verdict — the URI is not plaintext content. This is a safe-by-design check but wastes a Rust FFI call and JNI boundary crossing on every image/file capture. Skip `isSensitive` for URI strings; only run it for actual text content.

**Impact: Low** | **Effort: S**

---

### 5.2 Full PNG re-encode at capture time — `BitmapFactory.decodeStream` → `bitmap.compress(PNG, 100)` allocates 2× the raw bitmap in RAM
**File:** `ClipboardService.kt:921–951`

The image capture path:
1. Decodes the source stream into a full-res `Bitmap` (ARGB_8888, uncompressed)
2. Re-encodes to PNG at quality=100 into a `ByteArrayOutputStream`
3. Then calls `generateThumbnail` which scales the same Bitmap

At peak (during step 2), RAM holds: the decoded Bitmap (`W×H×4` bytes) plus the growing `ByteArrayOutputStream` backing array (up to `W×H×4` worst case for large PNG). For a 4K screenshot this is ~32 MB of transient RAM in a single `suspend fun`. The PNG re-encode is necessary for stable storage, but the `ByteArrayOutputStream` grows to 4× its contents on resize; pre-size it with `ByteArrayOutputStream(bitmap.allocationByteCount)` to halve the peak allocation.

**Impact: Low** | **Effort: S**

---

## Top 10 Performance Wins — Ranked by Impact/Effort Ratio

| Rank | Finding | File(s) | Impact | Effort | Gain |
|------|---------|---------|--------|--------|------|
| 1 | **Move `pruneByAge` off the `getItems()` hot path** — run at most once per minute via watermark | `ClipboardRepository.kt:133` | High | S | Eliminates O(N) SP scan on every scroll/load |
| 2 | **Track byte quota with a persisted counter** instead of O(N×4 SP reads) in `pruneToLimits` | `ClipboardRepository.kt:894` | High | M | Cuts 2 000 SP reads per capture to ~4 |
| 3 | **Adaptive P2P dial cadence** — 3 s initial, 30 s active, 5 min idle | `FgsSyncLoop.kt:140` | High | M | Cuts from 1 200 dials/hour to ~12 at idle |
| 4 | **Cap `parseCache` with `LruCache`** (e.g. 200 entries) | `ClipboardRepository.kt:1555` | High | S | Bounds memory; prevents unbounded growth |
| 5 | **Store binary blobs as files, not Base64 SP strings** | `ClipboardRepository.kt:248, 306, 271` | High | L | Eliminates 1.33× inflation + XML RAM load |
| 6 | **Cache `storedIds()` split result in-process** — invalidate on write | `ClipboardRepository.kt:1101` | Medium | S | Removes repeated 37 KB string allocations |
| 7 | **Cache `storedSourceIds()` in a `@Volatile` process-global set** | `ClipboardRepository.kt:1130` | Medium | S | Eliminates 1 000-entry CSV parse per inbound row |
| 8 | **Increase `pairResponderPoller` cadence to 5 s** | `ClipboardService.kt:679` | Medium | S | Cuts FFI poll from 3 600/h to 720/h |
| 9 | **Replace `grantUriToAll` (all packages) with targeted grant** | `HistoryActivity.kt:239` | Medium | M | Removes O(installed-apps) IPC on copy |
| 10 | **Remove `pruneByAge` call from `getItems`, replace `relativeTime` with minute-tick `derivedStateOf`** | `ClipboardRepository.kt:133`, `HistoryActivity.kt:213` | Medium | S | Eliminates 3 000 `currentTimeMillis` calls/sec + O(N) scan on scroll |

---

## Appendix: Intervals Reference

| Timer | Cadence | Source |
|-------|---------|--------|
| FgsSyncLoop Supabase poll (WS up) | 120 s | `FgsSyncLoop.kt:104` |
| FgsSyncLoop Supabase poll (WS down) | 60 s | `FgsSyncLoop.kt:111` |
| FgsSyncLoop Supabase poll (idle) | 300 s | `FgsSyncLoop.kt:118` |
| P2P dialer | 3 s | `FgsSyncLoop.kt:140` |
| P2P inbound listener drain | 3 s | `ClipboardService.kt:419` |
| Pair responder SAS poller | 1 s | `ClipboardService.kt:679` |
| WS heartbeat (Phoenix) | 30 s | `SupabaseRealtimeClient.kt:74` |
| WS TCP ping (OkHttp) | 25 s | `SupabaseRealtimeClient.kt:77` |
| WorkManager fallback poll | 15 min | `SupabasePollWorker.kt:194` |
| ViewModel store debounce | 300 ms | `ClipboardViewModel.kt:272` |
| Image byte cache ceiling | 16 MiB | `HistoryActivity.kt:272` |
| Bitmap cache ceiling | 8 MiB | `HistoryActivity.kt:287` |
| App icon cache ceiling | 2 MiB | `HistoryActivity.kt:359` |
| Parse cache | unbounded HashMap | `ClipboardRepository.kt:1555` |
| Seen-source-ids cap | 1 000 entries | `ClipboardRepository.kt:1347` |
