package com.copypaste.android

import android.util.Log

/**
 * Pin-management helpers for [ClipboardRepository].
 *
 * Extracted from [ClipboardRepository] (CopyPaste-ra15.4). These extension functions
 * access [ClipboardRepository]'s internal fields via the extension receiver and call
 * [ClipboardBlobCodec] / [ClipboardItemCache] directly.
 */

private const val TAG = "ClipboardRepository"

/**
 * Implementation of [ClipboardRepository.setPinned].
 *
 * Pin or unpin item [id]. Pinned items survive the retention prune pass and have no
 * sensitive TTL. Newly pinned items are prepended to the pinned list so they appear
 * at the top of the pinned section.
 */
internal fun ClipboardRepository.setPinnedImpl(id: String, pinned: Boolean) {
    synchronized(idsWriteLock) {
        val pinnedList = storedPinnedList().toMutableList()
        val changed = if (pinned) {
            if (id !in pinnedList) {
                pinnedList.add(0, id) // prepend — new pins appear at the top
                true
            } else false
        } else {
            pinnedList.remove(id)
        }
        if (changed) {
            // CopyPaste-up1c: bump lamport_ts in the stored blob so pin changes
            // are time-ordered into wall-clock space and propagate correctly via
            // LWW. Mirrors pin_item/unpin_item in copypaste-core/src/storage/items.rs
            // which call next_lamport_ts on every pin mutation.
            val existing = prefs.getString("item_$id", null)
            val nowMs = System.currentTimeMillis()
            val editor = prefs.edit()
                .putString(ClipboardRepository.KEY_PINNED_IDS, pinnedList.joinToString(","))
            var newLamport = nowMs
            if (existing != null && !ClipboardBlobCodec.isDeletedBlob(existing)) {
                val oldLamport = try {
                    existing.split("|").getOrNull(5)?.toLongOrNull() ?: 0L
                } catch (_: Exception) { 0L }
                newLamport = nextLamportTs(oldLamport, nowMs)
                editor.putString("item_$id", ClipboardBlobCodec.bumpBlobLamportTs(existing, newLamport))
            }
            // CopyPaste-0qpn: enqueue the pin/unpin mutation so it propagates
            // over relay, Supabase, and P2P. The pinOrder is the new position in
            // the pinned list (1-based for the macOS daemon convention).
            val newPinOrder: Double? = if (pinned) {
                (pinnedList.indexOf(id) + 1).toDouble().takeIf { it > 0.0 }
            } else null
            OutboundMutationQueue.enqueueMutation(
                appContext,
                OutboundMutationQueue.MutationRecord(
                    itemId = id,
                    op = if (pinned) OutboundMutationQueue.OP_PIN else OutboundMutationQueue.OP_UNPIN,
                    lamportTs = newLamport,
                    wallTimeMs = nowMs,
                    pinned = pinned,
                    pinOrder = newPinOrder,
                ),
            )
            // commit() (synchronous) so the new pinned set survives an immediate
            // force-stop (SIGKILL) — matches the project pattern from 0f1d1ef.
            editor.commit()
            ClipboardItemCache.evictParseCache(id) // blob changed — evict stale decrypt cache entry
        }
    }
    Log.d(TAG, "setPinned: item $id pinned=$pinned")
}

/**
 * Implementation of [ClipboardRepository.reorderPinned].
 *
 * Reorder pinned items. [ids] must contain exactly the currently-pinned item IDs in the
 * desired new display order (first element = top of the pinned section).
 */
internal fun ClipboardRepository.reorderPinnedImpl(ids: List<String>) {
    synchronized(idsWriteLock) {
        val currentPinned = storedPinnedList().toMutableSet()
        // Accept only ids that are actually pinned; preserve order from caller.
        val reordered = ids.filter { it in currentPinned }.toMutableList()
        // Append any pinned ids that were not included in the caller's list.
        val missing = currentPinned.filter { it !in reordered }
        reordered.addAll(missing)

        // CopyPaste-up1c: bump lamport_ts in every reordered blob so the new
        // pin-order propagates correctly via LWW. Mirrors reorder_pinned in
        // copypaste-core/src/storage/items.rs which calls next_lamport_ts per item.
        val nowMs = System.currentTimeMillis()
        val editor = prefs.edit()
            .putString(ClipboardRepository.KEY_PINNED_IDS, reordered.joinToString(","))
        for ((idx, itemId) in reordered.withIndex()) {
            val existing = prefs.getString("item_$itemId", null) ?: continue
            if (ClipboardBlobCodec.isDeletedBlob(existing)) continue
            val oldLamport = try {
                existing.split("|").getOrNull(5)?.toLongOrNull() ?: 0L
            } catch (_: Exception) { 0L }
            val newLamport = nextLamportTs(oldLamport, nowMs)
            editor.putString("item_$itemId", ClipboardBlobCodec.bumpBlobLamportTs(existing, newLamport))
            ClipboardItemCache.evictParseCache(itemId) // blob changed — evict stale decrypt cache entry

            // CopyPaste-0qpn: enqueue per-item OP_REORDER so the new pin order
            // propagates over all transports. pinOrder is 1-based (macOS daemon).
            OutboundMutationQueue.enqueueMutation(
                appContext,
                OutboundMutationQueue.MutationRecord(
                    itemId = itemId,
                    op = OutboundMutationQueue.OP_REORDER,
                    lamportTs = newLamport,
                    wallTimeMs = nowMs,
                    pinned = true,
                    pinOrder = (idx + 1).toDouble(),
                ),
            )
        }
        // commit() (synchronous) so the reordered set survives an immediate
        // force-stop (SIGKILL) — matches the project pattern from 0f1d1ef.
        editor.commit()
    }
    Log.d(TAG, "reorderPinned: new order = $ids")
}

/**
 * Implementation of [ClipboardRepository.applyAuthoritativePinState].
 *
 * Apply authoritative pin state from an inbound sync row without minting a new local
 * mutation. The remote lamport_ts is already authoritative.
 */
internal fun ClipboardRepository.applyAuthoritativePinStateImpl(
    id: String,
    pinned: Boolean,
    pinOrder: Double?,
) {
    synchronized(idsWriteLock) {
        val pinnedList = storedPinnedList().toMutableList()
        @Suppress("UNUSED_VARIABLE") // wasPinned is semantically meaningful even if unused below
        val wasPinned = id in pinnedList
        if (pinned) {
            // Remove from current position (if present) and re-insert at the
            // correct pin_order slot so a remote reorder converges.
            pinnedList.remove(id)
            if (pinOrder != null) {
                val insertAt = pinnedList.size  // default: append
                pinnedList.add(insertAt.coerceAtMost(pinnedList.size), id)
            } else {
                if (id !in pinnedList) pinnedList.add(id)
            }
        } else {
            // Authoritative unpin: remove regardless of local state.
            pinnedList.remove(id)
        }
        val changed = pinnedList != storedPinnedList()
        if (changed) {
            // Do NOT bump lamport_ts here — this is not a local mutation.
            prefs.edit()
                .putString(ClipboardRepository.KEY_PINNED_IDS, pinnedList.joinToString(","))
                .apply()
        }
    }
    Log.d(TAG, "applyAuthoritativePinState: item $id pinned=$pinned pinOrder=$pinOrder")
}

/**
 * Implementation of [ClipboardRepository.bumpToTop].
 *
 * Re-stamp [id] as the most-recently-used item (copy-back). Bumps BOTH the wall-time
 * (field 0) AND the lamport timestamp (field 5) so the item wins LWW merges on remote peers.
 *
 * Returns the new lamport timestamp, or -1L when the item was not found, pinned, or deleted.
 */
internal fun ClipboardRepository.bumpToTopImpl(id: String): Long {
    val newLamport: Long
    synchronized(idsWriteLock) {
        if (id in storedPinnedIds()) return -1L  // pinned items keep their fixed order
        val ids = storedIds().toMutableList()
        if (!ids.remove(id)) return -1L  // unknown id — nothing to bump
        val raw = prefs.getString("item_$id", null) ?: return -1L
        // Soft-delete tombstone: tombstoned items must not be bumped to the top
        // of the visible history — they are logically deleted.
        if (ClipboardBlobCodec.isDeletedBlob(raw)) return -1L
        val parts = raw.split("|")
        // v3 blob = <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>|<deleted>
        if (parts.size < 6) return -1L  // legacy/malformed — leave untouched
        val nowMs = System.currentTimeMillis()
        val prevLamport = parts[5].toLongOrNull() ?: 0L
        // cvns: advance lamport so this copy-back wins LWW on all peers.
        newLamport = nextLamportTs(prevLamport, nowMs)
        val rebuiltParts = parts.toMutableList()
        rebuiltParts[0] = nowMs.toString()    // fresh wall-time
        rebuiltParts[5] = newLamport.toString() // bumped lamport
        val rebuilt = rebuiltParts.joinToString("|")
        ids.add(id)  // re-append → most-recent position
        ClipboardItemCache.cachedIds = ids
        prefs.edit()
            .putString("item_$id", rebuilt)
            .putString(ClipboardRepository.KEY_ITEM_IDS, ids.joinToString(","))
            .commit()  // synchronous: survives an immediate force-stop (SIGKILL)
    }
    Log.d(TAG, "bumpToTop: re-stamped item $id to most-recent (lamport=$newLamport)")
    return newLamport
}
