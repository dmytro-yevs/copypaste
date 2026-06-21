package com.copypaste.android

import com.copypaste.android.ui.IpcSyncBadgeState
import com.copypaste.android.ui.SyncBadgeState
import com.copypaste.android.ui.buildSyncTooltip
import com.copypaste.android.ui.resolveSyncBadgeState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * CopyPaste-5qbe: parity tests for offline-signal unification across platforms.
 *
 * CANONICAL RULE:
 *   "Offline" (red dot) is determined by daemon/IPC-reported connectivity.
 *   OS-level network (ConnectivityManager) is a SECONDARY signal used only to
 *   distinguish NetworkOffline (clear root cause) from DaemonUnreachable
 *   (sync infra broken despite OS being online). Both show red.
 *
 *   IDLE (grey dot) = daemon/sync layer reachable but no recent activity.
 *   This mirrors the macOS SyncStatusChip where badge_state "idle" → grey dot.
 *
 *   Mapping (IpcSyncBadgeState → SyncBadgeState display model):
 *     SYNCED / SYNCING    → Connected (green)
 *     IDLE / MISCONFIGURED → Idle (grey)   ← was DaemonUnreachable (red) before fix
 *     OFFLINE / ERROR     → DaemonUnreachable (red)
 *
 *   resolveSyncBadgeState fallback (when no IPC badge_state):
 *     count > 0 AND recent sync        → Connected (green)
 *     OS offline                       → NetworkOffline (red)
 *     OS online, sync stale/zero count → Idle (grey)  ← was DaemonUnreachable (red) before fix
 *
 * All tests are pure Kotlin — no Android SDK, no Compose runtime.
 */
class SyncBadgeParityTest {

    private val NOW_MS = 1_000_000L
    // Inline the value (5 min) rather than referencing RECENT_SYNC_MS from DevicesActivity —
    // DevicesActivity imports Android Activity classes that are unavailable in pure JVM unit tests.
    // Matches the pattern in SyncBadgeStateTest and the macOS SyncStatusChip constant.
    private val RECENT_MS = 5 * 60 * 1_000L

    // ── IpcSyncBadgeState → display model ────────────────────────────────────

    @Test
    fun `SYNCED maps to Connected (green)`() {
        assertEquals(SyncBadgeState.Connected, IpcSyncBadgeState.SYNCED.toSyncBadgeState())
    }

    @Test
    fun `SYNCING maps to Connected (green)`() {
        assertEquals(SyncBadgeState.Connected, IpcSyncBadgeState.SYNCING.toSyncBadgeState())
    }

    @Test
    fun `IDLE maps to Idle (grey) — parity with web idle-is-grey rule`() {
        // Web: badgeStateToSyncState("idle") === "idle" (grey)
        // Android: IDLE must map to Idle (grey), not DaemonUnreachable (red).
        assertEquals(SyncBadgeState.Idle, IpcSyncBadgeState.IDLE.toSyncBadgeState())
    }

    @Test
    fun `MISCONFIGURED maps to Idle (grey) — parity with web misconfigured-is-grey rule`() {
        // Web: badgeStateToSyncState("misconfigured") === "idle" (grey)
        // Android: MISCONFIGURED must map to Idle (grey), not DaemonUnreachable (red).
        assertEquals(SyncBadgeState.Idle, IpcSyncBadgeState.MISCONFIGURED.toSyncBadgeState())
    }

    @Test
    fun `OFFLINE maps to DaemonUnreachable (red)`() {
        assertEquals(SyncBadgeState.DaemonUnreachable, IpcSyncBadgeState.OFFLINE.toSyncBadgeState())
    }

    @Test
    fun `ERROR maps to DaemonUnreachable (red)`() {
        assertEquals(SyncBadgeState.DaemonUnreachable, IpcSyncBadgeState.ERROR.toSyncBadgeState())
    }

    // ── resolveSyncBadgeState fallback ────────────────────────────────────────

    @Test
    fun `connected when count positive and sync recent`() {
        val result = resolveSyncBadgeState(
            liveOnlineCount = 1,
            lastActivityMs = NOW_MS - RECENT_MS + 1_000L, // within window
            recentSyncMs = RECENT_MS,
            hasInternet = true,
            nowMs = NOW_MS,
        )
        assertEquals(SyncBadgeState.Connected, result)
    }

    @Test
    fun `Idle (grey) when OS online but sync stale — not red`() {
        // Before fix: returned DaemonUnreachable (red).
        // After fix: returns Idle (grey) — matches web "idle" grey dot.
        val result = resolveSyncBadgeState(
            liveOnlineCount = 0, // no active peers
            lastActivityMs = NOW_MS - RECENT_MS - 60_000L, // stale
            recentSyncMs = RECENT_MS,
            hasInternet = true,
            nowMs = NOW_MS,
        )
        assertEquals(SyncBadgeState.Idle, result)
    }

    @Test
    fun `Idle (grey) when OS online and count positive but sync stale`() {
        // Peers configured but last sync is old → grey (not red).
        // Web equivalent: badge_state "idle" from daemon → grey dot.
        val result = resolveSyncBadgeState(
            liveOnlineCount = 1,
            lastActivityMs = NOW_MS - RECENT_MS - 60_000L, // outside window
            recentSyncMs = RECENT_MS,
            hasInternet = true,
            nowMs = NOW_MS,
        )
        assertEquals(SyncBadgeState.Idle, result)
    }

    @Test
    fun `Idle (grey) when OS online and no lastActivity (zero)`() {
        // First launch: never synced, OS online → grey (not red).
        val result = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = 0L, // never
            recentSyncMs = RECENT_MS,
            hasInternet = true,
            nowMs = NOW_MS,
        )
        assertEquals(SyncBadgeState.Idle, result)
    }

    @Test
    fun `NetworkOffline when OS has no internet`() {
        val result = resolveSyncBadgeState(
            liveOnlineCount = 0,
            lastActivityMs = 0L,
            recentSyncMs = RECENT_MS,
            hasInternet = false,
            nowMs = NOW_MS,
        )
        assertEquals(SyncBadgeState.NetworkOffline, result)
    }

    @Test
    fun `NetworkOffline when OS offline and sync is stale`() {
        // OS offline is a clear root cause when sync has also gone stale.
        // Note: if sync were RECENT (count > 0 AND within window), Connected wins
        // even over OS offline (see SyncBadgeStateTest "Connected even when OS offline
        // if sync was recent"). This test covers the stale-sync + OS-offline case.
        val result = resolveSyncBadgeState(
            liveOnlineCount = 1,
            lastActivityMs = NOW_MS - RECENT_MS - 60_000L, // stale — outside the window
            recentSyncMs = RECENT_MS,
            hasInternet = false,
            nowMs = NOW_MS,
        )
        assertEquals(SyncBadgeState.NetworkOffline, result)
    }

    // ── Idle is NOT red — regression guard ───────────────────────────────────

    @Test
    fun `Idle display state is distinct from both red states`() {
        assertNotEquals(SyncBadgeState.Idle, SyncBadgeState.DaemonUnreachable)
        assertNotEquals(SyncBadgeState.Idle, SyncBadgeState.NetworkOffline)
    }

    // ── Tooltip: Idle shows "No sync yet" or last-sync time, not "Daemon unreachable" ──

    @Test
    fun `buildSyncTooltip for Idle with no prior activity shows No sync yet`() {
        val tooltip = buildSyncTooltip(
            badgeState = SyncBadgeState.Idle,
            lastActivityMs = 0L,
            count = 0,
            nowMs = NOW_MS,
        )
        assertFalse(
            "Idle tooltip must not say 'Daemon unreachable'",
            tooltip.contains("Daemon unreachable"),
        )
        assertEquals("No sync yet · No paired devices", tooltip)
    }

    @Test
    fun `buildSyncTooltip for Idle with prior activity shows last-sync time`() {
        val lastMs = NOW_MS - 30_000L // 30 s ago
        val tooltip = buildSyncTooltip(
            badgeState = SyncBadgeState.Idle,
            lastActivityMs = lastMs,
            count = 1,
            nowMs = NOW_MS,
        )
        assertFalse(
            "Idle tooltip must not say 'Daemon unreachable'",
            tooltip.contains("Daemon unreachable"),
        )
        // "30s ago · 1 device"
        assertEquals("Last sync: 30s ago · 1 device", tooltip)
    }

    @Test
    fun `buildSyncTooltip for DaemonUnreachable shows Daemon unreachable`() {
        val tooltip = buildSyncTooltip(
            badgeState = SyncBadgeState.DaemonUnreachable,
            lastActivityMs = 0L,
            count = 0,
            nowMs = NOW_MS,
        )
        assertEquals("Daemon unreachable · No paired devices", tooltip)
    }

    @Test
    fun `buildSyncTooltip for NetworkOffline shows Daemon unreachable`() {
        val tooltip = buildSyncTooltip(
            badgeState = SyncBadgeState.NetworkOffline,
            lastActivityMs = 0L,
            count = 0,
            nowMs = NOW_MS,
        )
        assertEquals("Daemon unreachable · No paired devices", tooltip)
    }
}
