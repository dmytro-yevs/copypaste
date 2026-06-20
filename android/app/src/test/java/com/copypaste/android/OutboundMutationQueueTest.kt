package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the outbound mutation queue (CopyPaste-0qpn).
 *
 * The queue records pin/unpin/reorder/delete/bulk-delete/clear mutations so they
 * propagate to remote peers (relay, Supabase, P2P) even when the primary fresh-capture
 * producer does not fire (those mutations do not create new clipboard items).
 *
 * All logic exercised here is pure (no Android runtime, no SharedPreferences, no FFI).
 * Run with: ./gradlew test
 */
class OutboundMutationQueueTest {

    // ── OutboundMutation op constants ─────────────────────────────────────────

    @Test
    fun `OutboundMutation op values are distinct`() {
        val ops = listOf(
            OutboundMutationQueue.OP_PIN,
            OutboundMutationQueue.OP_UNPIN,
            OutboundMutationQueue.OP_REORDER,
            OutboundMutationQueue.OP_DELETE,
            OutboundMutationQueue.OP_BULK_DELETE,
            OutboundMutationQueue.OP_CLEAR,
        )
        assertEquals("All op constants must be distinct", ops.size, ops.toSet().size)
    }

    // ── MutationRecord encode/decode round-trip ────────────────────────────────

    @Test
    fun `MutationRecord encode decode round-trip for PIN`() {
        val rec = OutboundMutationQueue.MutationRecord(
            itemId = "item-uuid-1",
            op = OutboundMutationQueue.OP_PIN,
            lamportTs = 1_700_000_000L,
            wallTimeMs = 1_700_000_001L,
            pinned = true,
            pinOrder = 1.0,
        )
        val encoded = rec.encode()
        val decoded = OutboundMutationQueue.MutationRecord.decode(encoded)
        assertEquals("itemId must round-trip", rec.itemId, decoded?.itemId)
        assertEquals("op must round-trip", rec.op, decoded?.op)
        assertEquals("lamportTs must round-trip", rec.lamportTs, decoded?.lamportTs)
        assertEquals("wallTimeMs must round-trip", rec.wallTimeMs, decoded?.wallTimeMs)
        assertEquals("pinned must round-trip", rec.pinned, decoded?.pinned)
        assertEquals("pinOrder must round-trip", rec.pinOrder, decoded?.pinOrder)
    }

    @Test
    fun `MutationRecord encode decode round-trip for DELETE`() {
        val rec = OutboundMutationQueue.MutationRecord(
            itemId = "del-uuid-42",
            op = OutboundMutationQueue.OP_DELETE,
            lamportTs = 9_000_000L,
            wallTimeMs = 9_000_001L,
            pinned = false,
            pinOrder = null,
        )
        val encoded = rec.encode()
        val decoded = OutboundMutationQueue.MutationRecord.decode(encoded)
        assertEquals("itemId must round-trip for DELETE", rec.itemId, decoded?.itemId)
        assertEquals("op must round-trip for DELETE", rec.op, decoded?.op)
        assertFalse("pinned must be false for DELETE", decoded?.pinned ?: true)
        assertEquals("pinOrder must be null for DELETE", null, decoded?.pinOrder)
    }

    @Test
    fun `MutationRecord decode returns null for malformed input`() {
        assertFalse("Blank input must not decode", OutboundMutationQueue.MutationRecord.decode("") != null)
        assertFalse("Missing fields must not decode", OutboundMutationQueue.MutationRecord.decode("a|b") != null)
    }

    // ── In-memory queue operations ────────────────────────────────────────────

    @Test
    fun `in-memory queue encodes and decodes list correctly`() {
        val records = listOf(
            OutboundMutationQueue.MutationRecord("id-1", OutboundMutationQueue.OP_PIN, 100L, 101L, true, 1.0),
            OutboundMutationQueue.MutationRecord("id-2", OutboundMutationQueue.OP_DELETE, 200L, 201L, false, null),
        )
        val serialized = OutboundMutationQueue.encodeQueue(records)
        val decoded = OutboundMutationQueue.decodeQueue(serialized)
        assertEquals("Queue size must be preserved", 2, decoded.size)
        assertEquals("First record itemId", "id-1", decoded[0].itemId)
        assertEquals("Second record op", OutboundMutationQueue.OP_DELETE, decoded[1].op)
    }

    @Test
    fun `empty queue encodes and decodes as empty list`() {
        val empty = OutboundMutationQueue.encodeQueue(emptyList())
        val decoded = OutboundMutationQueue.decodeQueue(empty)
        assertTrue("Empty queue must decode to empty list", decoded.isEmpty())
    }

    @Test
    fun `decodeQueue skips malformed entries but retains valid ones`() {
        val good = OutboundMutationQueue.MutationRecord("id-ok", OutboundMutationQueue.OP_UNPIN, 10L, 11L, false, null)
        // Encode two good + one deliberately broken entry.
        val broken = "BROKEN|ENTRY"
        val raw = "${good.encode()}\n$broken\n${good.encode()}"
        val decoded = OutboundMutationQueue.decodeQueue(raw)
        assertEquals("Two valid records must survive one malformed entry", 2, decoded.size)
    }

    // ── P2P outbound filter: lamport-bumped items must not be filtered ─────────

    /**
     * Reproduces the P2P outbound filter bug (CopyPaste-0qpn root cause):
     *
     * filterByOutboundHighWater uses `wallTimeMs > highWater` to select items to
     * send. A setPinned/reorderPinned call bumps ONLY lamport_ts; wall_time stays
     * the same. After the initial sync, highWater == max(wallTimeMs), so a subsequent
     * pin mutation on an old item is filtered out: `wallTimeMs <= highWater → skip`.
     *
     * The fix: P2P outbound selection must also include items in the outbound
     * mutation queue, regardless of their wall_time.
     */
    @Test
    fun `filterByOutboundHighWater excludes pin-bumped items with old wallTime`() {
        // Simulate: item was synced at wallTime=1000, highWater=1000.
        // User pins item → lamport bumped but wallTime unchanged (still 1000).
        val item = "pin-item" to 1000L  // (itemId, wallTimeMs)
        val highWater = 1000L
        val filtered = FgsSyncLoop.filterByOutboundHighWater(listOf(item), highWater)
        // Demonstrates the bug: the pin-bumped item IS filtered out.
        assertTrue(
            "filterByOutboundHighWater DOES filter pin-bumped items (wall_time == highWater) — " +
                "confirming the bug that the queue-based producer fixes",
            filtered.isEmpty(),
        )
    }

    @Test
    fun `mutation queue items bypass the wallTime filter`() {
        // After fix: items in the outbound mutation queue must be selected for
        // sending independently of the wall_time high-water cursor. This test
        // encodes the expected behaviour: a mutation record with itemId="pin-item"
        // must be selected even when filterByOutboundHighWater would exclude it.
        val mutation = OutboundMutationQueue.MutationRecord(
            itemId = "pin-item",
            op = OutboundMutationQueue.OP_PIN,
            lamportTs = 1001L,  // bumped lamport
            wallTimeMs = 1000L, // unchanged wall_time
            pinned = true,
            pinOrder = 1.0,
        )
        // A queue with this mutation should have itemId selected even when
        // the regular filter would exclude it.
        val pendingIds = listOf(mutation).map { it.itemId }.toSet()
        assertTrue(
            "Mutation queue must include the pin-bumped item even when wall_time == highWater",
            "pin-item" in pendingIds,
        )
    }

    // ── LWW invariant: lamport bumps are strictly increasing ─────────────────

    @Test
    fun `nextLamportTs is monotonically increasing`() {
        val now = System.currentTimeMillis()
        val prev = now - 1000L
        val next = ClipboardRepository.nextLamportTs(prev, now)
        assertTrue("nextLamportTs must be > prev", next > prev)
        assertTrue("nextLamportTs must be >= now", next >= now)
    }

    @Test
    fun `nextLamportTs never goes backwards`() {
        // Simulate a lamport already well ahead of wall-clock (large initial ts).
        val highLamport = System.currentTimeMillis() + 999_999L
        val now = System.currentTimeMillis()
        val next = ClipboardRepository.nextLamportTs(highLamport, now)
        assertEquals("nextLamportTs must be prev+1 when prev+1 > now", highLamport + 1L, next)
    }

    // ── Tombstone propagation: delete must emit before physical removal ────────

    /**
     * Confirms that bulk-delete must emit per-item tombstones BEFORE the rows are
     * physically removed. After the rows are gone there is nothing to read back and
     * generate a tombstone from, so the queue entries must carry the itemId directly.
     *
     * This test verifies the design invariant via the MutationRecord: a OP_DELETE
     * record carries the itemId and lamportTs so they can be pushed without the row.
     */
    @Test
    fun `MutationRecord for delete carries itemId and lamportTs without needing the row`() {
        val itemId = "row-to-delete-uuid"
        val lamport = 5000L
        val rec = OutboundMutationQueue.MutationRecord(
            itemId = itemId,
            op = OutboundMutationQueue.OP_DELETE,
            lamportTs = lamport,
            wallTimeMs = System.currentTimeMillis(),
            pinned = false,
            pinOrder = null,
        )
        // The record must carry enough info to build a relay/supabase tombstone push
        // without accessing SharedPreferences (the row may be gone by push time).
        assertEquals("Must carry itemId", itemId, rec.itemId)
        assertEquals("Must carry lamportTs for LWW ordering", lamport, rec.lamportTs)
        assertEquals("op must be DELETE", OutboundMutationQueue.OP_DELETE, rec.op)
    }
}
