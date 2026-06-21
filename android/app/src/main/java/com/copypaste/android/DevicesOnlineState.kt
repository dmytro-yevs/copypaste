package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * "Online" recency threshold for the per-peer green dot.
 *
 * A peer that completed a successful P2P sync within the last [ONLINE_WINDOW_MS]
 * is rendered online (green dot); otherwise offline (grey). This mirrors the
 * macOS daemon's `ONLINE_THRESHOLD_SECS` (60 s) so both platforms agree on what
 * "online" means. The presence signal is [PairedPeer.lastSyncMs], stamped by
 * [FgsSyncLoop] (via [Settings.updatePeerLastSync]) on each successful dial —
 * NOT the old `lastSupabasePollWallTime` poll-cursor proxy.
 */
internal const val ONLINE_WINDOW_MS = 60_000L

/** True when [peer] synced within [ONLINE_WINDOW_MS] of [nowMs]. */
internal fun PairedPeer.isOnline(nowMs: Long = System.currentTimeMillis()): Boolean =
    lastSyncMs > 0L && (nowMs - lastSyncMs) <= ONLINE_WINDOW_MS

/**
 * How recent a last_sync_ms must be to count as "connected" in the badge
 * (PG-11). Mirrors macOS [SyncStatusChip.tsx] `RECENT_SYNC_MS = 5 * 60 * 1000`.
 * A peer that has not synced within this window is considered stale even if it
 * is still technically in the ONLINE_WINDOW_MS bracket — the badge should only
 * show green when we have evidence of a recent successful exchange.
 *
 * [SyncStatusBadge] should gate its "connected" colour on this threshold when
 * falling back to the configured-count path (PG-41 / PG-11 follow-up):
 * `lastActivityMs.value > 0 && (now - lastActivityMs.value) <= RECENT_SYNC_MS`.
 */
// c4q2.5: This value mirrors copypaste_ipc::SYNC_BADGE_RECENT_MS (crates/copypaste-ipc/src/methods.rs:208).
// Both must stay equal — if the Rust constant changes, update this too (and vice-versa).
internal const val RECENT_SYNC_MS = 5 * 60 * 1_000L

/**
 * CopyPaste-d6z3: pure online-derivation function matching macOS daemon logic.
 *
 * A peer is "online" iff EITHER:
 *  (a) its [lastSyncMs] is within [recentSyncMs] of [nowMs] (recent successful sync), OR
 *  (b) it is currently in the mDNS discovery table ([isMdnsDiscovered]).
 *
 * This mirrors the macOS `isPeerOnline` derivation: online = recentSync || mDNSDiscovered.
 * [onlineWindowMs] is retained as a separate parameter for future use (e.g. a tighter
 * P2P-contact window gate); currently [recentSyncMs] is the sole lastSyncMs gate.
 *
 * Pure function: no Android runtime dependencies — unit-testable without an emulator.
 */
internal fun isPeerOnline(
    lastSyncMs: Long,
    isMdnsDiscovered: Boolean,
    nowMs: Long,
    onlineWindowMs: Long,
    recentSyncMs: Long,
): Boolean {
    val recentSync = lastSyncMs > 0L && (nowMs - lastSyncMs) <= recentSyncMs
    return recentSync || isMdnsDiscovered
}

/**
 * Shared online-count state published by [DevicesScreen] and consumed by
 * [com.copypaste.android.ui.SyncStatusBadge] so both the footer dot+count AND
 * every PeerCard dot are driven by the SAME single computation.
 *
 * A paired peer is ONLINE iff its IP host appears in the current live mDNS
 * `discovered` set (IP-correlation — mDNS device_id is a UUID, NOT a cert
 * fingerprint, so we match on IP only), OR its lastSyncMs falls within
 * [ONLINE_WINDOW_MS] as a fallback.
 *
 * [DevicesScreen] updates this every ~1 s via [publish]. When the Devices tab
 * is not visible, [SyncStatusBadge] falls back to its own configured-target
 * count (value stays at whatever was last published).
 *
 * ## PG-11 recency gate
 * [lastActivityMs] carries the most-recent [PairedPeer.lastSyncMs] across all
 * peers. [SyncStatusBadge] should show "connected" (green) only when this value
 * is within [RECENT_SYNC_MS] of the current wall time. A link idle for >5 min
 * should show the grey idle dot even if count > 0 (parity with macOS chip).
 */
object DevicesOnlineState {
    private val _onlineCount = MutableStateFlow(-1)
    private val _lastActivityMs = MutableStateFlow(0L)

    /** -1 = not yet computed (badge may fall back to its own logic). */
    val onlineCount: StateFlow<Int> = _onlineCount.asStateFlow()

    /**
     * Wall-clock ms of the most-recent successful peer sync across all peers,
     * or 0 when no sync has ever occurred. Published alongside [onlineCount] so
     * [SyncStatusBadge] can apply the [RECENT_SYNC_MS] recency gate (PG-11)
     * without re-reading Settings.
     */
    val lastActivityMs: StateFlow<Long> = _lastActivityMs.asStateFlow()

    /**
     * CopyPaste-lwnz: true while a sync operation (cloud poll or P2P dial) is
     * actively in flight inside [FgsSyncLoop]. Consumed by [SyncStatusBadge] to
     * drive the SYNCING badge state (green with distinct label) so the badge is
     * no longer a dead state. Set via [setSyncing]; cleared automatically when
     * the operation completes.
     *
     * Thread-safe: [MutableStateFlow.value] assignments are atomic.
     */
    private val _isSyncing = MutableStateFlow(false)
    val isSyncing: StateFlow<Boolean> = _isSyncing.asStateFlow()

    /**
     * Called by [FgsSyncLoop] immediately before starting a sync operation and
     * again (with [active]=false) when the operation finishes (success or error).
     * Safe to call from any thread.
     */
    fun setSyncing(active: Boolean) {
        _isSyncing.value = active
    }

    /**
     * CopyPaste-5917.52: true when the last sync attempt failed with a hard error
     * (backend auth failure, relay unreachable, persistent P2P dial failure) and
     * the daemon has not recovered since. Set by [FgsSyncLoop] via [setSyncError].
     *
     * When true AND the OS has internet, [resolveSyncBadgeState] returns
     * [SyncBadgeState.DaemonUnreachable] (red dot) — making [DaemonUnreachable]
     * reachable via the production code path for the first time. Previously the
     * state was only reachable via the IPC path that does not yet exist on Android.
     *
     * Thread-safe: [MutableStateFlow.value] assignments are atomic.
     */
    private val _isSyncError = MutableStateFlow(false)
    val isSyncError: StateFlow<Boolean> = _isSyncError.asStateFlow()

    /**
     * Called by [FgsSyncLoop] when a sync operation ends in a hard error
     * ([error]=true) or recovers ([error]=false). Safe to call from any thread.
     */
    fun setSyncError(error: Boolean) {
        _isSyncError.value = error
    }

    internal fun publish(count: Int, maxLastSyncMs: Long = 0L) {
        _onlineCount.value = count
        if (maxLastSyncMs > _lastActivityMs.value) {
            _lastActivityMs.value = maxLastSyncMs
        }
    }

    /**
     * PG-41: start a background polling loop that publishes [onlineCount] /
     * [lastActivityMs] every [BACKGROUND_POLL_MS] using [Settings.pairedPeers]
     * and [isPeerOnline]. Intended to be called once from
     * [CopyPasteApplication.onCreate] (or a long-lived coroutine scope) so the
     * footer badge shows the real peer count BEFORE [DevicesScreen] is ever shown,
     * removing the binary fallback in [SyncStatusBadge].
     *
     * CopyPaste-d6z3: uses [isPeerOnline] with [RECENT_SYNC_MS] so the background
     * badge count matches macOS parity (online = recentSync OR mDNS-discovered).
     * The mDNS signal is not available in this context (it lives in ClipboardService),
     * so isMdnsDiscovered=false is passed; [DevicesScreen] provides the full composite
     * signal via [onlineByFingerprint] while the screen is visible.
     *
     * Safe to call from any coroutine scope; the loop exits when the scope is
     * cancelled. Does NOT use mDNS (that lives in ClipboardService).
     *
     * Note: caller must ensure [isNativeLibraryLoaded] before starting, or wrap
     * the body in a guard, to avoid crashing on devices where the .so failed.
     */
    suspend fun startBackgroundPolling(settings: Settings) {
        while (true) {
            val peers = settings.pairedPeers
            val nowMs = System.currentTimeMillis()
            // CopyPaste-d6z3: use isPeerOnline with RECENT_SYNC_MS (5 min, macOS parity)
            // instead of the old isOnline() which used the 60 s ONLINE_WINDOW_MS gate.
            // isMdnsDiscovered=false: mDNS lives in ClipboardService, unavailable here;
            // DevicesScreen overrides with the full composite signal while visible.
            val count = peers.count { peer ->
                isPeerOnline(
                    lastSyncMs = peer.lastSyncMs,
                    isMdnsDiscovered = false,
                    nowMs = nowMs,
                    onlineWindowMs = ONLINE_WINDOW_MS,
                    recentSyncMs = RECENT_SYNC_MS,
                )
            }
            val maxLastSyncMs = peers.maxOfOrNull { it.lastSyncMs } ?: 0L
            publish(count = count, maxLastSyncMs = maxLastSyncMs)
            delay(BACKGROUND_POLL_MS)
        }
    }

    /** Poll cadence for [startBackgroundPolling] — 30 s (parity with macOS chip). */
    private const val BACKGROUND_POLL_MS = 30_000L
}
