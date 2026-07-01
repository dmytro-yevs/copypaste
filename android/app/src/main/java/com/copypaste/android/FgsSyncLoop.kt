package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Runs an incoming-sync catch-up poll loop inside the always-alive foreground
 * service, complementing the [SupabaseRealtimeClient] WebSocket push channel.
 *
 * ## Sync architecture (WS-primary, poll-as-catch-up)
 * Clips arrive primarily via the Supabase Realtime WebSocket push channel
 * ([SupabaseRealtimeClient]), which delivers new rows in ~1 s. This poll loop
 * is the **catch-up safety net** that heals rows missed while the WS was down:
 *
 *   - WS connected    → poll every 120 s (catch-up only; WS is the fast path)
 *   - WS disconnected → poll every 60 s  (more frequent while the push channel is down)
 *   - Idle            → poll every 300 s (after [IDLE_THRESHOLD_POLLS] consecutive empty polls)
 *   - On each WS (re)connect → one immediate catch-up poll (triggered by [SupabaseRealtimeClient])
 *
 * The WS and the poll share the same `(wall_time, id)` cursor persisted in
 * [Settings] and the same [ClipboardRepository.storeItemWithLww] dedup gate,
 * so a row delivered by the WS and later re-seen by the catch-up poll is a
 * silent no-op.
 *
 * ## P2P LAN dial
 * The background P2P dial runs on its own [P2P_DIAL_INTERVAL_MS] cadence,
 * decoupled from the Supabase poll interval, so the mTLS link stays fresh even
 * while the poll is backed off to the idle interval.
 *
 * ## Collaborators (CopyPaste-vp63.35)
 * This class is a thin loop shell — cadence orchestration only. Work is
 * delegated to: [SupabaseCatchUpPoller] (Supabase keyset-cursor drain),
 * [P2pDialer] (P2P dial round — denylist, mDNS refresh, HW cursor, queue
 * augment, wakelock, per-transport ack; owns the PARTIAL_WAKE_LOCK,
 * CopyPaste-y4xa), [SyncedItemStore] (inbound item mapper shared with
 * [ClipboardService]'s P2P listener), [SyncLoopPolicy] (pure scheduling/
 * backoff/filter functions — see the companion forwarders below, kept for
 * call-site compatibility), and [isOnWifi] (`NetworkUtils.kt`, Wi-Fi-only gate).
 */
class FgsSyncLoop(
    private val settings: Settings,
    private val repository: ClipboardRepository,
    private val syncManager: SyncManager,
    private val deviceKeyStore: DeviceKeyStore,
    /** WS client whose [SupabaseRealtimeClient.isConnected] gate drives the
     *  catch-up poll interval. Null-safe: when absent the loop treats WS as down. */
    private val wsClient: SupabaseRealtimeClient? = null,
    /**
     * Called AT MOST ONCE after a full Supabase catch-up drain or P2P batch with
     * the text of the NEWEST stored text clip — auto-applies it to the system
     * clipboard (only the newest, to avoid spamming/re-triggering capture).
     * Sensitive/private-mode guards already suppressed it upstream. Null
     * (default) = no auto-apply (unit tests, no live clipboard context).
     */
    private val onSyncedTextClip: ((text: String) -> Unit)? = null,
    /**
     * CopyPaste-yaip: context used to read [OutboundMutationQueue] for P2P
     * outbound selection (bypasses the wall-time HW filter for pin/reorder/
     * delete mutations). Null (unit tests, stub mode) → queue augmentation is
     * silently skipped.
     */
    private val context: android.content.Context? = null,
) {
    private var job: Job? = null

    /**
     * The scope passed to [start] — kept so [SyncedItemStore] can launch
     * thumbnail tasks cancelled on FGS destroy. Null before [start] (unit
     * tests) → falls back to an ad-hoc scope so direct [storeSyncedItem]
     * callers do not crash.
     */
    private var fgsScope: CoroutineScope? = null

    /**
     * Shared inbound-item mapper (CopyPaste-vp63.35). Consumed by both
     * [P2pDialer] (Android→macOS direction) and this class's [storeSyncedItem]
     * (macOS→Android direction, called by [ClipboardService]'s inbound listener).
     */
    private val syncedItemStore = SyncedItemStore(settings, repository) { fgsScope }

    /** Supabase catch-up poll drain collaborator (CopyPaste-vp63.35). */
    private val poller = SupabaseCatchUpPoller(settings, repository, syncManager, onSyncedTextClip)

    /** P2P dial-round collaborator (CopyPaste-vp63.35). */
    private val dialer = P2pDialer(
        settings = settings,
        repository = repository,
        deviceKeyStore = deviceKeyStore,
        syncedItemStore = syncedItemStore,
        context = context,
        onSyncedTextClip = onSyncedTextClip,
    )

    /**
     * CopyPaste-mip2: CONFLATED opportunistic P2P wake signal — at most one
     * pending signal buffered (desired debounce: N captures → ONE extra dial).
     * Producer: [signalP2pWake]. Consumer: the inner inter-dial sleep in [start].
     */
    private val p2pWakeChannel = Channel<Unit>(Channel.CONFLATED)

    /**
     * CopyPaste-1t38: backoff state for the periodic relay/cloud mutation drain.
     * [mutationDrainFailures] counts consecutive ticks that left records pending;
     * feeds [backoffMs] to compute [mutationDrainBackoffUntilMs]. Both reset to 0
     * once the queue fully drains. Read/written only inside the [start] coroutine.
     */
    private var mutationDrainFailures: Int = 0
    private var mutationDrainBackoffUntilMs: Long = 0L

    companion object {
        private const val TAG = "FgsSyncLoop"

        /** Minimum P2P dial cadence — MUST NOT change value (external callers depend on it). */
        const val P2P_DIAL_INTERVAL_MS = SyncLoopPolicy.P2P_DIAL_INTERVAL_MS

        /** CopyPaste-44rq.41: effective P2P inter-dial sleep for [consecutiveEmpty] idle dials. */
        fun p2pDialIntervalMs(consecutiveEmpty: Int): Long =
            SyncLoopPolicy.p2pDialIntervalMs(consecutiveEmpty)

        /** WS-aware steady-state catch-up poll interval. */
        fun pollIntervalMs(wsConnected: Boolean, consecutiveEmpty: Int): Long =
            SyncLoopPolicy.pollIntervalMs(wsConnected, consecutiveEmpty)

        /** M6: pure exponential-backoff computation. */
        fun backoffMs(
            failures: Int,
            base: Long = 30_000L,
            max: Long = 480_000L,
        ): Long = SyncLoopPolicy.backoffMs(failures, base, max)

        /** Legacy shim used by existing [FgsSyncLoopBackoffTest]. */
        fun intervalForEmptyStreak(consecutiveEmpty: Int): Long =
            SyncLoopPolicy.intervalForEmptyStreak(consecutiveEmpty)

        /** CopyPaste-1t38: should the periodic loop attempt a relay/cloud mutation drain? */
        fun shouldAttemptDrain(queueSize: Int, nowMs: Long, backoffUntilMs: Long): Boolean =
            SyncLoopPolicy.shouldAttemptDrain(queueSize, nowMs, backoffUntilMs)

        /** Filter [allLocalItems] to those whose wallTimeMs is STRICTLY GREATER than [outboundHighWater]. */
        fun filterByOutboundHighWater(
            allLocalItems: List<Pair<String, Long>>,
            outboundHighWater: Long,
        ): List<Pair<String, Long>> =
            SyncLoopPolicy.filterByOutboundHighWater(allLocalItems, outboundHighWater)

        /** Max wallTimeMs from a list of (id, wallTimeMs) pairs. */
        fun maxWallTime(items: List<Pair<String, Long>>): Long =
            SyncLoopPolicy.maxWallTime(items)

        /** CopyPaste-yaip (P2P gap): select mutation-queue records for P2P outbound, bypassing the HW filter. */
        fun filterQueuedMutationsForP2P(
            pending: List<OutboundMutationQueue.MutationRecord>,
            outboundHighWater: Long,
        ): List<OutboundMutationQueue.MutationRecord> =
            SyncLoopPolicy.filterQueuedMutationsForP2P(pending, outboundHighWater)

        /** CopyPaste-yaip (P2P dedup): merge local item IDs with queued-mutation item IDs. */
        fun mergeQueuedItemIdsWithLocal(
            localItemIds: Set<String>,
            queuedMutations: List<OutboundMutationQueue.MutationRecord>,
        ): Set<String> = SyncLoopPolicy.mergeQueuedItemIdsWithLocal(localItemIds, queuedMutations)

        /** Select the newest text clip from a list of (text, wallTime) pairs. */
        fun newestTextClip(clips: List<Pair<String, Long>>): String? =
            SyncLoopPolicy.newestTextClip(clips)
    }

    /**
     * Start the poll loop on [scope] (typically the FGS's IO scope).
     * Idempotent — calling while already running is a no-op.
     */
    fun start(scope: CoroutineScope) {
        if (job?.isActive == true) return
        fgsScope = scope
        job = scope.launch(Dispatchers.IO) {
            Log.i(TAG, "FgsSyncLoop started")
            var consecutiveEmpty = 0
            var consecutiveFailures = 0
            // CopyPaste-44rq.41: idle backoff for P2P dials.
            // Grows to P2P_IDLE_DIAL_INTERVAL_MS after P2P_IDLE_THRESHOLD empty
            // dials; resets to 0 (→ P2P_DIAL_INTERVAL_MS) on any activity.
            var p2pConsecutiveEmpty = 0

            while (isActive) {
                // M6: poll FIRST, then delay (dead-first-minute fix). Skip the
                // network call when sync is disabled/unconfigured, but still
                // apply the normal interval (treated as an "empty" tick).
                //
                // CopyPaste-26zi: gate on isSupabaseConfigured directly — the
                // Supabase poll runs whenever Supabase is fully configured,
                // regardless of the syncBackend UI-hint enum (relay + cloud are
                // additive, matching the macOS daemon).
                //
                // CopyPaste-agde: enforce syncOnWifiOnly — skip the network call
                // while on cellular/unavailable connectivity (treated as
                // cellular-equivalent: safe default, no sensitive data over metered).
                val isWifiRequired = settings.syncOnWifiOnly
                val isWifi = !isWifiRequired || isOnWifi(context)
                // CopyPaste-26zi: also gate on the independent per-transport enable
                // flag. Disabling Supabase in Settings must stop its poll, even when
                // it is otherwise fully configured. Relay/Supabase are additive.
                val enabled = settings.syncEnabled &&
                    settings.supabaseEnabled &&
                    settings.isSupabaseConfigured &&
                    isWifi

                val nextDelay: Long
                if (!enabled) {
                    consecutiveEmpty++
                    consecutiveFailures = 0
                    nextDelay = pollIntervalMs(
                        wsConnected = wsClient?.isConnected ?: false,
                        consecutiveEmpty = consecutiveEmpty,
                    )
                } else {
                    // CopyPaste-lwnz: signal an active sync so SyncStatusBadge
                    // can show the SYNCING state while this call is in-flight.
                    // finally{} guarantees the flag is cleared on every exit
                    // path: normal return, transient exception, or cancellation.
                    DevicesOnlineState.setSyncing(true)
                    var pollError: Exception? = null
                    val newCount = try {
                        poller.poll()
                    } catch (e: CancellationException) {
                        throw e // let coroutine cancel normally; finally clears flag
                    } catch (e: Exception) {
                        pollError = e
                        0 // dummy; handled below after finally clears the flag
                    } finally {
                        DevicesOnlineState.setSyncing(false)
                    }
                    if (pollError != null) {
                        // M6: real exponential backoff. The old code did an
                        // unconditional 30 s delay HERE and *then* delayed the
                        // full interval at the top of the next loop (double
                        // delay), while the comment falsely claimed exponential
                        // backoff. Now a single backoff governs the next wait.
                        consecutiveFailures++
                        // CopyPaste-otb7: publish the ACTUAL Supabase poll outcome so the
                        // Sync Diagnostics Supabase Connection row is sourced from backend
                        // op results, not P2P peer presence.
                        DevicesOnlineState.setSupabaseOpResult(success = false, isAuthError = true)
                        val backoff = backoffMs(consecutiveFailures)
                        Log.w(TAG, "Poll failed (#$consecutiveFailures): ${pollError.message} — backing off ${backoff}ms")
                        // CopyPaste-234q: publish authoritative badge state via Rust FFI
                        // (computeAndroidSyncBadgeState) so the wire string is derived from
                        // the single canonical source of truth. is_auth_error=true drives
                        // "error" → DaemonUnreachable (red), matching the macOS daemon's
                        // badge_state on an auth failure. Previously this was hardcoded "error".
                        val nowMs = System.currentTimeMillis()
                        val liveCountOnErr = DevicesOnlineState.onlineCount.value.toLong()
                        val badgeOnError = computeAndroidSyncBadgeState(
                            onlineCount = liveCountOnErr,
                            lastActivityMs = DevicesOnlineState.lastActivityMs.value,
                            recentSyncMs = RECENT_SYNC_MS,
                            hasInternet = isWifi, // wifi is a reasonable proxy for internet here
                            isAuthError = true,   // any poll failure = auth/config error
                            isSyncing = false,    // setSyncing(false) was called in finally above
                            nowMs = nowMs,
                        )
                        DevicesOnlineState.setBadgeState(badgeOnError)
                        delay(backoff)
                        if (!isActive) break
                        continue // re-poll immediately after the backoff sleep
                    }

                    consecutiveFailures = 0
                    // CopyPaste-otb7: publish the successful Supabase poll outcome so the
                    // Sync Diagnostics Supabase Connection row reflects real backend
                    // health independently of P2P peer presence.
                    DevicesOnlineState.setSupabaseOpResult(success = true)
                    consecutiveEmpty = if (newCount > 0) 0 else consecutiveEmpty + 1
                    if (newCount > 0) {
                        Log.d(TAG, "FgsSyncLoop: $newCount new item(s) stored")
                    }
                    // CopyPaste-234q: publish authoritative badge state via Rust FFI
                    // (computeAndroidSyncBadgeState). Previously this was a hardcoded
                    // `if (liveCount > 0) "synced" else "idle"`. Using the FFI function
                    // ensures the priority logic (auth-error > syncing > synced > offline > idle)
                    // is derived from the single Rust source of truth — making
                    // IpcSyncBadgeState live: SyncStatusBadge routes through
                    // IpcSyncBadgeState.fromIpcString → toSyncBadgeState on every tick.
                    val liveCount = DevicesOnlineState.onlineCount.value.toLong()
                    val badgeOnSuccess = computeAndroidSyncBadgeState(
                        onlineCount = liveCount,
                        lastActivityMs = DevicesOnlineState.lastActivityMs.value,
                        recentSyncMs = RECENT_SYNC_MS,
                        hasInternet = isWifi,
                        isAuthError = false,
                        isSyncing = false,    // setSyncing(false) was called in finally above
                        nowMs = System.currentTimeMillis(),
                    )
                    DevicesOnlineState.setBadgeState(badgeOnSuccess)
                    nextDelay = pollIntervalMs(
                        wsConnected = wsClient?.isConnected ?: false,
                        consecutiveEmpty = consecutiveEmpty,
                    )
                }

                // Background Android→macOS LAN P2P dial, DECOUPLED from the poll
                // delay above — sleeps out `nextDelay` in per-chunk slices via
                // [P2pDialer], on an adaptive cadence (CopyPaste-44rq.41).
                // Failures are logged, never fatal. CopyPaste-lwnz: gate the
                // SYNCING badge around the dial. CopyPaste-agde: re-check the
                // wifi gate here — the transport could change since the poll
                // check above. CopyPaste-1t38: periodic relay/cloud mutation
                // drain, bounded and backoff-governed (single-flight inside
                // SyncManager.drainOutboundMutationQueue).
                if (isWifi) {
                    drainMutationsPeriodic()
                }

                if (isWifi) {
                    dialer.lastHadActivity = false
                    DevicesOnlineState.setSyncing(true)
                    try {
                        dialer.dialPairedPeer()
                    } finally {
                        DevicesOnlineState.setSyncing(false)
                    }
                    // CopyPaste-44rq.41: update P2P idle counter based on activity
                    // signalled by dialer.dialPairedPeer() via lastHadActivity.
                    if (dialer.lastHadActivity) {
                        p2pConsecutiveEmpty = 0
                    } else {
                        p2pConsecutiveEmpty++
                    }
                }
                if (!isActive) break

                // Compute the effective P2P chunk size for the inter-poll sleep.
                // CopyPaste-44rq.41: grows after P2P_IDLE_THRESHOLD empty dials.
                val p2pChunk = p2pDialIntervalMs(p2pConsecutiveEmpty)
                var remaining = nextDelay
                while (remaining > 0 && isActive) {
                    val chunk = minOf(remaining, p2pChunk)
                    // CopyPaste-mip2: event-driven opportunistic dial.
                    // Replace plain `delay` with a wake-interruptible wait:
                    // the loop exits early when signalP2pWake() fires (clipboard
                    // capture or mDNS peer discovery) OR falls through after the
                    // timeout — keeping the periodic loop as the fallback.
                    // The CONFLATED channel collapses bursts of signals into one;
                    // P2P_WAKE_DEBOUNCE_MS documents the intended debounce semantics.
                    if (withTimeoutOrNull(chunk) { p2pWakeChannel.receive() } != null) {
                        // Woken early: a capture or mDNS event fired. Log and fall
                        // through to the re-dial check below (remaining > 0).
                        Log.d(TAG, "P2P inner sleep: woken early by signalP2pWake()")
                    }
                    if (!isActive) break
                    remaining -= chunk
                    // Re-dial on each chunk boundary that is not the final poll
                    // tick (the post-poll dial above already covers tick zero).
                    if (remaining > 0 && isOnWifi(context)) {
                        dialer.lastHadActivity = false
                        DevicesOnlineState.setSyncing(true)
                        try {
                            dialer.dialPairedPeer()
                        } finally {
                            DevicesOnlineState.setSyncing(false)
                        }
                        // CopyPaste-44rq.41: update idle counter after each inner dial.
                        if (dialer.lastHadActivity) {
                            p2pConsecutiveEmpty = 0
                        } else {
                            p2pConsecutiveEmpty++
                        }
                    }
                }
                if (!isActive) break
            }
            Log.i(TAG, "FgsSyncLoop stopped")
        }
    }

    fun stop() {
        job?.cancel()
        job = null
        fgsScope = null
    }

    /**
     * CopyPaste-1t38: drain the [OutboundMutationQueue] over relay + cloud on the
     * periodic tick, with a backoff window so a persistently-offline device does
     * not hammer the network.
     *
     * No-op when there is no Android context (unit tests / stub mode) or when the
     * queue is empty / the backoff window has not elapsed ([shouldAttemptDrain]).
     * The drain delegates to [SyncManager.drainOutboundMutationQueue], which is
     * single-flight (so this never overlaps a concurrent UI-hook drain) and applies
     * per-transport acks: a record is removed only after every enabled transport
     * acknowledges it.
     *
     * After the attempt: if records remain pending the failure counter grows and
     * the backoff window extends (via [backoffMs]); a fully-drained queue resets
     * both. p2p acks are applied separately in [P2pDialer.dialPairedPeer].
     */
    private suspend fun drainMutationsPeriodic() {
        val ctx = context ?: return
        val queueSize = runCatching { OutboundMutationQueue.queueSize(ctx) }.getOrDefault(0)
        if (!shouldAttemptDrain(queueSize, System.currentTimeMillis(), mutationDrainBackoffUntilMs)) {
            if (queueSize == 0) {
                // Healthy: reset backoff so the next enqueue drains immediately.
                mutationDrainFailures = 0
                mutationDrainBackoffUntilMs = 0L
            }
            return
        }

        val removed = try {
            syncManager.drainOutboundMutationQueue(ctx, repository)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "periodic mutation drain failed: ${e.message}")
            0
        }

        val remaining = runCatching { OutboundMutationQueue.queueSize(ctx) }.getOrDefault(0)
        if (remaining > 0) {
            // Still pending (offline / a transport is down): back off before retrying.
            mutationDrainFailures += 1
            mutationDrainBackoffUntilMs = System.currentTimeMillis() + backoffMs(mutationDrainFailures)
            Log.d(
                TAG,
                "periodic mutation drain: removed $removed, $remaining still pending — " +
                    "backing off ${backoffMs(mutationDrainFailures)}ms",
            )
        } else {
            if (removed > 0) {
                Log.i(TAG, "periodic mutation drain: flushed $removed queued mutation(s)")
            }
            mutationDrainFailures = 0
            mutationDrainBackoffUntilMs = 0L
        }
    }

    /**
     * CopyPaste-mip2: signal the P2P dial loop to wake up early.
     *
     * Called by [ClipboardService] on two events:
     *   1. A NEW clipboard item was captured locally (text, image, or file).
     *   2. An mDNS-discovered peer count goes from zero to non-zero (a peer came
     *      online on the LAN).
     *
     * The underlying [p2pWakeChannel] is CONFLATED so rapid calls collapse into
     * a single queued wakeup — the inner sleep in [start] exits at most once per
     * signal burst, preventing runaway dial spam.
     *
     * Non-blocking: [Channel.trySend] never suspends.  Safe to call from any
     * context (UI thread, IO thread, callback) without holding a lock.
     *
     * No-op when the loop has not started (stub mode / unit tests): the send
     * is buffered in the CONFLATED channel and consumed the next time [start]
     * calls receive() — or discarded if [stop] is called first and the channel
     * is never drained.
     */
    fun signalP2pWake() {
        // trySend on a CONFLATED channel always succeeds (the existing pending
        // value is overwritten) — the result is always ChannelResult.success or
        // the value-already-present no-op. Ignore the result intentionally.
        p2pWakeChannel.trySend(Unit)
    }

    /**
     * Store one [uniffi.copypaste_android.SyncedItem] received over P2P.
     * Delegates to [SyncedItemStore] (CopyPaste-vp63.35), shared with
     * [P2pDialer]'s Android→macOS direction. See [SyncedItemStore.store] for
     * the full mapping contract.
     */
    suspend fun storeSyncedItem(item: uniffi.copypaste_android.SyncedItem): Boolean =
        syncedItemStore.store(item)
}
