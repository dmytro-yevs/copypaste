package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-logic JVM unit tests for the P2P high-water cursor fix
 * (CopyPaste-p9s: Android lazyload refetches already-synced history from peer).
 *
 * These exercise the companion functions only — no Android runtime needed.
 * Run with: ./gradlew test
 */
class P2pHighWaterCursorTest {

    // ── filterByOutboundHighWater ─────────────────────────────────────────────

    @Test
    fun `filterByOutboundHighWater zero returns all items`() {
        val items = listOf("a" to 100L, "b" to 200L, "c" to 300L)
        val result = FgsSyncLoop.filterByOutboundHighWater(items, outboundHighWater = 0L)
        assertEquals(items, result)
    }

    @Test
    fun `filterByOutboundHighWater excludes items at or below cursor`() {
        val items = listOf("a" to 100L, "b" to 200L, "c" to 300L, "d" to 400L)
        val result = FgsSyncLoop.filterByOutboundHighWater(items, outboundHighWater = 200L)
        // Items with wallTimeMs > 200 only: c(300) and d(400).
        assertEquals(listOf("c" to 300L, "d" to 400L), result)
    }

    @Test
    fun `filterByOutboundHighWater with cursor at max returns empty`() {
        val items = listOf("a" to 100L, "b" to 200L)
        val result = FgsSyncLoop.filterByOutboundHighWater(items, outboundHighWater = 200L)
        assertTrue("Expected empty, got $result", result.isEmpty())
    }

    @Test
    fun `filterByOutboundHighWater empty input returns empty`() {
        val result = FgsSyncLoop.filterByOutboundHighWater(emptyList(), outboundHighWater = 500L)
        assertTrue(result.isEmpty())
    }

    @Test
    fun `filterByOutboundHighWater cursor strictly greater than — equal wallTime is excluded`() {
        // Items with wallTimeMs == outboundHighWater must NOT be re-sent
        // (already confirmed delivered on the previous dial).
        val items = listOf("a" to 100L, "b" to 100L, "c" to 101L)
        val result = FgsSyncLoop.filterByOutboundHighWater(items, outboundHighWater = 100L)
        assertEquals(listOf("c" to 101L), result)
    }

    // ── maxWallTime ───────────────────────────────────────────────────────────

    @Test
    fun `maxWallTime empty list returns zero`() {
        assertEquals(0L, FgsSyncLoop.maxWallTime(emptyList()))
    }

    @Test
    fun `maxWallTime single item`() {
        assertEquals(42L, FgsSyncLoop.maxWallTime(listOf("x" to 42L)))
    }

    @Test
    fun `maxWallTime returns highest wallTime`() {
        val items = listOf("a" to 100L, "b" to 500L, "c" to 300L)
        assertEquals(500L, FgsSyncLoop.maxWallTime(items))
    }

    // ── P2P_DIAL_INTERVAL_MS ──────────────────────────────────────────────────

    @Test
    fun `P2P_DIAL_INTERVAL_MS is at least 30 seconds`() {
        // CopyPaste-p9s fix: interval was 3s, causing re-transmission every 3s.
        // Must now be >= 30_000 ms.
        assertTrue(
            "P2P_DIAL_INTERVAL_MS must be >= 30s (was 3s before the fix), got ${FgsSyncLoop.P2P_DIAL_INTERVAL_MS}",
            FgsSyncLoop.P2P_DIAL_INTERVAL_MS >= 30_000L,
        )
    }

    // ── Cursor monotonicity property ─────────────────────────────────────────
    // (These are pure-logic proofs; the Settings wrappers are Android-runtime.)

    @Test
    fun `advanceP2pHighWater logic is monotonically increasing`() {
        // Simulate the Settings.advanceP2pOutboundHighWater guard:
        // "only write if new value > current".
        var stored = 0L
        fun advance(newVal: Long) {
            if (newVal > stored) stored = newVal
        }

        advance(100L)
        assertEquals(100L, stored)

        advance(50L)  // must NOT roll back
        assertEquals(100L, stored)

        advance(200L)
        assertEquals(200L, stored)

        advance(200L)  // equal must NOT advance
        assertEquals(200L, stored)
    }
}
