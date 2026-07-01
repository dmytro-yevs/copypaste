package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Delete / clear / reset path for [ClipboardRepository]: single-item soft-delete,
 * bulk delete, clear-all, clear-unpinned, and the destructive full-store reset.
 *
 * Extracted from [ClipboardRepository] (CopyPaste-vp63.33). These extension functions
 * access [ClipboardRepository]'s internal fields via the extension receiver and call
 * [ClipboardBlobCodec] / [ClipboardItemCache] / [ClipboardDedupState] directly
 * (bypassing the removed private-wrapper aliases), mirroring the existing extraction
 * pattern from [ClipboardRepositoryPin] / [ClipboardRepositoryPrune] /
 * [ClipboardRepositorySync] (CopyPaste-ra15.4).
 *
 * NOTE: [deleteItemImpl] writes a soft-delete tombstone IN PLACE (the id stays in
 * [ClipboardRepository.KEY_ITEM_IDS] with a tombstone blob) so a re-sync cannot
 * resurrect it. [deleteItemsImpl] / [clearAllImpl] / [clearUnpinnedImpl] instead
 * HARD-remove the id and its blob from local storage, relying solely on the
 * enqueued [OutboundMutationQueue] record to inform peers. These are genuinely
 * different local persistence strategies (not just cosmetic variations of the
 * same body) — a single shared "tombstoneAndEnqueue" helper across all four would
 * risk silently changing one path's on-disk behaviour, so only the
 * provably-identical lamport-bump arithmetic ([oldLamportOf]) is deduplicated here.
 */

private const val TAG = "ClipboardRepository"

/** Shared `oldLamport` parse — identical body previously inlined at every delete call site. */
private fun oldLamportOf(raw: String?): Long =
    try {
        raw?.split("|")?.getOrNull(5)?.toLongOrNull() ?: 0L
    } catch (_: Exception) {
        0L
    }

/**
 * Implementation of [ClipboardRepository.deleteItem].
 */
internal suspend fun ClipboardRepository.deleteItemImpl(id: String): Boolean = withContext(Dispatchers.IO) {
    val tombstoneResult: Pair<Boolean, Long> = synchronized(idsWriteLock) {
        val ids = storedIds()
        if (id !in ids) return@synchronized false to 0L
        val existing = prefs.getString("item_$id", null) ?: return@synchronized false to 0L
        // Already a tombstone — nothing to do.
        if (ClipboardBlobCodec.isDeletedBlob(existing)) return@synchronized false to 0L

        val pinnedList = storedPinnedList().toMutableList()
        val wasPinned = pinnedList.remove(id)

        // Write a soft-delete tombstone: bump lamportTs to max(prev+1, nowMs) so
        // the tombstone is time-ordered into wall-clock space (CopyPaste-up1c),
        // preventing collisions with low-magnitude lamport values from older peers.
        // Mirrors next_lamport_ts() in copypaste-core/src/storage/items.rs ~line 68.
        val oldLamport = oldLamportOf(existing)
        val newLamport = nextLamportTs(oldLamport, System.currentTimeMillis())
        val tombstone = ClipboardBlobCodec.encodeTombstone(existing, newLamport)

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
            editor.putString(ClipboardRepository.KEY_PINNED_IDS, pinnedList.joinToString(","))
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
        ClipboardItemCache.evictParseCache(id) // A: evict stale decrypt cache entry (blob is now a tombstone)
        ClipboardService.onItemsDeleted(appContext, 1)
    }
    tombstoned
}

/**
 * Implementation of [ClipboardRepository.deleteItems].
 *
 * Bulk-delete items by [ids]. Items not present in the index are silently
 * skipped. Pinned state is cleaned up for any deleted ids. Image blobs are
 * removed alongside the item entry.
 */
internal fun ClipboardRepository.deleteItemsImpl(ids: List<String>) {
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
            if (ClipboardBlobCodec.isDeletedBlob(raw)) continue
            val oldLamport = oldLamportOf(raw)
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
            .putString(ClipboardRepository.KEY_ITEM_IDS, storedList.joinToString(","))
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
            editor.putString(ClipboardRepository.KEY_PINNED_IDS, pinnedList.joinToString(","))
        }
        editor.apply()
    }
    if (deletedCount > 0) {
        // A: evict deleted ids from the decrypt cache so stale entries don't linger.
        for (id in toDelete) ClipboardItemCache.evictParseCache(id)
        ClipboardService.onItemsDeleted(appContext, deletedCount)
    }
    Log.d(TAG, "deleteItems: removed $deletedCount items")
}

/**
 * Implementation of [ClipboardRepository.clearAll].
 *
 * Delete all UNPINNED items (text blobs + image blobs + synced-source-id set).
 * Pinned items are preserved — mirrors the macOS daemon `DELETE WHERE pinned = 0`
 * fix (HW-A13). Previously this wiped everything including pinned items;
 * the behaviour is now consistent across platforms so no user-pinned clip is
 * ever silently removed by a "clear" action.
 */
internal fun ClipboardRepository.clearAllImpl() {
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
            if (ClipboardBlobCodec.isDeletedBlob(raw)) continue
            val oldLamport = oldLamportOf(raw)
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
            .putString(ClipboardRepository.KEY_ITEM_IDS, remaining.joinToString(","))
            .remove(ClipboardRepository.KEY_SYNCED_SOURCE_IDS)
            .apply()
    }
    // Reset cross-listener dedup state so a re-copy after a clear stores a
    // fresh row instead of being silently skipped as a duplicate.
    ClipboardDedupState.resetDedupState()
    if (deletedCount > 0) {
        ClipboardItemCache.evictAllParseCache() // A: full cache wipe — most entries are now gone
        ClipboardService.onItemsDeleted(appContext, deletedCount)
    }
    Log.d(TAG, "clearAll: deleted $deletedCount unpinned items (pinned items preserved)")
}

/**
 * Implementation of [ClipboardRepository.resetDatabase].
 *
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
 * Unlike [clearAllImpl] which preserves pinned items and queues sync tombstones, this
 * wipes the SharedPreferences file entirely and resets all in-memory state. It does
 * NOT wipe [Settings] — user preferences (encryption key, device id, etc.) are
 * preserved so the device can continue to participate in sync after recovery.
 *
 * @param confirmed Must be `true` to proceed. Pass `false` to no-op (e.g. if the caller
 *   has not yet obtained user confirmation). Throws [IllegalArgumentException] if the
 *   parameter is omitted or false, to make silent misuse immediately visible in tests.
 */
internal fun ClipboardRepository.resetDatabaseImpl(confirmed: Boolean) {
    require(confirmed) {
        "resetDatabase must only be called with confirmed=true after an explicit user " +
            "confirmation dialog — this operation is irreversible and wipes all history."
    }
    synchronized(idsWriteLock) {
        // Wipe all item data in the repository SharedPreferences file.
        prefs.edit().clear().apply()
        // Reset in-memory parse cache so no stale entries linger after the wipe.
        ClipboardItemCache.evictAllParseCache()
        // Invalidate the id cache — prefs.clear() removed KEY_ITEM_IDS.
        ClipboardItemCache.cachedIds = emptyList()
        // Reset dedup state so the first captured item after reset is stored fresh.
        ClipboardDedupState.resetDedupState()
    }
    Log.w(TAG, "resetDatabase: clipboard SharedPreferences wiped (recovery action, confirmed=true)")
    // Notify the service that items were deleted so the persistent notification counter resets.
    ClipboardService.onItemsDeleted(appContext, 0)
}

/**
 * Implementation of [ClipboardRepository.clearUnpinned].
 *
 * Delete all UNPINNED items (text blobs + image blobs). Pinned items remain.
 * The synced-source-id set is also cleared (re-syncing pinned items is fine).
 */
internal fun ClipboardRepository.clearUnpinnedImpl() {
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
            if (ClipboardBlobCodec.isDeletedBlob(raw)) continue
            val oldLamport = oldLamportOf(raw)
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
            .putString(ClipboardRepository.KEY_ITEM_IDS, remaining.joinToString(","))
            .remove(ClipboardRepository.KEY_SYNCED_SOURCE_IDS)
            .apply()
    }
    if (deletedCount > 0) {
        ClipboardItemCache.evictAllParseCache() // A: full cache wipe — most entries are now gone
        ClipboardService.onItemsDeleted(appContext, deletedCount)
    }
    Log.d(TAG, "clearUnpinned: all unpinned items deleted")
}
