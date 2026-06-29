package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.util.Base64
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.UUID

/**
 * Persists clipboard items to SharedPreferences.
 *
 * Each item is stored as a pipe-delimited blob under key "item_<uuid>" so it
 * survives process death without requiring Room or a .so binary.
 * An ordered index of ids is kept under "item_ids" (comma-separated).
 *
 * Encryption is performed via UniFFI [encryptText] (XChaCha20-Poly1305, ADR-001).
 * On [UnsatisfiedLinkError] or [IllegalStateException] (native library absent),
 * the store operation FAILS rather than falling back to [localAesEncrypt]
 * (AES-256-GCM): the fallback produced items that peers and the daemon could not
 * decrypt, causing silent sync failures. A one-shot sentinel notification is posted
 * instead so the user knows encryption is unavailable. [localAesDecrypt] is kept
 * for reading any legacy AES-GCM items that were stored before this fix.
 *
 * ## Retention & quota enforcement
 *
 * items until the total stored payload bytes are within [Settings.storageQuotaBytes].
 * There is NO count cap — only a size/byte cap (mirrors the macOS desktop policy).
 *
 * PINNED items (tracked in [KEY_PINNED_IDS]) are never evicted by the prune pass
 * and have no TTL. They survive until the user explicitly clears them via
 * [clearAll] (which deletes everything) or [deleteItem] / [deleteItems].
 *
 * ## Sensitive items
 *
 * Sensitive items are STORED (not dropped) at capture time in [storeItem] and
 * on sync-in in [storeItemWithLww], matching the macOS daemon. The sensitivity
 * verdict is recomputed at read time by [parseItem] and surfaced via
 * [ClipboardItem.isSensitive], which drives the masked preview / PRIVATE chip in
 * the history UI. Sensitive items are still subject to the sensitive-TTL
 * auto-wipe pass in [pruneByAge].
 */
class ClipboardRepository(context: Context) {

    /**
     * Application context retained so the delete path can keep the
     * foreground-service notification counter honest (see [deleteItem]). Using
     * the application context avoids leaking an Activity.
     */
    internal val appContext: Context = context.applicationContext

    internal val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    /** Read fresh each store so a UI change to the cap takes effect immediately. */
    internal val settings = Settings(context)

    /**
     * Guard for read-modify-write on the comma-joined "item_ids" index.
     * SharedPreferences is process-wide, so without this lock two coroutines
     * (UI delete + service insert) can both read the same baseline list and
     * the loser's update silently drops the winner's entry. See HIGH-8.
     */
    internal val idsWriteLock = Any()

    /**
     * In-memory dedup window. Multiple OnPrimaryClipChangedListener owners
     * (ClipboardService, LogcatCaptureService, MainActivity) each fire
     * on the same copy, so without this guard one copy creates 2-3 duplicate
     * rows (HIGH-3). We skip a store when an identical-content item was stored
     * within [DEDUP_WINDOW_MS]. The time window preserves the legitimate
     * "same text copied again later" case — re-copying after the window stores
     * a fresh row as expected.
     *
     * The dedup state ([lastStoredKey], [lastStoredAtMs], [dedupLock]) lives in
     * the [companion object] so it is shared process-wide across every
     * repository instance. All three listener owners run in the same process and
     * each builds its own [ClipboardRepository]; per-instance state let the same
     * physical copy slip past three independent guards, producing dup×3 rows,
     * notifications and sync pushes.
     */

    /**
     * Guard for read-modify-write on the comma-joined "synced_source_ids" set
     * (LOW-2). Both Supabase poll callers can run concurrently (FGS loop +
     * WorkManager worker), so the seen-set must be mutated under a lock to avoid
     * a lost update that would let a duplicate row through.
     */
    internal val seenSourceIdsLock = Any()

    /**
     * Set to true the first time a native-library failure is posted as a
     * user-visible notification so we don't flood the notification shade on
     * every store call. Reset on app restart (in-memory only).
     *
     * SECURITY: the native-unavailable path must never silently downgrade to
     * AES-GCM (which produces items peers cannot decrypt). Instead we throw so
     * the item is not stored and post this sentinel notification once.
     */
    @Volatile internal var nativeUnavailableNotified = false

    /**
     * Subscribe to changes in the backing item store. Any write from the
     * foreground service, the accessibility service, or another in-process
     * writer mutates the shared "copypaste_items" prefs and fires [listener].
     */
    fun observe(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ): SharedPreferences.OnSharedPreferenceChangeListener {
        prefs.registerOnSharedPreferenceChangeListener(listener)
        return listener
    }

    fun stopObserving(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        prefs.unregisterOnSharedPreferenceChangeListener(listener)
    }

    /**
     * Load history items for display, most-recent-first, with lazy pagination.
     *
     * Each stored blob is DECRYPTED with [key] so the row shows a real preview.
     * The [ClipboardItem.pinned] field is populated from the persisted [KEY_PINNED_IDS] set.
     * Image bytes are NOT attached here — callers must use [getImageBytes] lazily per-row.
     *
     * ## Pagination
     *
     * Pinned items are ALWAYS returned regardless of [offset] — they always float to
     * the top of the history list. Unpinned items are paged: skip the [offset] most-
     * recent unpinned items and return the next [limit] of them. There is NO hard
     * item-count ceiling on the display — callers append pages as the user scrolls.
     *
     * @param key    AEAD decryption key.
     * @param limit  max number of UNPINNED items to return for this page.
     * @param offset number of UNPINNED items to skip before this page (0 = first page).
     */
    suspend fun getItems(
        key: ByteArray,
        limit: Int = PAGE_SIZE,
        offset: Int = 0,
    ): List<ClipboardItem> =
        withContext(Dispatchers.IO) {
            // AB-13: run the retention TTL auto-wipe on the same cadence as load
            // (cheap general-age fast-path; sensitive pass only decrypts aged rows).
            pruneByAge(key)

            // PG-19 (o0t3 / osxa): use lamport-ordered history from the FFI when the
            // native .so is loaded. getHistoryPage returns pinned items first (by
            // pin_order), then unpinned by lamport_ts DESC so causal ordering is
            // correct across devices (immune to wall-clock skew). Falls back to the
            // wall-time ORDER BY from the SharedPreferences index when the feature is
            // off (stub mode / android-uniffi-live not compiled in), as determined by
            // an empty return from getHistoryPage.
            val lamportOrderedIds: List<String>? = if (isNativeLibraryLoaded) {
                try {
                    val page = getHistoryPage(
                        dbPath = settings.dbPath,
                        key = key,
                        limit = limit,
                        offset = offset,
                    )
                    // A non-empty result means the live feature is on and we have a
                    // lamport-ordered page. An empty result (feature off) falls back below.
                    if (page.isNotEmpty()) page.map { it.itemId } else null
                } catch (e: CopypasteException) {
                    Log.w(TAG, "getItems: getHistoryPage failed (${e.message}) — falling back to wall-time order")
                    null
                }
            } else null

            if (lamportOrderedIds != null) {
                // Fast path: lamport-ordered IDs from the FFI. Decode each from
                // SharedPreferences using the existing parse cache + AEAD decrypt.
                val pinnedList = storedPinnedList()
                val pinnedSet = pinnedList.toHashSet()
                val pinnedIndex: Map<String, Int> = pinnedList.mapIndexed { idx, id -> id to idx }.toMap()
                return@withContext lamportOrderedIds.mapNotNull { id ->
                    val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
                    if (isDeletedBlob(raw)) return@mapNotNull null
                    val item = synchronized(ClipboardItemCache.parseCacheLock) {
                        val entry = ClipboardItemCache.parseCache[id]
                        if (entry != null && entry.rawBlob == raw) entry.item else null
                    } ?: run {
                        val parsed = parseItem(id, raw, key) ?: return@mapNotNull null
                        synchronized(ClipboardItemCache.parseCacheLock) {
                            ClipboardItemCache.parseCache[id] = ClipboardItemCache.ParsedEntry(raw, parsed)
                        }
                        parsed
                    }
                    val isPinned = id in pinnedSet
                    val binaryTooLarge = when {
                        item.isImage ->
                            (prefs.getString("item_img_$id", null)?.let { base64RawByteSize(it).toLong() } ?: 0L) > SYNC_MAX_BLOB_BYTES
                        item.isFile ->
                            (prefs.getString("item_file_$id", null)?.let { base64RawByteSize(it).toLong() } ?: 0L) > SYNC_MAX_BLOB_BYTES
                        else -> item.tooLargeToSync
                    }
                    item.copy(
                        pinned = isPinned,
                        pinnedSortIndex = if (isPinned) (pinnedIndex[id] ?: Int.MAX_VALUE) else -1,
                        tooLargeToSync = binaryTooLarge,
                    )
                }
            }

            // Fallback: wall-time ORDER BY from the SharedPreferences index.
            // Used when the native .so is absent or android-uniffi-live is off.
            val pinnedList = storedPinnedList()
            val pinnedSet = pinnedList.toHashSet()
            // Build index map: id → position in pinned list (0 = top of pinned section).
            val pinnedIndex: Map<String, Int> = pinnedList.mapIndexed { idx, id -> id to idx }.toMap()

            // All stored ids, newest-first (storedIds returns oldest→newest, so reverse).
            val allIds = storedIds().reversed()

            // Split into pinned and unpinned preserving recency order.
            val pinnedIds   = allIds.filter { it in pinnedSet }
            val unpinnedIds = allIds.filter { it !in pinnedSet }

            // Page of unpinned ids for this request.
            val unpinnedPage = unpinnedIds.drop(offset).take(limit)

            // Combine: pinned first (always), then the paged unpinned slice.
            (pinnedIds + unpinnedPage).mapNotNull { id ->
                val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
                // Soft-delete tombstone: skip deleted items in the visible list
                // (cheap last-field check, before any AEAD decrypt).
                if (isDeletedBlob(raw)) return@mapNotNull null
                // A: serve from parse cache when the raw blob is unchanged — avoids a
                // full AEAD decrypt + native isSensitive() for every row on every reload.
                // Only decrypt when the blob has actually been written since last load.
                val item = synchronized(ClipboardItemCache.parseCacheLock) {
                    val entry = ClipboardItemCache.parseCache[id]
                    if (entry != null && entry.rawBlob == raw) entry.item else null
                } ?: run {
                    val parsed = parseItem(id, raw, key) ?: return@mapNotNull null
                    synchronized(ClipboardItemCache.parseCacheLock) {
                        ClipboardItemCache.parseCache[id] = ClipboardItemCache.ParsedEntry(raw, parsed)
                    }
                    parsed
                }
                // AB-8: image bytes are fetched lazily per-row via the two-level LRU
                // in HistoryActivity (cachedThumbnailBitmap). Never eager here.
                val isPinned = id in pinnedSet
                // For binary payloads the synced blob is the full-res image / raw file, NOT the
                // thumbnail shown in the row. Measure its stored byte size cheaply from the
                // Base64 string length (no decode) against the same 8 MiB ceiling sync enforces.
                // Text items keep the plaintextLen-derived flag set in parseItem().
                val binaryTooLarge = when {
                    item.isImage ->
                        (prefs.getString("item_img_$id", null)?.let { base64RawByteSize(it).toLong() } ?: 0L) > SYNC_MAX_BLOB_BYTES
                    item.isFile ->
                        (prefs.getString("item_file_$id", null)?.let { base64RawByteSize(it).toLong() } ?: 0L) > SYNC_MAX_BLOB_BYTES
                    else -> item.tooLargeToSync
                }
                item.copy(
                    pinned = isPinned,
                    pinnedSortIndex = if (isPinned) (pinnedIndex[id] ?: Int.MAX_VALUE) else -1,
                    tooLargeToSync = binaryTooLarge,
                )
            }
        }

    /**
     * Returns the total number of non-deleted unpinned items in the store.
     * Used by the pagination logic in [ClipboardViewModel] to detect when all
     * pages have been loaded (no more items to fetch).
     * Excludes soft-delete tombstones — mirrors the isDeletedBlob() filter in
     * getItems() so the hasMore sentinel stays in sync with actual visible rows.
     */
    fun unpinnedItemCount(): Int {
        val pinnedSet = storedPinnedIds()
        return storedIds().count { id ->
            if (id in pinnedSet) return@count false
            val raw = prefs.getString("item_$id", null) ?: return@count false
            !isDeletedBlob(raw)
        }
    }

    /**
     * Return the raw PNG/JPEG bytes stored for image item [id], or null.
     * Image bytes are persisted under the key "item_img_<id>" as Base64 NO_WRAP.
     */
    fun getImageBytes(id: String): ByteArray? {
        val b64 = prefs.getString("item_img_$id", null) ?: return null
        return try {
            Base64.decode(b64, Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "getImageBytes: failed to decode image for $id: ${e.message}")
            null
        }
    }

    /**
     * Return the thumbnail bytes for image item [id], or null when no thumbnail
     * has been generated yet. Thumbnail bytes are stored under "item_thumb_<id>"
     * as Base64 NO_WRAP (WebP LOSSY on API 30+, PNG on older APIs).
     */
    fun getThumbnailBytes(id: String): ByteArray? {
        val b64 = prefs.getString("item_thumb_$id", null) ?: return null
        return try {
            Base64.decode(b64, Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "getThumbnailBytes: failed to decode thumb for $id: ${e.message}")
            null
        }
    }

    /**
     * AB-8: bytes a history ROW should render for image item [id]. Prefers the
     * stored thumbnail (small, generated at capture from a max-680-px Bitmap) and
     * falls back to full-res only when no thumbnail exists yet (lazy backfill for
     * items captured before thumbnail support). Called per-row on demand by
     * [HistoryActivity] through its bounded LRU — never eagerly in [getItems].
     */
    fun getDisplayImageBytes(id: String): ByteArray? =
        getThumbnailBytes(id) ?: getImageBytes(id)

    /**
     * Persist thumbnail bytes for item [id] under "item_thumb_<id>".
     *
     * No size gate is applied here — thumbnails are intentionally small (generated
     * from a max-680-px scaled Bitmap) so the quota overhead is negligible. The
     * caller ([ClipboardService.captureImageClip]) is responsible for only passing
     * the output of [ImageThumbnailUtils.generateThumbnail].
     */
    fun storeThumbnailBytes(id: String, bytes: ByteArray) {
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        prefs.edit().putString("item_thumb_$id", b64).apply()
        Log.d(TAG, "storeThumbnailBytes: stored ${bytes.size} bytes for $id")
    }

    /**
     * Return the raw file bytes stored for file item [id], or null.
     * File bytes are persisted under the key "item_file_<id>" as Base64 NO_WRAP.
     */
    fun getFileBytes(id: String): ByteArray? {
        val b64 = prefs.getString("item_file_$id", null) ?: return null
        return try {
            Base64.decode(b64, Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "getFileBytes: failed to decode file for $id: ${e.message}")
            null
        }
    }

    /**
     * Persist raw file bytes for item [id] under "item_file_<id>".
     * Rejects files larger than [Settings.maxImageSizeBytes] (reuses the same cap
     * as images — both are binary blobs subject to the same quota).
     */
    fun storeFileBytes(id: String, bytes: ByteArray) {
        val maxBytes = settings.maxImageSizeBytes
        if (bytes.size.toLong() > maxBytes) {
            Log.w(TAG, "storeFileBytes: file ${bytes.size} B exceeds cap $maxBytes — dropping")
            return
        }
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        prefs.edit().putString("item_file_$id", b64).apply()
        Log.d(TAG, "storeFileBytes: stored ${bytes.size} bytes for $id")
    }

    /**
     * Return the stored (fileName, mime) pair for file item [id], or (null, null).
     * Metadata is stored as a pipe-delimited pair under "item_filemeta_<id>".
     * An empty/absent field is returned as null.
     */
    fun getFileMeta(id: String): Pair<String?, String?> {
        val raw = prefs.getString("item_filemeta_$id", null) ?: return null to null
        val parts = raw.split("|", limit = 2)
        val fileName = parts.getOrNull(0)?.takeIf { it.isNotEmpty() }
        val mime = parts.getOrNull(1)?.takeIf { it.isNotEmpty() }
        return fileName to mime
    }

    /**
     * Persist filename and mime for file item [id] under "item_filemeta_<id>".
     * Either value may be null; stored as empty string in that case.
     */
    fun storeFileMeta(id: String, fileName: String?, mime: String?) {
        val encoded = "${fileName ?: ""}|${mime ?: ""}"
        prefs.edit().putString("item_filemeta_$id", encoded).apply()
    }

    /**
     * Persist raw image bytes for item [id].
     * Rejects images larger than [Settings.maxImageSizeBytes].
     */
    fun storeImageBytes(id: String, bytes: ByteArray) {
        val maxBytes = settings.maxImageSizeBytes
        if (bytes.size.toLong() > maxBytes) {
            Log.w(TAG, "storeImageBytes: image ${bytes.size} B exceeds maxImageSizeBytes $maxBytes — dropping")
            return
        }
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        prefs.edit().putString("item_img_$id", b64).apply()
        Log.d(TAG, "storeImageBytes: stored ${bytes.size} bytes for $id")
    }

    suspend fun deleteItem(id: String): Boolean = withContext(Dispatchers.IO) {
        val tombstoneResult: Pair<Boolean, Long> = synchronized(idsWriteLock) {
            val ids = storedIds()
            if (id !in ids) return@synchronized false to 0L
            val existing = prefs.getString("item_$id", null) ?: return@synchronized false to 0L
            // Already a tombstone — nothing to do.
            if (isDeletedBlob(existing)) return@synchronized false to 0L

            val pinnedList = storedPinnedList().toMutableList()
            val wasPinned = pinnedList.remove(id)

            // Write a soft-delete tombstone: bump lamportTs to max(prev+1, nowMs) so
            // the tombstone is time-ordered into wall-clock space (CopyPaste-up1c),
            // preventing collisions with low-magnitude lamport values from older peers.
            // Mirrors next_lamport_ts() in copypaste-core/src/storage/items.rs ~line 68.
            val oldLamport = try {
                val parts = existing.split("|")
                if (parts.size >= 6) parts[5].toLongOrNull() ?: 0L else 0L
            } catch (_: Exception) { 0L }
            val newLamport = nextLamportTs(oldLamport, System.currentTimeMillis())
            val tombstone = encodeTombstone(existing, newLamport)

            // CopyPaste-0qpn: enqueue the delete mutation BEFORE physical write so
            // the producer can push the tombstone even after the row is gone.
            OutboundMutationQueue.enqueueMutation(
                appContext,
                OutboundMutationQueue.MutationRecord(
                    itemId = id,
                    op = OutboundMutationQueue.OP_DELETE,
                    lamportTs = newLamport,
                    wallTimeMs = System.currentTimeMillis(),
                    pinned = false,
                    pinOrder = null,
                ),
            )

            // Clear binary sidecars: image/file bytes are no longer needed once
            // the item is logically deleted (saves storage; tombstone keeps the id
            // in the index to prevent re-sync resurrection).
            val editor = prefs.edit()
                .putString("item_$id", tombstone)
                .remove("item_img_$id")
                .remove("item_thumb_$id")
                .remove("item_file_$id")
                .remove("item_filemeta_$id")
            if (wasPinned) {
                editor.putString(KEY_PINNED_IDS, pinnedList.joinToString(","))
            }
            editor.apply()
            true to newLamport
        }
        val tombstoned = tombstoneResult.first
        // Keep the foreground-service notification's "captured today" count from
        // drifting above reality after a deletion: decrement by one (floored at
        // 0) and re-issue the notification so the shown number matches the store.
        // Only fires when an item was actually tombstoned.
        if (tombstoned) {
            evictParseCache(id) // A: evict stale decrypt cache entry (blob is now a tombstone)
            ClipboardService.onItemsDeleted(appContext, 1)
        }
        tombstoned
    }

    /**
     * Bulk-delete items by [ids]. Items not present in the index are silently
     * skipped. Pinned state is cleaned up for any deleted ids. Image blobs are
     * removed alongside the item entry.
     */
    fun deleteItems(ids: List<String>) {
        if (ids.isEmpty()) return
        val toDelete = ids.toSet()
        var deletedCount = 0
        synchronized(idsWriteLock) {
            val storedList = storedIds().toMutableList()
            val before = storedList.size

            // CopyPaste-0qpn: enqueue per-item OP_BULK_DELETE records BEFORE physical
            // removal so the producer has the itemId + lamportTs even after the rows
            // are gone. Tombstone lamport must be > any stored ts — use nextLamportTs.
            val nowMs = System.currentTimeMillis()
            for (id in toDelete) {
                if (id !in storedList) continue
                val raw = prefs.getString("item_$id", null) ?: continue
                if (isDeletedBlob(raw)) continue
                val oldLamport = try {
                    raw.split("|").getOrNull(5)?.toLongOrNull() ?: 0L
                } catch (_: Exception) { 0L }
                val newLamport = nextLamportTs(oldLamport, nowMs)
                OutboundMutationQueue.enqueueMutation(
                    appContext,
                    OutboundMutationQueue.MutationRecord(
                        itemId = id,
                        op = OutboundMutationQueue.OP_BULK_DELETE,
                        lamportTs = newLamport,
                        wallTimeMs = nowMs,
                        pinned = false,
                        pinOrder = null,
                    ),
                )
            }

            storedList.removeAll(toDelete)
            deletedCount = before - storedList.size
            val pinnedList = storedPinnedList().toMutableList()
            val pinnedBefore = pinnedList.size
            pinnedList.removeAll(toDelete)
            val pinnedChanged = pinnedList.size != pinnedBefore
            ClipboardItemCache.cachedIds = storedList
            val editor = prefs.edit()
                .putString(KEY_ITEM_IDS, storedList.joinToString(","))
            for (id in toDelete) {
                editor.remove("item_$id")
                editor.remove("item_img_$id")
                editor.remove("item_thumb_$id")
                editor.remove("item_file_$id")
                editor.remove("item_filemeta_$id")
                // Remove reverse-lookup key to prevent orphan LWW ghost on re-sync.
                editor.remove("item_id_ref_$id")
            }
            if (pinnedChanged) {
                editor.putString(KEY_PINNED_IDS, pinnedList.joinToString(","))
            }
            editor.apply()
        }
        if (deletedCount > 0) {
            // A: evict deleted ids from the decrypt cache so stale entries don't linger.
            for (id in toDelete) evictParseCache(id)
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "deleteItems: removed $deletedCount items")
    }

    /**
     * Delete all UNPINNED items (text blobs + image blobs + synced-source-id set).
     * Pinned items are preserved — mirrors the macOS daemon `DELETE WHERE pinned = 0`
     * fix (HW-A13). Previously this wiped everything including pinned items;
     * the behaviour is now consistent across platforms so no user-pinned clip is
     * ever silently removed by a "clear" action.
     */
    fun clearAll() {
        var deletedCount = 0
        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val ids = storedIds()

            // CopyPaste-0qpn: enqueue per-item OP_CLEAR records BEFORE physical removal
            // so the producer has the itemId + lamportTs even after the rows are gone.
            val nowMs = System.currentTimeMillis()
            for (id in ids) {
                if (id in pinnedSet) continue
                val raw = prefs.getString("item_$id", null) ?: continue
                if (isDeletedBlob(raw)) continue
                val oldLamport = try {
                    raw.split("|").getOrNull(5)?.toLongOrNull() ?: 0L
                } catch (_: Exception) { 0L }
                val newLamport = nextLamportTs(oldLamport, nowMs)
                OutboundMutationQueue.enqueueMutation(
                    appContext,
                    OutboundMutationQueue.MutationRecord(
                        itemId = id,
                        op = OutboundMutationQueue.OP_CLEAR,
                        lamportTs = newLamport,
                        wallTimeMs = nowMs,
                        pinned = false,
                        pinOrder = null,
                    ),
                )
            }

            val editor = prefs.edit()
            for (id in ids) {
                if (id !in pinnedSet) {
                    editor.remove("item_$id")
                    editor.remove("item_img_$id")
                    editor.remove("item_thumb_$id")
                    editor.remove("item_file_$id")
                    editor.remove("item_filemeta_$id")
                    // Remove reverse-lookup key to prevent orphan LWW ghost on re-sync.
                    editor.remove("item_id_ref_$id")
                }
            }
            // Retain only pinned ids in the index; clear the synced-source-id set
            // (re-syncing pinned items on the next poll is safe).
            val remaining = ids.filter { it in pinnedSet }
            deletedCount = ids.size - remaining.size
            ClipboardItemCache.cachedIds = remaining
            editor
                .putString(KEY_ITEM_IDS, remaining.joinToString(","))
                .remove(KEY_SYNCED_SOURCE_IDS)
                .apply()
        }
        // Reset cross-listener dedup state so a re-copy after a clear stores a
        // fresh row instead of being silently skipped as a duplicate.
        resetDedupState()
        if (deletedCount > 0) {
            evictAllParseCache() // A: full cache wipe — most entries are now gone
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "clearAll: deleted $deletedCount unpinned items (pinned items preserved)")
    }

    /**
     * CopyPaste-12f0 / bd CopyPaste-44rq.59: Degraded-DB recovery — wipe the entire
     * clipboard SharedPreferences store (all items, pinned or not, all indexes, all metadata).
     *
     * This is a DESTRUCTIVE operation with no undo, intended as a last resort when the
     * database is in a state that prevents normal operation (equivalent to macOS's
     * Settings → Storage → Reset database).
     *
     * [confirmed] MUST be true for the wipe to proceed; passing false (or calling without
     * explicit confirmation) is a no-op. This guards against accidental silent invocations —
     * the caller must show a confirmation dialog and only pass `confirmed = true` when the
     * user explicitly acknowledges the destructive action.
     *
     * Unlike [clearAll] which preserves pinned items and queues sync tombstones, this
     * wipes the SharedPreferences file entirely and resets all in-memory state. It does
     * NOT wipe [Settings] — user preferences (encryption key, device id, etc.) are
     * preserved so the device can continue to participate in sync after recovery.
     *
     * @param confirmed Must be `true` to proceed. Pass `false` to no-op (e.g. if the caller
     *   has not yet obtained user confirmation). Throws [IllegalArgumentException] if the
     *   parameter is omitted or false, to make silent misuse immediately visible in tests.
     */
    fun resetDatabase(confirmed: Boolean) {
        require(confirmed) {
            "resetDatabase must only be called with confirmed=true after an explicit user " +
                "confirmation dialog — this operation is irreversible and wipes all history."
        }
        synchronized(idsWriteLock) {
            // Wipe all item data in the repository SharedPreferences file.
            prefs.edit().clear().apply()
            // Reset in-memory parse cache so no stale entries linger after the wipe.
            evictAllParseCache()
            // Invalidate the id cache — prefs.clear() removed KEY_ITEM_IDS.
            ClipboardItemCache.cachedIds = emptyList()
            // Reset dedup state so the first captured item after reset is stored fresh.
            resetDedupState()
        }
        Log.w(TAG, "resetDatabase: clipboard SharedPreferences wiped (recovery action, confirmed=true)")
        // Notify the service that items were deleted so the persistent notification counter resets.
        ClipboardService.onItemsDeleted(appContext, 0)
    }

    /**
     * Delete all UNPINNED items (text blobs + image blobs). Pinned items remain.
     * The synced-source-id set is also cleared (re-syncing pinned items is fine).
     */
    fun clearUnpinned() {
        var deletedCount = 0
        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val ids = storedIds()

            // CopyPaste-0qpn: enqueue per-item OP_CLEAR records BEFORE physical removal
            // so tombstones can propagate even after the rows are gone.
            val nowMs = System.currentTimeMillis()
            for (id in ids) {
                if (id in pinnedSet) continue
                val raw = prefs.getString("item_$id", null) ?: continue
                if (isDeletedBlob(raw)) continue
                val oldLamport = try {
                    raw.split("|").getOrNull(5)?.toLongOrNull() ?: 0L
                } catch (_: Exception) { 0L }
                val newLamport = nextLamportTs(oldLamport, nowMs)
                OutboundMutationQueue.enqueueMutation(
                    appContext,
                    OutboundMutationQueue.MutationRecord(
                        itemId = id,
                        op = OutboundMutationQueue.OP_CLEAR,
                        lamportTs = newLamport,
                        wallTimeMs = nowMs,
                        pinned = false,
                        pinOrder = null,
                    ),
                )
            }

            val editor = prefs.edit()
            for (id in ids) {
                if (id !in pinnedSet) {
                    editor.remove("item_$id")
                    editor.remove("item_img_$id")
                    editor.remove("item_thumb_$id")
                    editor.remove("item_file_$id")
                    editor.remove("item_filemeta_$id")
                    // Remove reverse-lookup key to prevent orphan LWW ghost on re-sync.
                    editor.remove("item_id_ref_$id")
                }
            }
            // Retain only pinned ids in the index; clear source-id seen-set.
            val remaining = ids.filter { it in pinnedSet }
            deletedCount = ids.size - remaining.size
            ClipboardItemCache.cachedIds = remaining
            editor
                .putString(KEY_ITEM_IDS, remaining.joinToString(","))
                .remove(KEY_SYNCED_SOURCE_IDS)
                .apply()
        }
        if (deletedCount > 0) {
            evictAllParseCache() // A: full cache wipe — most entries are now gone
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "clearUnpinned: all unpinned items deleted")
    }

    /**
     * Pin or unpin item [id]. Extracted to ClipboardRepositoryPin.kt (CopyPaste-ra15.4).
     * Pinned items survive the retention prune pass and have no sensitive TTL.
     */
    fun setPinned(id: String, pinned: Boolean) = setPinnedImpl(id, pinned)

    /**
     * Reorder pinned items. Extracted to ClipboardRepositoryPin.kt (CopyPaste-ra15.4).
     * [ids] must contain exactly the currently-pinned item IDs in the desired new order.
     */
    fun reorderPinned(ids: List<String>) = reorderPinnedImpl(ids)

    /**
     * Apply authoritative pin state from an inbound sync row. Extracted to
     * ClipboardRepositoryPin.kt (CopyPaste-ra15.4). Does NOT mint a new local mutation.
     * @param id       The stable item_id.
     * @param pinned   Authoritative pin state from the remote row.
     * @param pinOrder Authoritative pin_order from the remote row (null = no ordering).
     */
    fun applyAuthoritativePinState(id: String, pinned: Boolean, pinOrder: Double?) =
        applyAuthoritativePinStateImpl(id, pinned, pinOrder)

    /**
     * Re-stamp [id] as the most-recently-used item (copy-back). Extracted to
     * ClipboardRepositoryPin.kt (CopyPaste-ra15.4).
     * Returns the new lamport timestamp, or -1L when the item was not found, pinned, or deleted.
     */
    fun bumpToTop(id: String): Long = bumpToTopImpl(id)

    /**
     * Encrypt [plaintext] with [key] and persist, returning the STABLE row id of
     * the stored item — or an empty string when nothing was stored (blank text,
     * oversized text, sensitive content, a recent local duplicate, or — for synced
     * items — already stored under the same [sourceId]).
     *
     * After inserting, calls [pruneToLimits] to enforce the storage-quota cap
     * (SIZE only — no count cap).
     *
     * [sourceApp] is the package name of the app that set the clipboard (e.g.
     * "com.agilebits.onepassword"). When non-null and present in
     * [KNOWN_SENSITIVE_PACKAGES], the item is stored with isSensitive forced to
     * true at read time (parseItem), regardless of the content classifier verdict.
     * Conservative: only ever overrides sensitivity to TRUE, never false.
     */
    suspend fun storeItem(
        plaintext: String,
        key: ByteArray,
        sourceId: String? = null,
        overrideId: String? = null,
        contentType: String = "text/plain",
        lamportTs: Long = 0L,
        wallTimeMs: Long = System.currentTimeMillis(),
        originDeviceId: String = "",
        sourceApp: String? = null,
    ): String = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext ""

        // ── Size enforcement: reject oversized text before any crypto work.
        val textBytes = plaintext.toByteArray(Charsets.UTF_8)
        val maxTextBytes = settings.maxTextSizeBytes
        if (textBytes.size.toLong() > maxTextBytes) {
            Log.w(TAG, "storeItem: text ${textBytes.size} B exceeds maxTextSizeBytes $maxTextBytes — dropping")
            return@withContext ""
        }

        // The id that dedup keys on: an explicit [sourceId] wins; otherwise the
        // incoming [overrideId] (which IS the stable remote id) is the source id.
        val dedupSourceId = sourceId ?: overrideId

        // ── LOW-2: source-id dedup for incoming synced items.
        if (dedupSourceId != null) {
            synchronized(seenSourceIdsLock) {
                val seen = storedSourceIds()
                if (!isNewSourceId(dedupSourceId, seen)) {
                    Log.d(TAG, "Synced item $dedupSourceId already stored — skipping")
                    return@withContext ""
                }
                recordSourceId(dedupSourceId, seen)
            }
        }

        // ── HIGH-3: cross-listener dedup (identical content within DEDUP_WINDOW_MS).
        // E7: key on content LENGTH + hash rather than a bare 32-bit hashCode().
        // A length-prefix makes an accidental collision far less likely — a
        // different clip would have to share both its length and its hashCode
        // within the window to be wrongly dropped.
        val dedupKey = "${plaintext.length}:${plaintext.hashCode()}"
        synchronized(ClipboardDedupState.dedupLock) {
            val now = System.currentTimeMillis()
            if (dedupKey == ClipboardDedupState.lastStoredKey && now - ClipboardDedupState.lastStoredAtMs < ClipboardDedupState.DEDUP_WINDOW_MS) {
                Log.d(TAG, "Duplicate clip within ${ClipboardDedupState.DEDUP_WINDOW_MS}ms — skipping")
                return@withContext ""
            }
            ClipboardDedupState.lastStoredKey = dedupKey
            ClipboardDedupState.lastStoredAtMs = now
        }

        // AB-6b — PARITY with macOS: do NOT drop sensitive items. macOS stores
        // them (the daemon persists every captured clip) and masks them in the
        // UI. Dropping them on Android meant macOS-captured secrets never showed
        // up here, breaking cross-device coherence. We now STORE the item; the
        // is_sensitive flag is recomputed at read time by parseItem() and drives
        // the PRIVATE chip + masked preview in HistoryActivity. (The native
        // detector threshold was aligned to >=0.70 in ABI 14 so the capture-time
        // and read-time verdicts agree.)

        // STABLE identity: reuse an incoming item's stable id verbatim; mint a
        // fresh UUID only for a locally-captured clip. This is the value bound
        // into the AEAD AAD and reused on every later push/sync.
        val id = overrideId?.takeIf { it.isNotBlank() } ?: UUID.randomUUID().toString()
        // key_version=2 matches the daemon's ITEM_KEY_VERSION_CURRENT (AAD "{id}|4|2").
        // This makes Android-stored items decryptable on the daemon side and vice versa.
        val keyVersion: UByte = 2u
        // SECURITY: do NOT fall back to localAesEncrypt (AES-256-GCM) on FFI failure.
        // AES-GCM items use a different key derivation/AAD format that peers (daemon,
        // other Android devices) cannot decrypt — storing them produces items that silently
        // fail sync with no user-visible error.  Instead, propagate the failure so the
        // caller skips this store, and post a one-shot sentinel notification so the user
        // knows something is wrong.
        val blob = try {
            encryptText(id, textBytes, key, keyVersion)
        } catch (e: IllegalStateException) {
            Log.e(TAG, "storeItem: native encryption unavailable (${e.message}) — " +
                "item NOT stored to avoid producing AES-GCM items that peers cannot decrypt")
            if (!nativeUnavailableNotified) {
                nativeUnavailableNotified = true
                NotificationHelper.notifyNativeUnavailable(appContext)
            }
            throw e
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "storeItem: native encryption unavailable (UnsatisfiedLinkError) — " +
                "item NOT stored to avoid producing AES-GCM items that peers cannot decrypt")
            if (!nativeUnavailableNotified) {
                nativeUnavailableNotified = true
                NotificationHelper.notifyNativeUnavailable(appContext)
            }
            throw IllegalStateException("UnsatisfiedLinkError: ${e.message}", e)
        }

        val encoded = encodeItem(blob, textBytes.size, contentType = contentType, lamportTs = lamportTs, wallTimeMs = wallTimeMs, originDeviceId = originDeviceId, keyVersion = keyVersion, sourceApp = sourceApp)
        synchronized(idsWriteLock) {
            // Append the id, removing any prior occurrence first so the index
            // stays canonical (no duplicate ids). A synced item re-stored under
            // the same overrideId — e.g. after clearUnpinned wiped the
            // synced-source-id seen-set while a pinned id stayed in the index —
            // would otherwise append a second copy of the same id, which then
            // crashes the history LazyColumn ("Key … was already used").
            val ids = appendUniqueId(storedIds(), id)
            ClipboardItemCache.cachedIds = ids
            prefs.edit()
                .putString("item_$id", encoded)
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                // Reverse-lookup: item_id → storage_id for LWW cloud sync.
                // For locally-captured items the storage id IS the item_id.
                .putString("item_id_ref_$id", id)
                .apply()
        }

        Log.d(TAG, "Stored item $id (${textBytes.size} bytes, contentType=$contentType)")

        // Prune to size-only quota after insert.
        pruneToLimits()
        id
    }

    /**
     * Store a cloud-synced item with Last-Writer-Wins semantics (Task 5).
     *
     * [itemId] is the stable UUID from the `item_id` column (same across devices).
     * [incomingLamportTs] is the lamport_ts from the cloud row (Unix-ms on both
     * sides, so the compare is valid cross-platform).
     *
     * Behaviour:
     * - If [itemId] is not yet stored locally → store as a new item (same as
     *   [storeItem]).
     * - If [itemId] already exists locally AND [incomingLamportTs] is strictly
     *   greater than the stored lamport_ts → replace the stored row in-place
     *   (re-encrypt with [key], keep the same storage id in the index).
     * - Otherwise (equal or older lamport_ts) → skip as a dup.
     *
     * Returns true when a new row was inserted or an existing row was replaced.
     */
    suspend fun storeItemWithLww(
        plaintext: String,
        key: ByteArray,
        itemId: String,
        incomingLamportTs: Long,
        wallTimeMs: Long = System.currentTimeMillis(),
        originDeviceId: String = "",
    ): Boolean = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext false

        // AB-6b — PARITY with macOS: store sensitive synced items instead of
        // dropping them. A sensitive clip captured on macOS must round-trip to
        // Android and render masked, not silently vanish. Sensitivity is
        // recomputed at read time by parseItem() and drives the masked preview.

        // ── REPLACE PATH: close the TOCTOU between the existingStorageId
        // lookup + storedLamportTs read and the final putString write.
        //
        // Previously the lookup and the lamport comparison happened OUTSIDE
        // idsWriteLock, so a concurrent deleteItem (which holds idsWriteLock
        // while it removes "item_<id>" and rewrites the index) could delete
        // the row between our read and our locked write, resurrecting a ghost
        // blob under a storage key that no longer appears in the index.
        //
        // Fix: encrypt into a local variable FIRST (encryption is expensive and
        // has no shared state — doing it inside the lock would increase
        // contention unnecessarily), then enter idsWriteLock for the entire
        // read-decide-write sequence: lookup → lamport compare → putString.
        // There is no re-entrant idsWriteLock acquisition inside the block
        // (no call to deleteItem / storedIds / storeItem), so no deadlock.

        val plaintextBytes = plaintext.toByteArray(Charsets.UTF_8)

        val replaced = synchronized(idsWriteLock) {
            val existingStorageId = prefs.getString("item_id_ref_$itemId", null)
                ?: return@synchronized false  // not yet stored → fall through to new-item path

            // LWW: apply the SAME total order as remote_wins() in
            // copypaste-sync/src/merge.rs ~lines 97-112:
            //   1. lamport_ts — larger wins.
            //   2. wall_time  — larger wins (tie-break on equal lamport).
            //   3. origin_device_id — lexicographically larger wins (deterministic).
            // CopyPaste-up1c: previously only lamport_ts was compared; the wall_time
            // + origin_device_id tie-break was missing, causing non-deterministic
            // conflict resolution on simultaneous edits.
            // Read the full raw blob once so we can extract both lamport_ts (field 5),
            // wall_time (field 0), and origin_device_id (field 7) for the 3-key LWW
            // without double-reading prefs.
            val storedRaw = prefs.getString("item_$existingStorageId", null)
            val storedParts = storedRaw?.split("|")
            val storedTs = storedParts?.getOrNull(5)?.toLongOrNull() ?: 0L
            val remoteWins = when {
                incomingLamportTs > storedTs -> true
                incomingLamportTs < storedTs -> false
                else -> {
                    // Equal lamport — compare wall_time then origin_device_id.
                    // Mirrors remote_wins() in copypaste-sync/src/merge.rs ~lines 106-109.
                    val storedWall = storedParts?.getOrNull(0)?.toLongOrNull() ?: 0L
                    val storedOrigin = storedParts?.getOrNull(7) ?: ""
                    when {
                        wallTimeMs > storedWall -> true
                        wallTimeMs < storedWall -> false
                        else -> originDeviceId > storedOrigin
                    }
                }
            }
            if (!remoteWins) {
                Log.d(TAG, "LWW: skipping dup item_id=$itemId (stored=$storedTs, incoming=$incomingLamportTs)")
                return@synchronized null  // null = "skip, do not store as new item either"
            }

            // Replace in-place: re-encrypt and overwrite the stored blob.
            // key_version=2 matches the daemon's ITEM_KEY_VERSION_CURRENT.
            val lwwKeyVersion: UByte = 2u
            // SECURITY: same fail-closed rule as storeItem — do NOT fall back to
            // AES-GCM on FFI failure. Propagate so the LWW replace is skipped.
            val blob = try {
                encryptText(existingStorageId, plaintextBytes, key, lwwKeyVersion)
            } catch (e: IllegalStateException) {
                Log.e(TAG, "LWW replace: native encryption unavailable (${e.message}) — " +
                    "skipping replace to avoid producing AES-GCM items that peers cannot decrypt")
                if (!nativeUnavailableNotified) {
                    nativeUnavailableNotified = true
                    NotificationHelper.notifyNativeUnavailable(appContext)
                }
                return@synchronized null  // null → skip, do not attempt new-item insert
            } catch (e: UnsatisfiedLinkError) {
                Log.e(TAG, "LWW replace: native encryption unavailable (UnsatisfiedLinkError) — " +
                    "skipping replace to avoid producing AES-GCM items that peers cannot decrypt")
                if (!nativeUnavailableNotified) {
                    nativeUnavailableNotified = true
                    NotificationHelper.notifyNativeUnavailable(appContext)
                }
                return@synchronized null  // null → skip, do not attempt new-item insert
            }
            val encoded = encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs, wallTimeMs = wallTimeMs, originDeviceId = originDeviceId, keyVersion = lwwKeyVersion)
            prefs.edit().putString("item_$existingStorageId", encoded).apply()
            evictParseCache(existingStorageId) // A: blob changed — evict stale decrypt entry
            Log.d(TAG, "LWW replaced item_id=$itemId storageId=$existingStorageId (lamport $storedTs→$incomingLamportTs)")
            true  // replaced successfully
        }

        // null  → duplicate (older/equal lamport), skip (nothing changed → no prune)
        // true  → replaced in-place; prune since the replace may have grown a row
        // false → item not found, fall through to new-item insert below
        when (replaced) {
            null -> return@withContext false
            true -> {
                // The replace's synchronized(idsWriteLock) block has already exited
                // above, so pruneToLimits() (which takes idsWriteLock) cannot deadlock.
                pruneToLimits()
                return@withContext true
            }
            else -> { /* false: fall through to new-item insert below */ }
        }

        // New item: generate a fresh storage id and store normally.
        // key_version=2 matches the daemon's ITEM_KEY_VERSION_CURRENT.
        val newKeyVersion: UByte = 2u
        val storageId = itemId // Use the stable item_id as the storage key for easy lookup.
        // SECURITY: same fail-closed rule — do NOT fall back to AES-GCM on FFI failure.
        val blob = try {
            encryptText(storageId, plaintextBytes, key, newKeyVersion)
        } catch (e: IllegalStateException) {
            Log.e(TAG, "storeItemWithLww: native encryption unavailable (${e.message}) — " +
                "skipping new-item insert to avoid producing AES-GCM items that peers cannot decrypt")
            if (!nativeUnavailableNotified) {
                nativeUnavailableNotified = true
                NotificationHelper.notifyNativeUnavailable(appContext)
            }
            return@withContext false
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "storeItemWithLww: native encryption unavailable (UnsatisfiedLinkError) — " +
                "skipping new-item insert to avoid producing AES-GCM items that peers cannot decrypt")
            if (!nativeUnavailableNotified) {
                nativeUnavailableNotified = true
                NotificationHelper.notifyNativeUnavailable(appContext)
            }
            return@withContext false
        }
        val encoded = encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs, wallTimeMs = wallTimeMs, originDeviceId = originDeviceId, keyVersion = newKeyVersion)

        synchronized(idsWriteLock) {
            // TOCTOU guard: re-check inside the lock. A concurrent caller (FgsSyncLoop
            // + SupabasePollWorker both polling) may have raced through the new-item
            // path and already inserted this itemId between our first lookup (above,
            // which returned false) and now. If so, abort to avoid a duplicate row.
            if (prefs.getString("item_id_ref_$storageId", null) != null) {
                Log.d(TAG, "storeItemWithLww: duplicate detected under lock for item_id=$itemId — aborting")
                return@withContext false
            }
            val ids = appendUniqueId(storedIds(), storageId)
            ClipboardItemCache.cachedIds = ids
            prefs.edit()
                .putString("item_$storageId", encoded)
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                .putString("item_id_ref_$storageId", storageId)
                .apply()
        }
        Log.d(TAG, "storeItemWithLww: stored new item_id=$itemId as storageId=$storageId")
        pruneToLimits()
        true
    }

    /**
     * Return the id of the most recently stored item, or null when the index is
     * empty. Used by image capture callers that need the id that [storeItem] just
     * wrote so they can call [storeImageBytes] under the same key.
     *
     * Safe to call immediately after [storeItem] returns true because storeItem
     * appends the new id at the END of the comma-joined index before returning.
     * The caller runs on [Dispatchers.IO] and storeItem holds [idsWriteLock] for
     * the entire append, so by the time storeItem returns the id is visible here.
     */
    fun lastStoredId(): String? = storedIds().lastOrNull()

    /**
     * CopyPaste-vg4r: return the stored lamport_ts for a stable [itemId] (the relay/cloud
     * item_id, not a local storage id), or null when the item does not exist locally.
     *
     * Used by binary ingest paths (image, file) in [SyncManager.ingestRelaySseItem] to
     * apply LWW ordering without going through [storeItemWithLww] (which is text-only).
     * The caller compares the incoming lamport_ts against the stored one before
     * deciding whether to overwrite:
     *   - incoming > stored → overwrite (new version wins)
     *   - incoming <= stored → skip (local version is current or newer)
     *
     * Thread-safe: reads are protected by the [idsWriteLock] monitor (same lock that
     * [storeItem] / [storeItemWithLww] hold during their read-decide-write sequences).
     */
    fun storedLamportTsForItemId(itemId: String): Long? {
        val storageId = synchronized(idsWriteLock) {
            prefs.getString("item_id_ref_$itemId", null)
        } ?: return null
        val raw = prefs.getString("item_$storageId", null) ?: return null
        // Blob format: <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>|…
        // lamportTs is field index 5.
        return raw.split("|").getOrNull(5)?.toLongOrNull()
    }

    /**
     * Total number of stored items (pinned + unpinned).
     * Used by the history header count display (parity with macOS).
     * Excludes soft-delete tombstones that remain in KEY_ITEM_IDS to prevent
     * re-sync resurrection — mirrors the isDeletedBlob() filter in getItems().
     */
    fun totalItemCount(): Int = storedIds().count { id ->
        val raw = prefs.getString("item_$id", null) ?: return@count false
        !isDeletedBlob(raw)
    }

    /**
     * Decrypt and return the FULL plaintext for item [id], or null when the item
     * does not exist or cannot be decrypted.
     *
     * Used by the copy-to-clipboard path in [HistoryActivity] to ensure the user
     * copies the complete original text, not the 140-char [ClipboardItem.snippet].
     */
    suspend fun loadFullPlaintext(id: String, key: ByteArray): String? =
        withContext(Dispatchers.IO) {
            loadFullPlaintextBlocking(id, key)
        }

    /**
     * Full-content search. Returns the subset of [ids] whose FULL text matches [query].
     *
     * PG-17 (mxoq / osxa): When the native library is loaded, delegates to the
     * FTS5-indexed [ftsSearch] FFI (O(log N), ranked by relevance) so Android
     * uses the SAME FTS5 engine as the macOS daemon's `search` IPC handler. The
     * FTS result set (keyed by [uniffi.copypaste_android.SearchResultItem.itemId])
     * is intersected with the caller's [ids] to respect the display-visible set.
     *
     * Falls back to the O(N) full-decrypt scan when the native library is absent
     * (stub mode / test environment) so existing behaviour is preserved.
     *
     * A blank [query] returns all [ids] unchanged. Decryption failures in the
     * fallback path are treated as non-matches.
     *
     * Runs on [Dispatchers.IO]; the caller is expected to debounce.
     */
    suspend fun searchIds(ids: List<String>, query: String, key: ByteArray): Set<String> =
        withContext(Dispatchers.IO) {
            val q = query.trim()
            if (q.isEmpty()) return@withContext ids.toSet()

            // PG-17 (mxoq): use FTS5 when the native .so is available.
            if (isNativeLibraryLoaded) {
                val idsSet = ids.toHashSet()
                return@withContext try {
                    ftsSearch(
                        dbPath = settings.dbPath,
                        key = key,
                        query = q,
                        // Fetch up to all known ids + a small buffer; the FTS index may
                        // contain items that have since been deleted from the local store,
                        // so we cap at ids.size + 50 to avoid unbounded allocations.
                        limit = (ids.size + 50).coerceAtLeast(50),
                    ).mapTo(HashSet()) { it.itemId }.intersect(idsSet)
                } catch (e: CopypasteException) {
                    Log.w(TAG, "searchIds: ftsSearch failed (${e.message}) — falling back to O(N) decrypt scan")
                    // Fall through to the O(N) fallback below.
                    ids.filterTo(HashSet()) { id ->
                        val full = loadFullPlaintextBlocking(id, key)
                        full != null && full.contains(q, ignoreCase = true)
                    }
                }
            }

            // Fallback: O(N) full-decrypt scan (stub mode / no live .so).
            ids.filterTo(HashSet()) { id ->
                val full = loadFullPlaintextBlocking(id, key)
                full != null && full.contains(q, ignoreCase = true)
            }
        }

    /**
     * Synchronous full-plaintext decrypt for use inside an already-`IO` context
     * (e.g. [searchIds]). Mirrors [loadFullPlaintext] without an extra dispatch.
     */
    internal fun loadFullPlaintextBlocking(id: String, key: ByteArray): String? {
        val raw = prefs.getString("item_$id", null) ?: return null
        val parts = raw.split("|")
        val nonceB64 = parts.getOrNull(3) ?: return null
        val ctB64 = parts.getOrNull(4) ?: return null
        return try {
            val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
            val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
            decryptForPreview(id, ciphertext, nonce, key, keyVersionFromParts(parts))
        } catch (e: Exception) {
            Log.d(TAG, "loadFullPlaintextBlocking: decrypt failed for $id: ${e.message}")
            null
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /**
     * Enforce the storage-quota cap by evicting the oldest UNPINNED items.
     *
     * Only the byte quota is enforced — there is no count cap (mirrors desktop policy).
     * "Stored payload bytes" approximates the UTF-8 byte length of each blob string
     * (text) plus the stored Base64 length for image bytes.
     *
     * PINNED items are counted in total bytes but never evicted.
     */

    /**
     * CopyPaste-iovc: public entry-point so Settings can retroactively apply the
     * history cap immediately after the user changes [Settings.maxHistoryItems]
     * and taps Save — without waiting for the next clipboard capture to call the
     * private [pruneToLimits] path.
     *
     * Delegates to [pruneToLimits] (no new logic; just visibility promotion).
     */
    fun applyHistoryCap() {
        pruneToLimits()
    }

    /**
     * CopyPaste-bdac.88: Compute how many items reducing the "Maximum stored
     * items" cap to [newMax] would PERMANENTLY tombstone — WITHOUT mutating the
     * store. Used by Settings to populate the confirmation dialog before the
     * destructive [applyHistoryCap] runs.
     *
     * Counts LIVE (non-tombstone) items only; PINNED items are never evicted, so
     * the result is the number of live UNPINNED items that would be removed to
     * bring the live count down to [newMax]. Returns 0 when [newMax] is >= the
     * current live item count — a non-destructive change that needs no
     * confirmation (matching the macOS display-filter, which deletes nothing).
     *
     * Delegates to the shared [planCountCapEvictions] planner so the dialog count
     * and the actual [pruneToLimits] count-cap pass can never disagree.
     */
    fun countPrunableByMaxItems(newMax: Int): Int =
        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val liveIds = storedIds().filter { id ->
                val raw = prefs.getString("item_$id", null) ?: return@filter false
                !isDeletedBlob(raw)
            }
            planCountCapEvictions(liveIds, pinnedSet, newMax).size
        }

    // Extracted to ClipboardRepositoryPrune.kt (CopyPaste-ra15.4).
    private fun pruneToLimits() = pruneToLimitsImpl()

    // Extracted to ClipboardRepositoryPrune.kt (CopyPaste-ra15.4).
    // AB-13 — retention TTL auto-wipe (macOS parity). See pruneByAgeImpl for docs.
    private fun pruneByAge(key: ByteArray? = null) = pruneByAgeImpl(key)

    /**
     * General retention TTL in seconds. Read from the same "copypaste" prefs file
     * Settings owns (key `general_ttl_secs`) so a future settings UI can drive it;
     * defaults to [DEFAULT_GENERAL_TTL_SECS] (30 days) to mirror the macOS
     * `sync_ttl_secs` retention floor. `0` disables the general age pass.
     */
    internal fun generalTtlSecs(): Long =
        appContext.getSharedPreferences(SETTINGS_PREFS_NAME, Context.MODE_PRIVATE)
            .getLong(KEY_GENERAL_TTL_SECS, DEFAULT_GENERAL_TTL_SECS)
            .coerceAtLeast(0L)

    /**
     * Raw decoded byte count of a Base64 (NO_WRAP) string, computed without
     * allocating the decoded buffer. NO_WRAP emits no line breaks, so the input
     * length is a multiple of 4 and any padding is 0–2 trailing '=' chars:
     *   rawBytes = (len / 4) * 3 - paddingCount
     * Used by [pruneToLimits] so image rows are accounted in the same unit
     * ([storeImageBytes] caps on raw `bytes.size`), preventing the byte quota
     * from being over-counted by the ~1.33x Base64 inflation.
     */
    private fun base64RawByteSize(b64: String): Int =
        ClipboardBlobCodec.base64RawByteSize(b64)

    /**
     * The ordered id index, read back canonical: blanks removed and any
     * duplicate ids collapsed to their FIRST (oldest) occurrence. Persisting a
     * dup-free index is the invariant every writer relies on; reading it
     * de-duplicated also heals any index that an older build may have corrupted,
     * so the history LazyColumn never sees a repeated key.
     */
    internal fun storedIds(): List<String> {
        // Fast path: in-memory snapshot populated by every writer.
        ClipboardItemCache.cachedIds?.let { return it }
        // Cold start: parse from SharedPreferences and cache.
        val ids = prefs.getString(KEY_ITEM_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?.distinct()
            ?: emptyList()
        ClipboardItemCache.cachedIds = ids
        return ids
    }

    /**
     * Append [id] to [current], guaranteeing it appears exactly once and at the
     * end (most-recent position). Any prior occurrence is removed first so the
     * index can never hold the same id twice — the root invariant that keeps the
     * history LazyColumn's `key = { it.id }` from crashing on a duplicate.
     */
    internal fun appendUniqueId(current: List<String>, id: String): List<String> {
        val next = current.toMutableList()
        next.remove(id)
        next.add(id)
        return next
    }

    /** Ordered list of pinned ids — position 0 is displayed at the top of the pinned section. */
    internal fun storedPinnedList(): List<String> =
        prefs.getString(KEY_PINNED_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?: emptyList()

    internal fun storedPinnedIds(): Set<String> = storedPinnedList().toHashSet()

    internal fun storedSourceIds(): LinkedHashSet<String> =
        LinkedHashSet(
            prefs.getString(KEY_SYNCED_SOURCE_IDS, "")
                ?.split(",")
                ?.filter { it.isNotBlank() }
                ?: emptyList()
        )

    internal fun recordSourceId(sourceId: String, seen: LinkedHashSet<String>) {
        seen.add(sourceId)
        while (seen.size > MAX_SEEN_SOURCE_IDS) {
            val oldest = seen.iterator().next()
            seen.remove(oldest)
        }
        prefs.edit().putString(KEY_SYNCED_SOURCE_IDS, seen.joinToString(",")).apply()
    }

    /**
     * Encode a stored item as a pipe-delimited string (v5 format, 10 fields):
     * <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>|<deleted>|<originDeviceId>|<keyVersion>|<sourceApp>
     *
     * The lamportTs field (index 5) was added for LWW cloud sync. Legacy rows
     * (only 5 fields) are read back with lamportTs=0.
     *
     * The deleted field (index 6) was added for local soft-delete tombstones.
     * Legacy rows (fewer than 7 fields) parse as deleted=false (back-compat).
     * A tombstone has deleted=1; its ciphertext/nonce are empty strings so the
     * encrypted payload is not retained on disk after a user delete.
     *
     * The originDeviceId field (index 7) was added for origin-device attribution
     * (parity with macOS HistoryView device filter + DeviceBadge). Legacy rows
     * (fewer than 8 fields) parse as originDeviceId=null (back-compat). Blank
     * string is stored for locally-captured items with no known device id.
     *
     * The keyVersion field (index 8) identifies the AEAD key generation used to
     * encrypt the payload.
     *
     * The sourceApp field (index 9) stores the package name of the source app (e.g.
     * "com.agilebits.onepassword"). Legacy rows (fewer than 10 fields) parse as
     * sourceApp=null (back-compat), which leaves isSensitive unaffected — old blobs
     * are never force-marked sensitive by this field. Blank string is stored for
     * locally-captured items with no known source app.
     */
    private fun encodeItem(
        blob: EncryptedBlob,
        plaintextLen: Int,
        contentType: String = "text/plain",
        lamportTs: Long = 0L,
        wallTimeMs: Long = System.currentTimeMillis(),
        deleted: Boolean = false,
        originDeviceId: String = "",
        keyVersion: UByte = 2u,
        sourceApp: String? = null,
    ): String = ClipboardBlobCodec.encodeItem(blob, plaintextLen, contentType, lamportTs, wallTimeMs, deleted, originDeviceId, keyVersion, sourceApp)

    /**
     * Build a tombstone blob for item [id].
     *
     * The tombstone keeps the entry in the id-index so a re-sync cannot
     * resurrect the deleted item, but clears the encrypted payload to avoid
     * retaining plaintext on disk. Field layout mirrors [encodeItem] v4:
     * <nowMs>|<contentType>|0||tombstone|<lamportTs>|1|<originDeviceId>
     *
     * The nonce field is empty and the ciphertext is the literal string
     * "tombstone" (harmless; the deleted flag prevents any decrypt attempt).
     * The lamportTs is bumped so LWW on the same item_id sees this as newer
     * and will not be overwritten by a stale re-sync of the original text.
     * The original originDeviceId (index 7) is preserved so the tombstone still
     * attributes to its source device.
     */
    private fun encodeTombstone(existingRaw: String, bumpedLamportTs: Long): String =
        ClipboardBlobCodec.encodeTombstone(existingRaw, bumpedLamportTs)

    /**
     * Read the deleted flag from a raw blob string.
     * Field 6 (index 6) is the deleted flag: "1" = deleted, absent/other = false.
     * Back-compat: blobs with fewer than 7 fields (legacy v1/v2 format) are NOT deleted.
     * NOTE: index 6 is read explicitly (not the LAST field) because v4 appended
     * originDeviceId at index 7 — the deleted flag is no longer terminal.
     */
    private fun isDeletedBlob(raw: String): Boolean =
        ClipboardBlobCodec.isDeletedBlob(raw)

    /**
     * Return a new blob string with field 5 (lamport_ts) replaced by [newLamportTs].
     *
     * CopyPaste-up1c: used by setPinned / reorderPinned to stamp a nextLamportTs
     * value into the blob WITHOUT re-encrypting (the AEAD ciphertext fields are
     * unchanged — only the metadata field is updated). This is safe because
     * lamport_ts is not part of the AEAD AAD; the cipher only covers the plaintext.
     *
     * Returns the raw string unchanged when the blob has fewer than 6 fields (legacy
     * pre-lamport format) — those items cannot carry a lamport stamp.
     */
    private fun bumpBlobLamportTs(raw: String, newLamportTs: Long): String =
        ClipboardBlobCodec.bumpBlobLamportTs(raw, newLamportTs)

    private fun parseItem(id: String, raw: String, key: ByteArray): ClipboardItem? =
        ClipboardBlobCodec.parseItem(id, raw, key)

    /**
     * Read the AEAD key_version stored in field 8 (index 8) of a pipe-delimited
     * blob string. Returns 1 (legacy) when the field is absent or unparseable,
     * so pre-4i2 items (written without the field) still decrypt correctly.
     */
    private fun keyVersionFromParts(parts: List<String>): UByte =
        ClipboardBlobCodec.keyVersionFromParts(parts)

    private fun decryptForPreview(
        id: String,
        ciphertext: ByteArray,
        nonce: ByteArray,
        key: ByteArray,
        keyVersion: UByte,
    ): String = ClipboardBlobCodec.decryptForPreview(id, ciphertext, nonce, key, keyVersion)

    /**
     * Read the stored lamport_ts for the item at [storageId].
     * Returns 0 when the item does not exist or has no lamport_ts (legacy format).
     */
    private fun storedLamportTs(storageId: String): Long {
        val raw = prefs.getString("item_$storageId", null) ?: return 0L
        return try {
            val parts = raw.split("|")
            if (parts.size >= 6) parts[5].toLong() else 0L
        } catch (_: Exception) {
            0L
        }
    }

    companion object {
        private const val TAG = "ClipboardRepository"

        /**
         * Package names of apps whose clipboard content must always be treated as
         * sensitive (isSensitive=true), regardless of the content-classifier verdict.
         * Defined in [ClipboardBlobCodec.KNOWN_SENSITIVE_PACKAGES]; re-exported here
         * for call-site compatibility (CopyPaste-44rq.48 / PRIV-7).
         */
        val KNOWN_SENSITIVE_PACKAGES: Set<String>
            get() = ClipboardBlobCodec.KNOWN_SENSITIVE_PACKAGES

        /**
         * Compute the next Lamport timestamp — delegates to the package-level
         * [com.copypaste.android.nextLamportTs] extracted in ClipboardRepositoryPlan.kt
         * (CopyPaste-ra15.4). Kept here so callers using [ClipboardRepository.nextLamportTs]
         * are unaffected.
         *
         * `max(prev + 1, now_ms)` — monotonic and wall-clock time-ordered.
         */
        fun nextLamportTs(prevLamport: Long, nowMs: Long): Long =
            com.copypaste.android.nextLamportTs(prevLamport, nowMs)

        /**
         * CopyPaste-bdac.88 / crh3.39 / crh3.108 — PURE count-cap planner.
         *
         * Delegates to the package-level [com.copypaste.android.planCountCapEvictions]
         * extracted in ClipboardRepositoryPlan.kt (CopyPaste-ra15.4). Kept here so
         * callers using [ClipboardRepository.planCountCapEvictions] are unaffected.
         *
         * See ClipboardRepositoryPlan.kt for full documentation.
         */
        internal fun planCountCapEvictions(
            liveIds: List<String>,
            pinned: Set<String>,
            maxItems: Int,
        ): List<String> = com.copypaste.android.planCountCapEvictions(liveIds, pinned, maxItems)

        /**
         * Sync size ceiling in bytes (8 MiB). Delegates to [ClipboardBlobCodec.SYNC_MAX_BLOB_BYTES]
         * -- single source of truth, do not scatter the literal.
         */
        const val SYNC_MAX_BLOB_BYTES: Long = 8L * 1024 * 1024

        /** SharedPreferences file name -- single source of truth, not scattered as string literals. */
        const val PREFS_NAME = "copypaste_items"

        /**
         * Name of the SharedPreferences file that [Settings] owns ("copypaste").
         * [generalTtlSecs] reads the general retention TTL from here so the value
         * is shared with any future settings UI without coupling to [Settings]'s
         * private prefs handle. Must stay in sync with the literal in Settings.
         */
        const val SETTINGS_PREFS_NAME = "copypaste"

        /** Pref key for the general retention TTL (seconds); `0` disables. */
        const val KEY_GENERAL_TTL_SECS = "general_ttl_secs"

        /**
         * Default general retention TTL = 30 days, mirroring the macOS
         * `SYNC_TTL_SECS` (2_592_000 s) retention floor. Items older than this are
         * auto-wiped by [pruneByAge] unless pinned.
         */
        const val DEFAULT_GENERAL_TTL_SECS: Long = 30L * 24 * 60 * 60

        /**
         * Default page size for [getItems] pagination.
         * First page = pinned + 50 most-recent unpinned; each subsequent page appends
         * 50 more unpinned rows as the user scrolls near the end of the list.
         */
        const val PAGE_SIZE = 50

        fun normalizeContentTypeForSync(stored: String): String =
            if (stored == "text" || stored.startsWith("text/")) "text" else stored

        const val KEY_ITEM_IDS = "item_ids"
        const val KEY_SYNCED_SOURCE_IDS = "synced_source_ids"
        const val KEY_PINNED_IDS = "pinned_ids"

        const val MAX_SEEN_SOURCE_IDS = 1_000

        // -- Dedup state -- delegates to ClipboardDedupState ------------------
        //
        // Process-wide dedup state extracted to [ClipboardDedupState] (CopyPaste-g06m.20).
        // These forwarding members preserve the public API for callers that use
        // ClipboardRepository.expectClip / shouldSkipExpectedClip / etc.

        /** @see ClipboardDedupState.expectClip */
        fun expectClip(content: String) = ClipboardDedupState.expectClip(content)

        /** @see ClipboardDedupState.shouldSkipExpectedClip */
        fun shouldSkipExpectedClip(content: String): Boolean =
            ClipboardDedupState.shouldSkipExpectedClip(content)

        /** @see ClipboardDedupState.expectImageUri */
        fun expectImageUri(uri: android.net.Uri) = ClipboardDedupState.expectImageUri(uri)

        /** @see ClipboardDedupState.shouldSkipExpectedImageUri */
        fun shouldSkipExpectedImageUri(uri: android.net.Uri): Boolean =
            ClipboardDedupState.shouldSkipExpectedImageUri(uri)

        /**
         * Zero the cross-listener dedup window. Call after [clearAll] so a re-copy
         * of the same text immediately after a clear is stored as a fresh row rather
         * than silently skipped as a recent duplicate.
         * @see ClipboardDedupState.resetDedupState
         */
        fun resetDedupState() = ClipboardDedupState.resetDedupState()

        /** @see ClipboardDedupState.isNewSourceId */
        fun isNewSourceId(sourceId: String, seen: Set<String>): Boolean =
            ClipboardDedupState.isNewSourceId(sourceId, seen)

        // -- Parse cache -- delegates to ClipboardItemCache -------------------

        const val PREVIEW_MAX_CHARS = 140
        const val UNABLE_TO_PREVIEW = "(unable to preview)"

        /**
         * Evict a single id from the parse cache.
         * @see ClipboardItemCache.evictParseCache
         */
        fun evictParseCache(id: String) = ClipboardItemCache.evictParseCache(id)

        /** Evict ALL entries -- call on clearAll / clearUnpinned.
         * @see ClipboardItemCache.evictAllParseCache
         */
        fun evictAllParseCache() = ClipboardItemCache.evictAllParseCache()

        /** @see ClipboardBlobCodec.previewFromPlaintext */
        fun previewFromPlaintext(text: String): String =
            ClipboardBlobCodec.previewFromPlaintext(text)

        /** @see ClipboardBlobCodec.localAesDecrypt */
        fun localAesDecrypt(ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray =
            ClipboardBlobCodec.localAesDecrypt(ciphertext, nonce, key)

        /** @see ClipboardBlobCodec.localAesEncrypt */
        fun localAesEncrypt(plaintext: ByteArray, key: ByteArray): EncryptedBlob =
            ClipboardBlobCodec.localAesEncrypt(plaintext, key)
    }

    /**
     * Decrypt ALL locally stored items into [uniffi.copypaste_android.LocalItem]
     * values for a P2P/cloud sync push.
     *
     * No arbitrary count cap is applied. The only legitimate size bound is the
     * byte-cap retention (items are pruned when local storage exceeds the
     * configured byte limit), which already runs at capture/load time. The sync
     * layer deduplicates via LWW/Lamport, so re-offering previously-synced items
     * is cheap and guarantees full convergence between devices.
     *
     * For `content_type == "file"` items: the stored plaintext is a human-readable
     * label (e.g. "[file: report.pdf]"). The actual bytes are loaded from the
     * file-bytes sidecar store ([getFileBytes]) and used as the FFI `plaintext`
     * so the peer receives the real file content. File metadata is attached via
     * [getFileMeta] into the new ABI-8 `fileName`/`mime` fields. Items whose
     * file-bytes sidecar is missing (e.g. storage failure at capture) are skipped.
     */
    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    suspend fun localItemsForSync(
        key: ByteArray,
    ): List<uniffi.copypaste_android.LocalItem> = localItemsForSyncImpl(key)

    /**
     * Apply an inbound soft-delete tombstone (from relay, P2P, or cloud) with LWW
     * semantics.
     *
     * Two cases:
     *  1. **Item known locally**: tombstone iff incoming [lamportTs] is STRICTLY
     *     greater than the stored row's lamport_ts (newer remote delete wins; a stale
     *     re-sync cannot resurrect a re-pinned item).
     *  2. **Item unknown locally (delete-before-create)**: insert a ghost tombstone
     *     so that a later arriving create for the same [itemId] loses the LWW
     *     comparison. Mirrors daemon relay.rs `insert_tombstone` ~lines 924-940
     *     (CopyPaste-bfiu). The ghost tombstone is invisible in the UI
     *     (isDeletedBlob → filtered by getItems).
     *
     * If the stored lamport_ts >= [lamportTs] (known-item case) → no-op (local
     * state is at least as new).
     *
     * Returns true when a tombstone was written (for caller stats).
     */
    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    suspend fun applyInboundTombstoneWithLww(
        itemId: String,
        lamportTs: Long,
    ): Boolean = applyInboundTombstoneWithLwwImpl(itemId, lamportTs)

    /**
     * DEAD CODE — relay incoming sync is disabled.
     * Use [FgsSyncLoop.poll] (via [SyncManager.pollFromSupabase]) for incoming cloud sync.
     * @throws UnsupportedOperationException always — to surface accidental callers.
     */
    @Deprecated(
        message = "Relay incoming sync is disabled: items were encrypted with the sender's " +
            "local per-device key that no other device holds, making every fetched payload " +
            "undecryptable. Use FgsSyncLoop (Supabase poll) for incoming cloud sync.",
        replaceWith = ReplaceWith("syncManager.pollFromSupabase()"),
        level = DeprecationLevel.ERROR,
    )
    @Suppress("UnusedParameter") // params kept for binary-compat; function is intentionally dead
    suspend fun syncItems(syncManager: SyncManager, encryptionKey: ByteArray): List<String> {
        throw UnsupportedOperationException(
            "relay cloud backend is disabled — use Supabase for cross-device cloud sync"
        )
    }

    // ── CopyPaste-8jx8: Export / Import clipboard history ────────────────────
    //
    // Export: produce a JSON file with text items' decrypted snippets and metadata.
    //   - Only TEXT content_type items are exported (binary image/file payloads are
    //     omitted — too large and not portable across devices/encryption keys).
    //   - Sensitive items are skipped to avoid leaking secrets into unencrypted files.
    //   - Pinned state is preserved so the user can round-trip their pinned clips.
    //   - Full plaintext is loaded (not just the snippet) so imports preserve content.
    //
    // Import: read the export JSON and insert each item that does not yet exist locally
    //   (deduplication is by item ID). Items are re-encrypted with the current device key.
    //
    // Format: JSON object { "version": 1, "exported_at": epochMs, "items": [ ... ] }
    //   Each item: { "id", "content_type", "snippet", "full_text", "wall_time_ms", "pinned" }
    //
    // Security:
    //   - The export JSON is PLAINTEXT. The caller (SettingsActivity) must use the
    //     storage-access-framework (SAF / ACTION_CREATE_DOCUMENT) so the user picks
    //     the destination; never auto-write to external storage without SAF.
    //   - Import uses the same storeItem() path so the new items are immediately
    //     encrypted with the device's current key.

    /**
     * Export TEXT clipboard items as a JSON string.
     *
     * Returns the JSON [String] on success. Image and file items are omitted.
     * Sensitive items (flagged by [ClipboardItem.isSensitive]) are omitted unless
     * [includeSensitive] is true — matching the macOS "Include sensitive items"
     * export toggle (CopyPaste-crh3.40). The default is false (safe default: secrets
     * stay out of plaintext export files unless the user explicitly opts in).
     *
     * [encryptionKey] is needed to decrypt stored ciphertext for the full-text field.
     * The returned JSON is plaintext — the caller must write it to a user-chosen
     * location via the Storage Access Framework (ACTION_CREATE_DOCUMENT).
     */
    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    suspend fun exportHistory(
        encryptionKey: ByteArray,
        includeSensitive: Boolean = false,
    ): String = exportHistoryImpl(encryptionKey, includeSensitive)

    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    suspend fun importHistory(json: String, encryptionKey: ByteArray): Int =
        importHistoryImpl(json, encryptionKey)
}
