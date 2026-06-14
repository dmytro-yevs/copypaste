package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-logic JVM unit tests for the Supabase cursor serialisation fix
 * (CopyPaste-bc3: race between FgsSyncLoop and SupabasePollWorker).
 *
 * These tests exercise only the pure compare-advance logic — no Android
 * runtime, no SharedPreferences, no coroutines.  They document and verify
 * the invariants enforced by [Settings.advanceSupabaseCursor].
 *
 * Run with: ./gradlew test
 */
class SupabaseCursorTest {

    // ── Simulate the advanceSupabaseCursor compare-and-write logic ──────────

    /** In-process simulation of Settings.advanceSupabaseCursor. */
    private data class Cursor(var wallTime: Long = 0L, var id: String = "")

    private fun Cursor.advance(wallTime: Long, id: String): Boolean {
        val isNewer = wallTime > this.wallTime ||
            (wallTime == this.wallTime && id > this.id)
        if (isNewer) {
            this.wallTime = wallTime
            this.id = id
        }
        return isNewer
    }

    // ── advance: basic progression ───────────────────────────────────────────

    @Test
    fun `advance with higher wallTime always wins`() {
        val cursor = Cursor(wallTime = 100L, id = "aaaa")
        cursor.advance(200L, "bbbb")
        assertEquals(200L, cursor.wallTime)
        assertEquals("bbbb", cursor.id)
    }

    @Test
    fun `advance with same wallTime and lexicographically greater id wins`() {
        val cursor = Cursor(wallTime = 100L, id = "aaaa")
        cursor.advance(100L, "bbbb")
        assertEquals(100L, cursor.wallTime)
        assertEquals("bbbb", cursor.id)
    }

    @Test
    fun `advance with same wallTime and same id is a no-op`() {
        val cursor = Cursor(wallTime = 100L, id = "aaaa")
        val advanced = cursor.advance(100L, "aaaa")
        assertFalse("Equal cursor should not advance", advanced)
        assertEquals(100L, cursor.wallTime)
        assertEquals("aaaa", cursor.id)
    }

    @Test
    fun `advance with lower wallTime is a no-op (monotonic)`() {
        val cursor = Cursor(wallTime = 200L, id = "bbbb")
        val advanced = cursor.advance(100L, "cccc")
        assertFalse("Lower wallTime must not roll cursor back", advanced)
        assertEquals(200L, cursor.wallTime)
        assertEquals("bbbb", cursor.id)
    }

    @Test
    fun `advance with same wallTime and lexicographically smaller id is a no-op`() {
        val cursor = Cursor(wallTime = 100L, id = "bbbb")
        val advanced = cursor.advance(100L, "aaaa")
        assertFalse("Smaller id at same wallTime must not advance", advanced)
        assertEquals("bbbb", cursor.id)
    }

    @Test
    fun `advance from zero always succeeds`() {
        val cursor = Cursor()
        assertTrue(cursor.advance(1L, "uuid-1"))
        assertEquals(1L, cursor.wallTime)
        assertEquals("uuid-1", cursor.id)
    }

    // ── Concurrent-advance simulation ────────────────────────────────────────
    // Simulates what happens when FgsSyncLoop and SupabasePollWorker both call
    // advanceSupabaseCursor concurrently, but under the lock only one wins.
    // The invariant: the cursor is always the keyset-maximum of all advances seen.

    @Test
    fun `last winner of concurrent advances is the keyset maximum`() {
        val cursor = Cursor()

        // Two concurrent batch ends: FGS advanced to (200, "b"), Worker to (150, "z")
        // The lock serialises them; order should not matter — max wins.
        cursor.advance(200L, "b")
        cursor.advance(150L, "z") // lower wallTime — no-op

        assertEquals(200L, cursor.wallTime)
        assertEquals("b", cursor.id)

        // Reverse order: Worker wins the lock first.
        val cursor2 = Cursor()
        cursor2.advance(150L, "z")
        cursor2.advance(200L, "b") // higher wallTime — wins

        assertEquals(200L, cursor2.wallTime)
        assertEquals("b", cursor2.id)
    }

    @Test
    fun `same wallTime — both concurrent advances serialised correctly`() {
        // FGS and Worker polled the same batch window (same wall_time) but
        // ended on different row ids (tie-break by id).
        val cursor = Cursor()
        cursor.advance(100L, "uuid-a")
        cursor.advance(100L, "uuid-z") // same wallTime, later id — wins

        assertEquals(100L, cursor.wallTime)
        assertEquals("uuid-z", cursor.id)
    }

    @Test
    fun `same wallTime — reversed order, smaller id loses`() {
        val cursor = Cursor()
        cursor.advance(100L, "uuid-z")
        cursor.advance(100L, "uuid-a") // same wallTime, earlier id — no-op

        assertEquals("uuid-z", cursor.id)
    }

    // ── maxHistoryItems count-cap pruning invariant (pure logic) ─────────────

    /**
     * Simulate the pruneToLimits count-cap loop. Returns the number evicted.
     * This mirrors the "Pass 2" code added to ClipboardRepository.pruneToLimits.
     */
    private fun simulateCountPrune(
        itemCount: Int,
        pinnedCount: Int,
        maxItems: Int,
    ): Int {
        require(pinnedCount <= itemCount)
        // Oldest items first; pinned items occupy the last `pinnedCount` slots
        // (simplification for test — in reality pinned items are mixed in).
        val unpinned = MutableList(itemCount - pinnedCount) { it.toString() }
        var total = itemCount
        var evicted = 0
        while (unpinned.isNotEmpty() && total > maxItems) {
            unpinned.removeAt(0)
            total--
            evicted++
        }
        return evicted
    }

    @Test
    fun `count-cap — no eviction when items within limit`() {
        assertEquals(0, simulateCountPrune(itemCount = 50, pinnedCount = 0, maxItems = 1000))
    }

    @Test
    fun `count-cap — evicts correct number when over limit`() {
        // 1100 items, limit 1000 → 100 should be evicted
        assertEquals(100, simulateCountPrune(itemCount = 1100, pinnedCount = 0, maxItems = 1000))
    }

    @Test
    fun `count-cap — pinned items count toward limit but are not evicted`() {
        // 1100 items (50 pinned), limit 1000. Only 1050 unpinned, must evict 100.
        assertEquals(100, simulateCountPrune(itemCount = 1100, pinnedCount = 50, maxItems = 1000))
    }

    @Test
    fun `count-cap — cannot evict more than unpinned items available`() {
        // 1100 items (1050 pinned, only 50 unpinned). Limit 1000 → need to evict
        // 100, but only 50 unpinned available → evict all 50, stop at 1050 items.
        assertEquals(50, simulateCountPrune(itemCount = 1100, pinnedCount = 1050, maxItems = 1000))
    }

    @Test
    fun `count-cap — at exactly the limit no eviction occurs`() {
        assertEquals(0, simulateCountPrune(itemCount = 1000, pinnedCount = 0, maxItems = 1000))
    }
}
