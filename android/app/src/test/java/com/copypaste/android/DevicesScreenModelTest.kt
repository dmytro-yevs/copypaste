package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the DevicesScreen presence logic — the per-peer
 * "online" dot now reads [PairedPeer.lastSyncMs] (a real-presence signal
 * stamped by FgsSyncLoop), not the old Supabase poll-cursor proxy.
 *
 * These tests do NOT require Android SDK or an emulator. They verify that
 * [PairedPeer.isOnline] is true only when the last-sync timestamp is recent
 * (within [ONLINE_WINDOW_MS], mirroring the macOS daemon's 60 s threshold).
 */
class DevicesScreenModelTest {

    private fun peer(lastSyncMs: Long) = PairedPeer(
        fingerprint = "fp",
        syncAddr = "host:7007",
        name = "Test Mac",
        sessionKeyWrappedB64 = "",
        sessionKeyIvB64 = "",
        lastSyncMs = lastSyncMs,
    )

    @Test
    fun `isOnline returns true when last sync is within window`() {
        val now = System.currentTimeMillis()
        val recentMs = now - 30_000L // 30 seconds ago
        assertTrue("30s ago sync must be considered online", peer(recentMs).isOnline(now))
    }

    @Test
    fun `isOnline returns false when last sync is old`() {
        val now = System.currentTimeMillis()
        val oldMs = now - (ONLINE_WINDOW_MS + 1_000L)
        assertFalse("Stale last-sync must not be online", peer(oldMs).isOnline(now))
    }

    @Test
    fun `isOnline returns false when last sync is zero`() {
        assertFalse("Zero last-sync (never synced) must not be online", peer(0L).isOnline())
    }

    @Test
    fun `isOnline is true exactly at the window boundary`() {
        val now = System.currentTimeMillis()
        val boundaryMs = now - ONLINE_WINDOW_MS
        assertTrue("Sync exactly at the threshold is still online", peer(boundaryMs).isOnline(now))
    }
}
