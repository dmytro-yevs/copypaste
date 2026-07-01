package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.36: characterization tests for [SyncCursorsStore] — the
 * Supabase compound keyset cursor's monotonicity (wall_time, then id
 * tie-break) and the per-peer P2P high-water cursor monotonic-advance/clear
 * semantics. Pure logic; no Android framework dependency beyond
 * [FakeSharedPreferences].
 */
class SyncCursorsStoreTest {

    private fun store() = SyncCursorsStore(FakeSharedPreferences())

    @Test
    fun `advanceSupabaseCursor advances on a strictly greater wall time`() {
        val s = store()
        s.advanceSupabaseCursor(100L, "a")
        s.advanceSupabaseCursor(200L, "b")
        assertEquals(200L, s.lastSupabasePollWallTime)
        assertEquals("b", s.lastSupabasePollId)
    }

    @Test
    fun `advanceSupabaseCursor advances on equal wall time with a lexicographically greater id`() {
        val s = store()
        s.advanceSupabaseCursor(100L, "aaa")
        s.advanceSupabaseCursor(100L, "bbb")
        assertEquals(100L, s.lastSupabasePollWallTime)
        assertEquals("bbb", s.lastSupabasePollId)
    }

    @Test
    fun `advanceSupabaseCursor ignores a stale wall time`() {
        val s = store()
        s.advanceSupabaseCursor(200L, "b")
        s.advanceSupabaseCursor(100L, "z") // stale: must not roll the cursor backward
        assertEquals(200L, s.lastSupabasePollWallTime)
        assertEquals("b", s.lastSupabasePollId)
    }

    @Test
    fun `advanceSupabaseCursor ignores equal wall time with a lexicographically lesser id`() {
        val s = store()
        s.advanceSupabaseCursor(100L, "bbb")
        s.advanceSupabaseCursor(100L, "aaa")
        assertEquals("bbb", s.lastSupabasePollId)
    }

    @Test
    fun `p2p outbound high-water advances monotonically`() {
        val s = store()
        assertEquals(0L, s.p2pOutboundHighWater("fp1"))
        s.advanceP2pOutboundHighWater("fp1", 500L)
        s.advanceP2pOutboundHighWater("fp1", 300L) // stale, ignored
        assertEquals(500L, s.p2pOutboundHighWater("fp1"))
        s.advanceP2pOutboundHighWater("fp1", 600L)
        assertEquals(600L, s.p2pOutboundHighWater("fp1"))
    }

    @Test
    fun `p2p inbound high-water advances monotonically and is independent per fingerprint`() {
        val s = store()
        s.advanceP2pInboundHighWater("fp1", 10L)
        s.advanceP2pInboundHighWater("fp2", 999L)
        assertEquals(10L, s.p2pInboundHighWater("fp1"))
        assertEquals(999L, s.p2pInboundHighWater("fp2"))
    }

    @Test
    fun `clearP2pHighWater resets both cursors for a fingerprint only`() {
        val s = store()
        s.advanceP2pOutboundHighWater("fp1", 10L)
        s.advanceP2pInboundHighWater("fp1", 20L)
        s.advanceP2pOutboundHighWater("fp2", 30L)

        s.clearP2pHighWater("fp1")

        assertEquals(0L, s.p2pOutboundHighWater("fp1"))
        assertEquals(0L, s.p2pInboundHighWater("fp1"))
        assertTrue(s.p2pOutboundHighWater("fp2") == 30L)
    }
}
