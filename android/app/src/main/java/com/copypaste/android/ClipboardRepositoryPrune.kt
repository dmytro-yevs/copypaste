package com.copypaste.android

import android.util.Log

/**
 * Pruning helpers for [ClipboardRepository].
 *
 * Extracted from [ClipboardRepository] (CopyPaste-ra15.4). These extension functions
 * access [ClipboardRepository]'s internal fields via the extension receiver and call
 * [ClipboardBlobCodec] directly (bypassing the thin private-wrapper aliases in the class).
 */

private const val TAG = "ClipboardRepository"

/**
 * Enforce the storage-quota byte cap and the [Settings.maxHistoryItems] count cap
 * by evicting the oldest UNPINNED items.
 *
 * Implementation of [ClipboardRepository.pruneToLimits] and
 * [ClipboardRepository.applyHistoryCap]. See the callers for full documentation.
 */
internal fun ClipboardRepository.pruneToLimitsImpl() {
    val quotaBytes = settings.storageQuotaBytes.coerceAtLeast(0L)
    // Settings.maxHistoryItems default 1000; coerceAtLeast(1) guards against
    // a persisted 0 that would evict everything including pinned items.
    val maxItems = settings.maxHistoryItems.coerceAtLeast(1)
    var evictedCount = 0

    synchronized(idsWriteLock) {
        val pinnedSet = storedPinnedIds()
        val ids = storedIds().toMutableList()

        val blobSizes: Map<String, Int> = ids.associate { id ->
            val textBytes = prefs.getString("item_$id", null)?.toByteArray(Charsets.UTF_8)?.size ?: 0
            // Measure the SAME unit storeImageBytes caps on: raw decoded bytes,
            // not the ~1.33x-inflated Base64 string length.
            val imgBytes = prefs.getString("item_img_$id", null)
                ?.let { ClipboardBlobCodec.base64RawByteSize(it) } ?: 0
            val thumbBytes = prefs.getString("item_thumb_$id", null)
                ?.let { ClipboardBlobCodec.base64RawByteSize(it) } ?: 0
            val fileBytes = prefs.getString("item_file_$id", null)
                ?.let { ClipboardBlobCodec.base64RawByteSize(it) } ?: 0
            id to (textBytes + imgBytes + thumbBytes + fileBytes)
        }

        var totalBytes = blobSizes.values.sumOf { it.toLong() }
        // LIVE (non-tombstone) ids: tombstones remain in KEY_ITEM_IDS to block
        // re-sync resurrection, but must NOT be re-counted or re-evicted.
        val liveIds = ids.filter { id ->
            val raw = prefs.getString("item_$id", null)
            raw != null && !ClipboardBlobCodec.isDeletedBlob(raw)
        }
        var liveCount = liveIds.size
        val unpinned = liveIds.filter { it !in pinnedSet }.toMutableList()

        val editor = prefs.edit()
        var didEvict = false
        val nowMs = System.currentTimeMillis()

        // CopyPaste-44rq.58: tombstone-evict instead of hard-delete so peers learn
        // about the eviction and do NOT re-push the item on the next sync.
        //
        // INVARIANTS:
        //  1. Tombstone blob written to "item_$evictId" (deleted=1, bumped lamportTs).
        //  2. "item_id_ref_$evictId" NOT removed — storeItemWithLww uses it to reject
        //     stale re-syncs via lamport comparison (resurrection prevention).
        //  3. Evicted id NOT removed from KEY_ITEM_IDS — tombstone must stay in index.
        //  4. OP_DELETE mutation enqueued so peers learn of the eviction.
        fun tombstoneEvict(evictId: String) {
            val existing = prefs.getString("item_$evictId", null)
            val oldLamport = try {
                existing?.split("|")?.getOrNull(5)?.toLongOrNull() ?: 0L
            } catch (_: Exception) { 0L }
            val newLamport = nextLamportTs(oldLamport, nowMs)
            if (existing != null && !ClipboardBlobCodec.isDeletedBlob(existing)) {
                editor.putString("item_$evictId", ClipboardBlobCodec.encodeTombstone(existing, newLamport))
            }
            OutboundMutationQueue.enqueueMutation(
                appContext,
                OutboundMutationQueue.MutationRecord(
                    itemId = evictId,
                    op = OutboundMutationQueue.OP_DELETE,
                    lamportTs = newLamport,
                    wallTimeMs = nowMs,
                    pinned = false,
                    pinOrder = null,
                ),
            )
            editor.remove("item_img_$evictId")
            editor.remove("item_thumb_$evictId")
            editor.remove("item_file_$evictId")
            editor.remove("item_filemeta_$evictId")
            // Do NOT remove "item_id_ref_$evictId": storeItemWithLww uses this ref to
            // find the tombstone blob. Removing it caused resurrection (CopyPaste-44rq.58).
            ClipboardItemCache.evictParseCache(evictId)
        }

        // Pass 1: byte-quota eviction (oldest unpinned first).
        // Keep evicted ids in `ids` so their tombstone blobs remain in KEY_ITEM_IDS
        // (mirrors deleteItem's single-item path which never removes from KEY_ITEM_IDS).
        while (unpinned.isNotEmpty()) {
            val quotaExceeded = quotaBytes > 0 && totalBytes > quotaBytes
            if (!quotaExceeded) break

            val evictId = unpinned.removeAt(0)
            // Do NOT remove from ids — the tombstone must stay in KEY_ITEM_IDS.
            val sz = blobSizes[evictId] ?: 0
            totalBytes -= sz
            tombstoneEvict(evictId)
            liveCount-- // byte-pass eviction also reduces the live count
            didEvict = true
            evictedCount++
            Log.d(TAG, "pruneToLimits: evicted $evictId (blob ${sz}B, totalNow=${totalBytes}B)")
        }

        // Pass 2: count-cap eviction (crh3.108 continuous enforcement). Evicts the
        // OLDEST unpinned items until the LIVE item count is <= maxItems. Mirrors the
        // pure [planCountCapEvictions] planner that also drives the Settings confirmation
        // count (bdac.88), so the predicted and actual deletion counts always agree.
        //
        // `liveCount` is the running live total (initialised above, decremented by
        // Pass 1); `ids` itself is NOT reduced so tombstone entries remain in
        // KEY_ITEM_IDS (see INVARIANT 3).
        while (unpinned.isNotEmpty() && liveCount > maxItems) {
            val evictId = unpinned.removeAt(0)
            // Do NOT remove from ids — the tombstone must stay in KEY_ITEM_IDS.
            liveCount--
            tombstoneEvict(evictId)
            didEvict = true
            evictedCount++
            Log.d(TAG, "pruneToLimits: count-evicted $evictId (live now $liveCount/$maxItems)")
        }

        if (didEvict) {
            ClipboardItemCache.cachedIds = ids
            editor.putString(ClipboardRepository.KEY_ITEM_IDS, ids.joinToString(",")).apply()
        }
    }

    if (evictedCount > 0) {
        ClipboardService.onItemsDeleted(appContext, evictedCount)
    }
}

/**
 * AB-13 — retention TTL auto-wipe (macOS parity).
 *
 * Implementation of [ClipboardRepository.pruneByAge]. See the caller for full documentation.
 */
internal fun ClipboardRepository.pruneByAgeImpl(key: ByteArray? = null) {
    val generalTtlSecs = generalTtlSecs().coerceAtLeast(0L)
    val sensitiveTtlSecs = settings.sensitiveTtlSecs.coerceAtLeast(0L)
    if (generalTtlSecs == 0L && sensitiveTtlSecs == 0L) return // both disabled

    val now = System.currentTimeMillis()
    val generalCutoffMs = if (generalTtlSecs > 0L) now - generalTtlSecs * 1000L else Long.MIN_VALUE
    val sensitiveCutoffMs = if (sensitiveTtlSecs > 0L) now - sensitiveTtlSecs * 1000L else Long.MIN_VALUE
    var deletedCount = 0

    synchronized(idsWriteLock) {
        val pinnedSet = storedPinnedIds()
        val ids = storedIds()
        val editor = prefs.edit()
        val survivors = ArrayList<String>(ids.size)

        for (id in ids) {
            if (id in pinnedSet) {
                survivors.add(id) // pinned items never age out
                continue
            }
            val raw = prefs.getString("item_$id", null)
            if (raw == null) {
                // Index references a missing blob — drop the dangling id.
                continue
            }
            val wallTimeMs = raw.substringBefore('|').toLongOrNull()
            if (wallTimeMs == null) {
                survivors.add(id) // malformed — leave it for the normal prune
                continue
            }

            // General retention: oldest-first absolute age cap.
            val expiredByGeneral = generalTtlSecs > 0L && wallTimeMs < generalCutoffMs

            // Sensitive retention: only decrypt+classify items already past the
            // sensitive window (cheap fast-path skips the vast majority of rows).
            val expiredBySensitive = sensitiveTtlSecs > 0L &&
                wallTimeMs < sensitiveCutoffMs &&
                ClipboardBlobCodec.isItemSensitive(id, raw, key)

            if (expiredByGeneral || expiredBySensitive) {
                editor.remove("item_$id")
                editor.remove("item_img_$id")
                editor.remove("item_thumb_$id")
                editor.remove("item_file_$id")
                editor.remove("item_filemeta_$id")
                editor.remove("item_id_ref_$id")
                deletedCount++
                Log.d(TAG, "pruneByAge: wiped $id (general=$expiredByGeneral, sensitive=$expiredBySensitive)")
            } else {
                survivors.add(id)
            }
        }

        if (deletedCount > 0) {
            ClipboardItemCache.cachedIds = survivors
            editor.putString(ClipboardRepository.KEY_ITEM_IDS, survivors.joinToString(",")).apply()
        }
    }

    if (deletedCount > 0) {
        ClipboardService.onItemsDeleted(appContext, deletedCount)
    }
}
