package com.copypaste.android

import android.util.Base64
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Read/query path for [ClipboardRepository]: paginated history load, item counts,
 * full-plaintext decrypt, and full-content search.
 *
 * Extracted from [ClipboardRepository] (CopyPaste-vp63.33). These extension functions
 * access [ClipboardRepository]'s internal fields via the extension receiver and call
 * [ClipboardBlobCodec] directly (bypassing the removed private-wrapper aliases),
 * mirroring the existing extraction pattern from [ClipboardRepositoryPin] /
 * [ClipboardRepositoryPrune] / [ClipboardRepositorySync] (CopyPaste-ra15.4).
 */

private const val TAG = "ClipboardRepository"

/**
 * Implementation of [ClipboardRepository.getItems].
 *
 * Load history items for display, most-recent-first, with lazy pagination.
 *
 * Each stored blob is DECRYPTED with [key] so the row shows a real preview.
 * The [ClipboardItem.pinned] field is populated from the persisted [ClipboardRepository.KEY_PINNED_IDS] set.
 * Image bytes are NOT attached here — callers must use [ClipboardRepository.getImageBytes] lazily per-row.
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
internal suspend fun ClipboardRepository.getItemsImpl(
    key: ByteArray,
    limit: Int = ClipboardRepository.PAGE_SIZE,
    offset: Int = 0,
): List<ClipboardItem> =
    withContext(Dispatchers.IO) {
        // AB-13: run the retention TTL auto-wipe on the same cadence as load
        // (cheap general-age fast-path; sensitive pass only decrypts aged rows).
        pruneByAgeImpl(key)

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
                if (ClipboardBlobCodec.isDeletedBlob(raw)) return@mapNotNull null
                val item = synchronized(ClipboardItemCache.parseCacheLock) {
                    val entry = ClipboardItemCache.parseCache[id]
                    if (entry != null && entry.rawBlob == raw) entry.item else null
                } ?: run {
                    val parsed = ClipboardBlobCodec.parseItem(id, raw, key) ?: return@mapNotNull null
                    synchronized(ClipboardItemCache.parseCacheLock) {
                        ClipboardItemCache.parseCache[id] = ClipboardItemCache.ParsedEntry(raw, parsed)
                    }
                    parsed
                }
                val isPinned = id in pinnedSet
                val binaryTooLarge = when {
                    item.isImage ->
                        (prefs.getString("item_img_$id", null)?.let { ClipboardBlobCodec.base64RawByteSize(it).toLong() } ?: 0L) > ClipboardRepository.SYNC_MAX_BLOB_BYTES
                    item.isFile ->
                        (prefs.getString("item_file_$id", null)?.let { ClipboardBlobCodec.base64RawByteSize(it).toLong() } ?: 0L) > ClipboardRepository.SYNC_MAX_BLOB_BYTES
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
            if (ClipboardBlobCodec.isDeletedBlob(raw)) return@mapNotNull null
            // A: serve from parse cache when the raw blob is unchanged — avoids a
            // full AEAD decrypt + native isSensitive() for every row on every reload.
            // Only decrypt when the blob has actually been written since last load.
            val item = synchronized(ClipboardItemCache.parseCacheLock) {
                val entry = ClipboardItemCache.parseCache[id]
                if (entry != null && entry.rawBlob == raw) entry.item else null
            } ?: run {
                val parsed = ClipboardBlobCodec.parseItem(id, raw, key) ?: return@mapNotNull null
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
                    (prefs.getString("item_img_$id", null)?.let { ClipboardBlobCodec.base64RawByteSize(it).toLong() } ?: 0L) > ClipboardRepository.SYNC_MAX_BLOB_BYTES
                item.isFile ->
                    (prefs.getString("item_file_$id", null)?.let { ClipboardBlobCodec.base64RawByteSize(it).toLong() } ?: 0L) > ClipboardRepository.SYNC_MAX_BLOB_BYTES
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
 * Implementation of [ClipboardRepository.unpinnedItemCount].
 *
 * Returns the total number of non-deleted unpinned items in the store.
 * Used by the pagination logic in [ClipboardViewModel] to detect when all
 * pages have been loaded (no more items to fetch).
 * Excludes soft-delete tombstones — mirrors the isDeletedBlob() filter in
 * getItems() so the hasMore sentinel stays in sync with actual visible rows.
 */
internal fun ClipboardRepository.unpinnedItemCountImpl(): Int {
    val pinnedSet = storedPinnedIds()
    return storedIds().count { id ->
        if (id in pinnedSet) return@count false
        val raw = prefs.getString("item_$id", null) ?: return@count false
        !ClipboardBlobCodec.isDeletedBlob(raw)
    }
}

/**
 * Implementation of [ClipboardRepository.totalItemCount].
 *
 * Total number of stored items (pinned + unpinned).
 * Used by the history header count display (parity with macOS).
 * Excludes soft-delete tombstones that remain in KEY_ITEM_IDS to prevent
 * re-sync resurrection — mirrors the isDeletedBlob() filter in getItems().
 */
internal fun ClipboardRepository.totalItemCountImpl(): Int = storedIds().count { id ->
    val raw = prefs.getString("item_$id", null) ?: return@count false
    !ClipboardBlobCodec.isDeletedBlob(raw)
}

/**
 * Implementation of [ClipboardRepository.loadFullPlaintext].
 *
 * Decrypt and return the FULL plaintext for item [id], or null when the item
 * does not exist or cannot be decrypted.
 *
 * Used by the copy-to-clipboard path in [HistoryActivity] to ensure the user
 * copies the complete original text, not the 140-char [ClipboardItem.snippet].
 */
internal suspend fun ClipboardRepository.loadFullPlaintextImpl(id: String, key: ByteArray): String? =
    withContext(Dispatchers.IO) {
        loadFullPlaintextBlockingImpl(id, key)
    }

/**
 * Full-content search. Returns the subset of [ids] whose FULL text matches [query].
 *
 * Implementation of [ClipboardRepository.searchIds].
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
internal suspend fun ClipboardRepository.searchIdsImpl(ids: List<String>, query: String, key: ByteArray): Set<String> =
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
                    val full = loadFullPlaintextBlockingImpl(id, key)
                    full != null && full.contains(q, ignoreCase = true)
                }
            }
        }

        // Fallback: O(N) full-decrypt scan (stub mode / no live .so).
        ids.filterTo(HashSet()) { id ->
            val full = loadFullPlaintextBlockingImpl(id, key)
            full != null && full.contains(q, ignoreCase = true)
        }
    }

/**
 * Implementation of [ClipboardRepository.loadFullPlaintextBlocking].
 *
 * Synchronous full-plaintext decrypt for use inside an already-`IO` context
 * (e.g. [searchIdsImpl]). Mirrors [loadFullPlaintextImpl] without an extra dispatch.
 */
internal fun ClipboardRepository.loadFullPlaintextBlockingImpl(id: String, key: ByteArray): String? {
    val raw = prefs.getString("item_$id", null) ?: return null
    val parts = raw.split("|")
    val nonceB64 = parts.getOrNull(3) ?: return null
    val ctB64 = parts.getOrNull(4) ?: return null
    return try {
        val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
        val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
        ClipboardBlobCodec.decryptForPreview(id, ciphertext, nonce, key, ClipboardBlobCodec.keyVersionFromParts(parts))
    } catch (e: Exception) {
        Log.d(TAG, "loadFullPlaintextBlocking: decrypt failed for $id: ${e.message}")
        null
    }
}
