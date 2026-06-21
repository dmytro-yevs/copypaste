package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-d6z3: online/offline derivation diverges from macOS (mDNS+lastSync vs live sinks).
 *
 * Root-cause: macOS derives online/offline by checking (a) mDNS-discovered peers within
 * ONLINE_WINDOW_MS (60 s) and (b) lastSyncMs within RECENT_SYNC_MS (5 min). Android
 * DevicesOnlineState only uses lastSyncMs without the mDNS-alive signal.
 *
 * Fix: DevicesOnlineState.startBackgroundPolling and publish() must use the same
 * (mDNS-discovered OR lastSyncMs-within-window) composite signal as macOS. A peer is
 * "online" if EITHER listDiscovered() lists it (live mDNS presence) OR its lastSyncMs
 * is within RECENT_SYNC_MS.
 *
 * The pure derivation helper isPeerOnline() is testable without Android runtime.
 */
class OnlineOfflineDerivationTest {

    /** Mirrors macOS ONLINE_WINDOW_MS = 60 000 ms (daemon constants). */
    private val ONLINE_WINDOW_MS = 60_000L

    /** Mirrors RECENT_SYNC_MS = 5 min. */
    private val RECENT_SYNC_MS = 5 * 60 * 1_000L

    private val NOW = 1_000_000L

    /**
     * A peer is online when lastSyncMs is within RECENT_SYNC_MS — macOS parity.
     */
    @Test
    fun `peer online when lastSyncMs within RECENT_SYNC_MS`() {
        val online = isPeerOnline(
            lastSyncMs = NOW - RECENT_SYNC_MS + 1_000L, // 1 s before window edge
            isMdnsDiscovered = false,
            nowMs = NOW,
            onlineWindowMs = ONLINE_WINDOW_MS,
            recentSyncMs = RECENT_SYNC_MS,
        )
        assertTrue("peer with recent lastSyncMs must be online", online)
    }

    /**
     * A peer is online when mDNS lists it (live on-net presence), even if lastSyncMs is stale.
     */
    @Test
    fun `peer online when mDNS discovered even with stale lastSyncMs`() {
        val online = isPeerOnline(
            lastSyncMs = NOW - RECENT_SYNC_MS - 60_000L, // stale
            isMdnsDiscovered = true,
            nowMs = NOW,
            onlineWindowMs = ONLINE_WINDOW_MS,
            recentSyncMs = RECENT_SYNC_MS,
        )
        assertTrue("mDNS-discovered peer must be online even with stale lastSyncMs", online)
    }

    /**
     * A peer with stale lastSyncMs and not in mDNS is offline.
     */
    @Test
    fun `peer offline when lastSyncMs stale and not mDNS discovered`() {
        val online = isPeerOnline(
            lastSyncMs = NOW - RECENT_SYNC_MS - 60_000L, // stale
            isMdnsDiscovered = false,
            nowMs = NOW,
            onlineWindowMs = ONLINE_WINDOW_MS,
            recentSyncMs = RECENT_SYNC_MS,
        )
        assertFalse("stale lastSyncMs and no mDNS → offline", online)
    }

    /**
     * A peer with lastSyncMs == 0 (never synced) and not mDNS discovered is offline.
     */
    @Test
    fun `peer offline when never synced and not mDNS discovered`() {
        val online = isPeerOnline(
            lastSyncMs = 0L,
            isMdnsDiscovered = false,
            nowMs = NOW,
            onlineWindowMs = ONLINE_WINDOW_MS,
            recentSyncMs = RECENT_SYNC_MS,
        )
        assertFalse("never-synced peer with no mDNS → offline", online)
    }

    /**
     * Count computation: online count = peers where isPeerOnline.
     */
    @Test
    fun `onlineCount is number of online peers`() {
        data class PeerState(val lastSyncMs: Long, val isMdnsDiscovered: Boolean)
        val peers = listOf(
            PeerState(NOW - 1_000L, false),          // recent sync → online
            PeerState(NOW - RECENT_SYNC_MS - 1L, true),  // mDNS discovered → online
            PeerState(0L, false),                     // never synced → offline
        )
        val count = peers.count { p ->
            isPeerOnline(p.lastSyncMs, p.isMdnsDiscovered, NOW, ONLINE_WINDOW_MS, RECENT_SYNC_MS)
        }
        assertEquals(2, count)
    }
}

// isPeerOnline is defined in DevicesActivity.kt (main source, same package).
// The local stub has been removed: the tests now exercise the canonical production
// implementation to ensure the derivation stays in sync.
