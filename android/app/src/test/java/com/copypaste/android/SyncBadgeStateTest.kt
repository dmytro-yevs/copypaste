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
 * - [SyncBadgeState.Idle] is returned when OS is online but sync has not worked recently
 *   (canonical 5qbe rule: OS-online + stale = grey, not red).
 *
 * These tests confirm the alignment with macOS SyncStatusChip behaviour post-5qbe:
 * the badge shows DANGER only when the OS itself has no internet (NetworkOffline) or
 * when the daemon IPC signals an explicit failure (DaemonUnreachable via IpcSyncBadgeState).
 * OS-online + no-recent-sync → Idle (grey), not red — mirrors macOS "idle" grey dot.
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
     * OS is online but no sync has ever occurred → Idle (grey).
     * Canonical 5qbe rule: OS-online + stale/no-sync = Idle (grey), not red.
     * DaemonUnreachable (red) requires an authoritative IPC badge_state of OFFLINE/ERROR.
     * On a fresh install with sync never attempted, showing red would be a false alarm.
     */
    @Test
    fun `Idle when OS online but sync never worked`() {
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = 0L,
            recentSyncMs = recentSyncMs,
            hasInternet = true,
        )
        assertEquals(SyncBadgeState.Idle, state)
    }

    /**
     * Sync worked long ago (> RECENT_SYNC_MS) and OS is online → Idle (grey).
     * Canonical 5qbe rule: OS-online + stale = Idle (grey), not red.
     * Mirrors macOS chip: badge_state "idle" → grey dot (not DANGER).
     * A hard failure (auth error, relay down) requires an authoritative IPC signal to show red.
     */
    @Test
    fun `Idle when last sync is stale and OS online`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = nowMs - (6 * 60 * 1_000L), // 6 min ago — past 5-min window
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.Idle, state)
    }

    /**
     * Count > 0 but lastActivityMs is stale → NOT Connected, falls through to Idle
     * (since OS is online). The recency gate must reject stale counts.
     * Canonical 5qbe rule: stale + OS-online → Idle (grey), not red.
     */
    @Test
    fun `Idle when count is positive but last activity stale`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 3,
            lastActivityMs = nowMs - (10 * 60 * 1_000L), // 10 min ago
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.Idle, state)
    }

    /**
     * Count = 0 (DevicesScreen published real count of 0) but lastActivityMs is recent.
     * Should NOT be Connected (no peers online), falls through to Idle (grey).
     * OS is online but no live peers → Idle (not red) per 5qbe canonical rule.
     */
    @Test
    fun `Idle when count is zero even with recent activity`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = nowMs - 30_000L,
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            nowMs = nowMs,
        )
        assertEquals(SyncBadgeState.Idle, state)
    }

    /** SyncBadgeState.Connected is distinct from both red states and the grey Idle state. */
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
        assertTrue("Connected should not be Idle", state !is SyncBadgeState.Idle)
    }

    /**
     * OS offline states remain non-Connected, and Idle is distinct from both red states.
     */
    @Test
    fun `Error states are not Connected`() {
        // OS offline → NetworkOffline (red)
        val offline = resolveSyncBadgeState(0, 0L, recentSyncMs, hasInternet = false)
        // OS online, no sync, no error → Idle (grey) — not red, not Connected
        val idle = resolveSyncBadgeState(0, 0L, recentSyncMs, hasInternet = true)
        assertTrue(offline !is SyncBadgeState.Connected)
        assertTrue(idle !is SyncBadgeState.Connected)
        assertTrue(idle is SyncBadgeState.Idle)
    }

    /**
     * CopyPaste-5917.52: isSyncError=true + hasInternet=true → DaemonUnreachable (red).
     * This is the production path for DaemonUnreachable — driven by FgsSyncLoop hard errors.
     * Previously DaemonUnreachable was only reachable via IpcSyncBadgeState (IPC path not yet wired).
     */
    @Test
    fun `DaemonUnreachable when isSyncError true and OS has internet`() {
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = 0L,
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            isSyncError = true,
        )
        assertEquals(SyncBadgeState.DaemonUnreachable, state)
    }

    /**
     * CopyPaste-5917.52: when OS is offline, NetworkOffline takes priority over isSyncError
     * (the root cause is the OS being offline, not a daemon error — clearer for the user).
     */
    @Test
    fun `NetworkOffline takes priority over isSyncError when OS is offline`() {
        val state = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = 0L,
            recentSyncMs = recentSyncMs,
            hasInternet = false,
            isSyncError = true,
        )
        assertEquals(SyncBadgeState.NetworkOffline, state)
    }

    /**
     * CopyPaste-5917.52: recent sync takes priority over isSyncError (recovered mid-session).
     */
    @Test
    fun `Connected takes priority over isSyncError when sync is recent`() {
        val nowMs = System.currentTimeMillis()
        val state = resolveSyncBadgeState(
            liveOnlineCount = 1,
            lastActivityMs = nowMs - 30_000L,
            recentSyncMs = recentSyncMs,
            hasInternet = true,
            isSyncError = true,
        )
        assertEquals(SyncBadgeState.Connected, state)
    }
}
