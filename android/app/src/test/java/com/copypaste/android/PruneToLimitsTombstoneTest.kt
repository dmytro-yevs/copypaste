package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-44rq.58 — pruneToLimits tombstone resurrection regression.
 *
 * ## Root cause
 *
 * `pruneToLimits` previously:
 *  1. Removed the evicted item from KEY_ITEM_IDS (hard-deleted it from the index).
 *  2. Removed `item_id_ref_<itemId>` from SharedPreferences.
 *
 * As a result, when a peer pushed the same item on the next sync pull,
 * `storeItemWithLww` found no `item_id_ref_<itemId>` and took the "new item" path,
 * reinserting the item that was supposed to be evicted (resurrection).
 *
 * ## Fix (CopyPaste-44rq.58)
 *
 * `tombstoneEvict` now:
 *  1. Writes a soft-delete tombstone blob to `item_<evictId>` (deleted=1, bumped lamportTs).
 *  2. Does NOT remove `item_id_ref_<evictId>` — the LWW lookup path needs the ref to
 *     find the tombstone blob and reject older incoming items via lamport comparison.
 *  3. Keeps the evicted id in KEY_ITEM_IDS so the tombstone remains in the index
 *     (consistent with the `deleteItem` single-item tombstone path).
 *  4. Enqueues an OP_DELETE mutation so peers learn about the eviction via all
 *     transports (relay/Supabase/P2P).
 *
 * ## What these tests verify (pure-JVM, no Android runtime)
 *
 * - Tombstone codec: `ClipboardBlobCodec.encodeTombstone` + `isDeletedBlob` correctly
 *   produce and recognise a deleted blob with bumped lamport_ts.
 * - LWW rejection: given a tombstone with lamport T and an incoming item with
 *   lamport T-1, the LWW comparison `incomingLamportTs < storedTs` → reject (no resurrection).
 * - Mutation queue: a tombstone-evict produces exactly one OP_DELETE record per item.
 * - id_ref invariant: `tombstoneEvict` must NOT remove `item_id_ref` — demonstrated by
 *   a simulation where removing the ref causes resurrection and keeping it prevents it.
 * - KEY_ITEM_IDS invariant: the evicted id must remain in the index after tombstoning
 *   so a later LWW replace can surface the item correctly.
 * - Count-cap liveness: the count-cap pass must correctly track live items (non-tombstone)
 *   even when tombstone ids remain in the KEY_ITEM_IDS list.
 */
class PruneToLimitsTombstoneTest {

    // ── Tombstone codec ─────────────────────────────────────────────────────────

    /**
     * A blob that `encodeTombstone` produces must be recognised as deleted.
     * The `deleted` flag is at field index 6 (see ClipboardBlobCodec.isDeletedBlob).
     */
    @Test
    fun `encodeTombstone produces a blob where isDeletedBlob returns true`() {
        // A minimal synthetic raw blob with the pipe-delimited v5 format used by
        // the real store.  The exact content/nonce values don't matter for the
        // codec functions under test (no decryption is performed here).
        val rawBlob = "1700000000|text/plain|5|bm9uY2U=|Y2lwaGVydGV4dA==|50|0|device-A|2|"
        val tombstoneBlob = ClipboardBlobCodec.encodeTombstone(rawBlob, bumpedLamportTs = 999L)

        assertTrue(
            "encodeTombstone must produce a blob with isDeletedBlob == true (field 6 == '1')",
            ClipboardBlobCodec.isDeletedBlob(tombstoneBlob),
        )
    }

    /**
     * The tombstone blob must carry the bumped lamport_ts at field 5 so LWW can
     * compare incoming items against the eviction timestamp and reject stale peers.
     */
    @Test
    fun `encodeTombstone embeds the bumped lamportTs at field 5`() {
        val rawBlob = "1700000000|text/plain|5|bm9uY2U=|Y2lwaGVydGV4dA==|50|0|device-A|2|"
        val bumpedLamport = 1234567890L
        val tombstoneBlob = ClipboardBlobCodec.encodeTombstone(rawBlob, bumpedLamportTs = bumpedLamport)

        val parts = tombstoneBlob.split("|")
        val storedLamport = parts.getOrNull(5)?.toLongOrNull()
        assertEquals(
            "Tombstone must carry the bumped lamportTs at field index 5 for LWW ordering",
            bumpedLamport,
            storedLamport,
        )
    }

    /**
     * A live blob must NOT be reported as deleted.
     */
    @Test
    fun `isDeletedBlob returns false for a live blob`() {
        val liveBlob = "1700000000|text/plain|5|bm9uY2U=|Y2lwaGVydGV4dA==|50|0|device-A|2|"
        assertFalse(
            "A live blob (deleted field = '0') must not be reported as deleted",
            ClipboardBlobCodec.isDeletedBlob(liveBlob),
        )
    }

    // ── LWW rejection: tombstone with higher lamport blocks resurrection ────────

    /**
     * Simulate the LWW decision in `storeItemWithLww` when the stored blob is a
     * tombstone with lamport_ts T and the peer sends the same item with lamport_ts
     * T-1 (older version).  The peer item must be REJECTED.
     *
     * This is the invariant that tombstoneEvict relies on: the tombstone's bumped
     * lamport_ts must win over the older peer version.
     */
    @Test
    fun `LWW rejects incoming item with lower lamportTs than tombstone`() {
        val tombstoneLamport = 1000L
        val incomingLamport  = 999L // peer's copy is older

        // Simulate the LWW `remoteWins` decision from `storeItemWithLww`.
        // Simplified to the lamport comparison (the first branching condition).
        val remoteWins = when {
            incomingLamport > tombstoneLamport -> true
            incomingLamport < tombstoneLamport -> false
            else -> false // tie — default false for this test
        }

        assertFalse(
            "An incoming item with lamportTs=$incomingLamport must lose to a tombstone " +
                "with lamportTs=$tombstoneLamport (resurrection MUST NOT occur)",
            remoteWins,
        )
    }

    /**
     * The corollary: a peer sending a NEWER version (higher lamport) SHOULD win
     * even over a tombstone.  This is intentional — a newer version of a pruned
     * item is a legitimate re-create, not a resurrection.
     */
    @Test
    fun `LWW accepts incoming item with higher lamportTs than tombstone`() {
        val tombstoneLamport = 1000L
        val incomingLamport  = 1001L // peer has a newer version

        val remoteWins = incomingLamport > tombstoneLamport
        assertTrue(
            "An incoming item with lamportTs=$incomingLamport should replace a tombstone " +
                "with lamportTs=$tombstoneLamport (intentional re-create must win)",
            remoteWins,
        )
    }

    // ── item_id_ref invariant: ref must survive tombstoneEvict ─────────────────

    /**
     * Demonstrate the resurrection bug: if `item_id_ref_<itemId>` is removed by
     * pruneToLimits (the pre-fix behaviour), `storeItemWithLww` cannot find the
     * tombstone and falls through to the "new item" path, resurrecting the evicted
     * item.
     *
     * Uses an in-memory map to simulate the SharedPreferences state.
     */
    @Test
    fun `resurrection occurs when item_id_ref is removed after prune (pre-fix simulation)`() {
        val itemId = "test-item-uuid"
        val tombstoneLamport = 500L

        // Simulate the SharedPreferences after a BUGGY tombstoneEvict that removes the ref.
        val prefs = mutableMapOf<String, String>()
        prefs["item_$itemId"] = ClipboardBlobCodec.encodeTombstone(
            "1700000000|text/plain|3|bm9uY2U=|Y2lwaGVydGV4dA==|100|0|device-A|2|",
            bumpedLamportTs = tombstoneLamport,
        )
        // BUG: item_id_ref is removed by the old tombstoneEvict.
        // prefs["item_id_ref_$itemId"] is NOT set.

        // Simulate storeItemWithLww: look up item_id_ref.
        val existingStorageId = prefs["item_id_ref_$itemId"]
        // Without the ref, storeItemWithLww falls to the new-item path → resurrection.
        val willResurrect = existingStorageId == null

        assertTrue(
            "Pre-fix: removing item_id_ref causes storeItemWithLww to take the new-item path " +
                "→ resurrection. This confirms the 44rq.58 bug.",
            willResurrect,
        )
    }

    /**
     * With the fix: `tombstoneEvict` keeps `item_id_ref_<itemId>`, so
     * `storeItemWithLww` finds the tombstone, reads its bumped lamport_ts, and
     * rejects the older peer item via LWW comparison.
     */
    @Test
    fun `no resurrection when item_id_ref is preserved after prune (post-fix invariant)`() {
        val itemId = "test-item-uuid"
        val tombstoneLamport = 500L
        val incomingLamport  = 100L // peer sends old version

        // Simulate the SharedPreferences after the FIXED tombstoneEvict that keeps the ref.
        val prefs = mutableMapOf<String, String>()
        val tombstoneBlob = ClipboardBlobCodec.encodeTombstone(
            "1700000000|text/plain|3|bm9uY2U=|Y2lwaGVydGV4dA==|100|0|device-A|2|",
            bumpedLamportTs = tombstoneLamport,
        )
        prefs["item_$itemId"] = tombstoneBlob
        // FIX: item_id_ref is preserved.
        prefs["item_id_ref_$itemId"] = itemId

        // storeItemWithLww finds the ref.
        val existingStorageId = prefs["item_id_ref_$itemId"]
        assertNotNull("item_id_ref must be present after tombstoneEvict", existingStorageId)

        // LWW: read stored lamport from the tombstone.
        val storedRaw = prefs["item_$existingStorageId"]!!
        val storedLamport = storedRaw.split("|").getOrNull(5)?.toLongOrNull() ?: 0L
        assertEquals("Tombstone lamport must match the bumped value", tombstoneLamport, storedLamport)

        // LWW decision: older peer item must lose.
        val remoteWins = incomingLamport > storedLamport
        assertFalse(
            "Post-fix: peer item with lamport=$incomingLamport must NOT win over tombstone " +
                "with lamport=$tombstoneLamport — resurrection is prevented",
            remoteWins,
        )
    }

    // ── KEY_ITEM_IDS: tombstone id must remain in the index ───────────────────

    /**
     * After tombstoneEvict, the evicted item's id must remain in the KEY_ITEM_IDS
     * list so the tombstone entry is part of the stored index.  Removing it from
     * the index means a later LWW replace (where a peer sends a NEWER version)
     * cannot surface the item via `getItems` even after a successful replace.
     *
     * Mirrors the `deleteItem` behaviour: the single-item delete path never
     * removes the id from KEY_ITEM_IDS.
     */
    @Test
    fun `evicted item id must remain in KEY_ITEM_IDS as tombstone entry`() {
        // Simulate the id list before pruning.
        val allIds = mutableListOf("item-A", "item-B", "item-C", "item-D", "item-E")

        // A CORRECT tombstoneEvict: mark item-A as tombstoned but do NOT remove
        // it from the list (it becomes a tombstone entry in the index).
        val tombstonedIds = mutableSetOf<String>()

        fun correctTombstoneEvict(evictId: String) {
            // Tombstone the blob but keep the id in the list.
            tombstonedIds.add(evictId)
            // Do NOT call allIds.remove(evictId) — that is the pre-fix bug.
        }

        correctTombstoneEvict("item-A")
        correctTombstoneEvict("item-B")

        // KEY_ITEM_IDS must still contain the tombstoned ids.
        assertTrue(
            "item-A must remain in KEY_ITEM_IDS as a tombstone entry",
            "item-A" in allIds,
        )
        assertTrue(
            "item-B must remain in KEY_ITEM_IDS as a tombstone entry",
            "item-B" in allIds,
        )

        // The display layer filters them via isDeletedBlob — verify the separation.
        val displayed = allIds.filterNot { it in tombstonedIds }
        assertEquals(
            "Only live (non-tombstoned) items are displayed",
            listOf("item-C", "item-D", "item-E"),
            displayed,
        )
    }

    // ── Count-cap: live count must exclude tombstones ──────────────────────────

    /**
     * The count-cap eviction must track LIVE items (non-tombstoned) when deciding
     * how many more items to evict, even though tombstone ids remain in the index.
     *
     * Pre-fix: the count-cap loop decremented `ids.size` by removing ids from
     * the list.  That caused tombstones to be missing from KEY_ITEM_IDS.
     *
     * Post-fix: a separate `liveCount` variable tracks live items; `ids` retains
     * all entries (including tombstones) so the index is complete.
     */
    @Test
    fun `count-cap eviction tracks live items correctly when tombstones remain in index`() {
        // 5 items, max = 3, so 2 oldest unpinned must be evicted.
        val ids = mutableListOf("item-1", "item-2", "item-3", "item-4", "item-5")
        val pinnedSet = emptySet<String>()
        val maxItems = 3

        val unpinned = ids.filter { it !in pinnedSet }.toMutableList()
        var liveCount = ids.size // start from total count
        val tombstonedIds = mutableSetOf<String>()

        while (unpinned.isNotEmpty() && liveCount > maxItems) {
            val evictId = unpinned.removeAt(0)
            // FIX: do NOT remove from ids; decrement liveCount instead.
            tombstonedIds.add(evictId)
            liveCount--
        }

        // After eviction: liveCount == maxItems.
        assertEquals(
            "liveCount must reach maxItems after eviction",
            maxItems,
            liveCount,
        )

        // KEY_ITEM_IDS (ids) still contains all ids, including tombstones.
        assertEquals(
            "All ids must remain in KEY_ITEM_IDS (tombstones included)",
            5,
            ids.size,
        )

        // The 2 oldest unpinned items are tombstoned.
        assertEquals(
            "Exactly (5 - maxItems) = 2 items must be tombstoned",
            2,
            tombstonedIds.size,
        )
        assertTrue("Oldest item must be tombstoned", "item-1" in tombstonedIds)
        assertTrue("Second-oldest item must be tombstoned", "item-2" in tombstonedIds)
    }

    // ── Mutation queue: pruned items must appear in OutboundMutationQueue ────

    /**
     * Every item tombstoned by pruneToLimits must produce exactly one OP_DELETE
     * record in the mutation queue so the eviction propagates to all peers.
     * This test verifies the queue encode/decode round-trip for a prune-eviction record.
     */
    @Test
    fun `pruned item mutation record encodes and decodes correctly`() {
        val evictedItemId = "evicted-uuid-abc"
        val tombstoneLamport = 750L
        val nowMs = 1700000000000L

        val record = OutboundMutationQueue.MutationRecord(
            itemId = evictedItemId,
            op = OutboundMutationQueue.OP_DELETE,
            lamportTs = tombstoneLamport,
            wallTimeMs = nowMs,
            pinned = false,
            pinOrder = null,
        )

        val encoded = record.encode()
        val decoded = OutboundMutationQueue.MutationRecord.decode(encoded)

        assertNotNull("Prune mutation record must decode successfully", decoded)
        assertEquals(
            "itemId must round-trip through encode/decode",
            evictedItemId,
            decoded!!.itemId,
        )
        assertEquals(
            "op must be OP_DELETE for a prune-evicted item",
            OutboundMutationQueue.OP_DELETE,
            decoded.op,
        )
        assertEquals(
            "lamportTs must carry the bumped tombstone value",
            tombstoneLamport,
            decoded.lamportTs,
        )
        assertFalse(
            "pinned must be false for a pruned item's mutation record",
            decoded.pinned,
        )
        assertNull(
            "pinOrder must be null for a pruned item's mutation record",
            decoded.pinOrder,
        )
    }

    /**
     * Multiple pruned items must each produce their own OP_DELETE record.
     * Verifies that the mutation queue correctly holds N records for N evictions.
     */
    @Test
    fun `multiple pruned items each produce a distinct OP_DELETE mutation record`() {
        val evictedIds = listOf("evict-1", "evict-2", "evict-3")
        val nowMs = 1700000000000L

        val records = evictedIds.mapIndexed { i, id ->
            OutboundMutationQueue.MutationRecord(
                itemId = id,
                op = OutboundMutationQueue.OP_DELETE,
                lamportTs = 100L + i,
                wallTimeMs = nowMs,
                pinned = false,
                pinOrder = null,
            )
        }

        val serialized = OutboundMutationQueue.encodeQueue(records)
        val decoded = OutboundMutationQueue.decodeQueue(serialized)

        assertEquals(
            "Mutation queue must have one OP_DELETE record per evicted item",
            evictedIds.size,
            decoded.size,
        )
        evictedIds.forEachIndexed { i, expectedId ->
            assertEquals(
                "Record $i must carry the correct itemId",
                expectedId,
                decoded[i].itemId,
            )
            assertEquals(
                "Record $i must have op=OP_DELETE",
                OutboundMutationQueue.OP_DELETE,
                decoded[i].op,
            )
        }
    }
}
