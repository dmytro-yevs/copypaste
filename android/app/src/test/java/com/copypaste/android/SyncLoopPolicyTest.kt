package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.35 — [SyncLoopPolicy] characterization tests.
 *
 * Restores coverage for the pure scheduling/backoff/filtering helpers after
 * their extraction from [FgsSyncLoop]'s companion object. Companion-level
 * backoff/interval regressions remain covered by [FgsSyncLoopBackoffTest]
 * (calls [FgsSyncLoop]'s forwarding stubs); this file exercises the
 * remaining pure functions directly against [SyncLoopPolicy].
 */
class SyncLoopPolicyTest {

    // ── p2pDialIntervalMs ───────────────────────────────────────────────────

    @Test
    fun `p2pDialIntervalMs stays at base while below idle threshold`() {
        assertEquals(SyncLoopPolicy.P2P_DIAL_INTERVAL_MS, SyncLoopPolicy.p2pDialIntervalMs(0))
        assertEquals(SyncLoopPolicy.P2P_DIAL_INTERVAL_MS, SyncLoopPolicy.p2pDialIntervalMs(2))
    }

    @Test
    fun `p2pDialIntervalMs grows to idle cap at threshold`() {
        assertEquals(300_000L, SyncLoopPolicy.p2pDialIntervalMs(3))
        assertEquals(300_000L, SyncLoopPolicy.p2pDialIntervalMs(50))
    }

    // ── shouldAttemptDrain ──────────────────────────────────────────────────

    @Test
    fun `shouldAttemptDrain is false when queue is empty`() {
        assertFalse(SyncLoopPolicy.shouldAttemptDrain(queueSize = 0, nowMs = 1000L, backoffUntilMs = 0L))
    }

    @Test
    fun `shouldAttemptDrain is false while backoff window has not elapsed`() {
        assertFalse(SyncLoopPolicy.shouldAttemptDrain(queueSize = 1, nowMs = 500L, backoffUntilMs = 1000L))
    }

    @Test
    fun `shouldAttemptDrain is true once backoff window elapses with pending records`() {
        assertTrue(SyncLoopPolicy.shouldAttemptDrain(queueSize = 1, nowMs = 1000L, backoffUntilMs = 1000L))
    }

    // ── filterByOutboundHighWater ───────────────────────────────────────────

    @Test
    fun `filterByOutboundHighWater returns everything when high-water is zero (first dial)`() {
        val items = listOf("a" to 100L, "b" to 200L)
        assertEquals(items, SyncLoopPolicy.filterByOutboundHighWater(items, outboundHighWater = 0L))
    }

    @Test
    fun `filterByOutboundHighWater keeps only items strictly newer than high-water`() {
        val items = listOf("a" to 100L, "b" to 200L, "c" to 300L)
        val filtered = SyncLoopPolicy.filterByOutboundHighWater(items, outboundHighWater = 200L)
        assertEquals(listOf("c" to 300L), filtered)
    }

    // ── maxWallTime ─────────────────────────────────────────────────────────

    @Test
    fun `maxWallTime is zero for empty list`() {
        assertEquals(0L, SyncLoopPolicy.maxWallTime(emptyList()))
    }

    @Test
    fun `maxWallTime returns the highest wallTimeMs`() {
        val items = listOf("a" to 100L, "b" to 500L, "c" to 300L)
        assertEquals(500L, SyncLoopPolicy.maxWallTime(items))
    }

    // ── filterQueuedMutationsForP2P / mergeQueuedItemIdsWithLocal ──────────

    private fun mutation(itemId: String) =
        OutboundMutationQueue.MutationRecord(itemId, OutboundMutationQueue.OP_PIN, 1L, 1L, true, 1.0)

    @Test
    fun `filterQueuedMutationsForP2P bypasses the wall-time high-water filter unconditionally`() {
        val pending = listOf(mutation("m1"), mutation("m2"))
        // High-water is irrelevant — ALL pending mutations are always returned.
        assertEquals(pending, SyncLoopPolicy.filterQueuedMutationsForP2P(pending, outboundHighWater = Long.MAX_VALUE))
    }

    @Test
    fun `mergeQueuedItemIdsWithLocal is the union deduplicated by item id`() {
        val local = setOf("a", "b")
        val queued = listOf(mutation("b"), mutation("c"))
        val merged = SyncLoopPolicy.mergeQueuedItemIdsWithLocal(local, queued)
        assertEquals(setOf("a", "b", "c"), merged)
    }

    // ── newestTextClip ──────────────────────────────────────────────────────

    @Test
    fun `newestTextClip returns null for empty list`() {
        assertEquals(null, SyncLoopPolicy.newestTextClip(emptyList()))
    }

    @Test
    fun `newestTextClip picks highest wall time, ties resolved by last-in-order`() {
        val clips = listOf("first" to 1000L, "tie-a" to 5000L, "tie-b" to 5000L)
        assertEquals("tie-b", SyncLoopPolicy.newestTextClip(clips))
    }
}
