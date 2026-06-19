package com.copypaste.android

import com.copypaste.android.ui.SyncBadgeState
import com.copypaste.android.ui.resolveSyncBadgeState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for [resolveSyncBadgeState] — the PG-10 / 5qbe offline-signal fix.
 *
 * Verifies that:
 * - [SyncBadgeState.Connected] is returned when sync has worked recently (daemon-primary signal).
 * - [SyncBadgeState.NetworkOffline] is returned when OS has no internet.
 * - [SyncBadgeState.DaemonUnreachable] is returned when OS is online but sync has not worked.
 *
 * These tests confirm the alignment with macOS SyncStatusChip behaviour:
 * the badge should show DANGER when sync is broken regardless of OS network state
 * (not just when Wi-Fi is absent — that was the pre-5qbe bug).
 */
class SyncBadgeStateTest {

    private val recentSyncMs = 5 * 60 * 1_000L // 5 min, mirrors RECENT_SYNC_MS

    /**
     * Sync worked recently (count > 0, lastActivityMs within window) → Connected.
     * OS internet state is irrelevant when sync is confirmed working.
     */
    @Test
    fun `Connected when count and last activity are both recent`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 1,
            lastActivityMs = nowMs - 60_000L, // 1 min ago — well within 5 min
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.Connected, state)
    }

    /**
     * Sync worked recently AND OS is offline → still Connected.
     * (Edge case: device lost Wi-Fi after a recent successful sync — badge should
     * still show green for the 5 min window, not flip to danger immediately.)
     */
    @Test
    fun `Connected even when OS offline if sync was recent`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 2,
            lastActivityMs = nowMs - 30_000L, // 30 s ago
            recentSyncMs = recentSyncMs,
            hasInternet = false, // OS says offline
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.Connected, state)
    }

    /**
     * No sync has ever occurred (lastActivityMs == 0) and OS is offline → NetworkOffline.
     * The raw OS signal is the most actionable hint here.
     */
    @Test
    fun `NetworkOffline when no sync ever and OS is offline`() {
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = 0L,
            recentSyncMs = recentSyncMs,
            hasInternet = false,
        )
        assertEquals(SyncBadgeState.NetworkOffline, state)
    }

    /**
     * OS is online but no sync has ever occurred → DaemonUnreachable.
     * This is the key PG-10 fix: Android must NOT show "idle/grey" here — it must
     * show DANGER (same as macOS when daemon IPC is unresponsive) so the user knows
     * to check their sync credentials/config.
     */
    @Test
    fun `DaemonUnreachable when OS online but sync never worked`() {
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = 0L,
            recentSyncMs = recentSyncMs,
            hasInternet = true,
        )
        assertEquals(SyncBadgeState.DaemonUnreachable, state)
    }

    /**
     * Sync worked long ago (> RECENT_SYNC_MS) and OS is online → DaemonUnreachable.
     * The pre-5qbe bug would have shown grey "idle" here; correct behaviour is DANGER
     * because we have evidence sync stopped working (same as macOS chip's stale-IPC case).
     */
    @Test
    fun `DaemonUnreachable when last sync is stale and OS online`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = nowMs - (6 * 60 * 1_000L), // 6 min ago — past 5-min window
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.DaemonUnreachable, state)
    }

    /**
     * Count > 0 but lastActivityMs is stale → NOT Connected, falls through to DaemonUnreachable
     * (since OS is online). The recency gate must reject stale counts.
     */
    @Test
    fun `DaemonUnreachable when count is positive but last activity stale`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 3,
            lastActivityMs = nowMs - (10 * 60 * 1_000L), // 10 min ago
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.DaemonUnreachable, state)
    }

    /**
     * Count = 0 (DevicesScreen published real count of 0) but lastActivityMs is recent.
     * Should NOT be Connected (no peers online), falls through to DaemonUnreachable.
     */
    @Test
    fun `DaemonUnreachable when count is zero even with recent activity`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = nowMs - 30_000L,
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.DaemonUnreachable, state)
    }

    /** SyncBadgeState.Connected is distinct from both error states. */
    @Test
    fun `Connected state is not an error state`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 1,
            lastActivityMs = nowMs - 1_000L,
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertTrue("Connected should not be NetworkOffline", state !is SyncBadgeState.NetworkOffline)
        assertTrue("Connected should not be DaemonUnreachable", state !is SyncBadgeState.DaemonUnreachable)
    }

    /** Both error states are distinct from Connected. */
    @Test
    fun `Error states are not Connected`() {
        val offline = resolveSyncBadgeState(0, 0L, recentSyncMs, hasInternet = false)
        val unreachable = resolveSyncBadgeState(0, 0L, recentSyncMs, hasInternet = true)
        assertTrue(offline !is SyncBadgeState.Connected)
        assertTrue(unreachable !is SyncBadgeState.Connected)
    }
}
