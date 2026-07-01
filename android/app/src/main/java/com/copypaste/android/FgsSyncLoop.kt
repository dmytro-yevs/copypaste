package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
// syncWithPeer resolves to the package-local ABI-9 wrapper in
// CopypasteBindings.kt (ByteArray sessionKey + revokedFingerprints + deviceId),
// not the generated uniffi.copypaste_android.syncWithPeer.

/**
 * Runs an incoming-sync catch-up poll loop inside the always-alive foreground
 * service, complementing the [SupabaseRealtimeClient] WebSocket push channel.
 *
 * ## Sync architecture (WS-primary, poll-as-catch-up)
 *
 * Clips arrive primarily via the Supabase Realtime WebSocket push channel
 * ([SupabaseRealtimeClient]), which delivers new rows in ~1 s after they land
 * in the database.  This poll loop is the **catch-up safety net** that heals
 * any rows missed while the WS was down (Doze, OEM kills, network flap):
 *
 *   - WS connected   → poll every 120 s (catch-up only; WS is the fast path)
 *   - WS disconnected→ poll every 60 s  (more frequent while the push channel
 *                       is down so incoming clips are not delayed too long)
 *   - Idle           → poll every 300 s (both states, after [IDLE_THRESHOLD_POLLS]
 *                       consecutive empty polls while the FGS is alive)
 *   - On each WS (re)connect → one immediate catch-up poll (triggered by the
 *     WS client itself via [SupabaseRealtimeClient])
 *
 * The WS and the poll share the same `(wall_time, id)` cursor persisted in
 * [Settings] and the same [ClipboardRepository.storeItemWithLww] dedup gate,
 * so a row delivered by the WS and later re-seen by the catch-up poll is a
 * silent no-op.
 *
 * ## P2P LAN dial
 * The background P2P dial runs on its own [P2P_DIAL_INTERVAL_MS] cadence,
 * decoupled from the Supabase poll interval.  The poll delay can grow to the
 * idle cap, but P2P dials still fire frequently so the mTLS link is established
 * quickly.
 *
 * ## Cursor strategy (Tasks 4/5/6)
 * Uses an ascending compound keyset cursor (wall_time, id) that mirrors the
 * macOS daemon's `build_poll_url`. For every row in the batch — including
 * self-echo (own deviceId) rows and blank rows — the cursor is advanced BEFORE
 * any `continue`. This prevents stalling on a batch of own-device rows.
 *
 * ## LWW replace (Task 5)
 * When an incoming row's item_id already exists locally, the incoming
 * lamport_ts is compared to the stored row's. If strictly newer, the local
 * row is replaced (last-writer-wins), mirroring the daemon's cloud.rs LWW.
 *
 * ## Retry backoff
 * - RETRY_BACKOFF_BASE_MS = 30_000 (30 s) — first retry after a transient error;
 *   doubles each consecutive failure up to RETRY_BACKOFF_MAX_MS (real exponential
 *   backoff, reset to 0 failures on the first success).
 *
 * WakeLock: an explicit PARTIAL_WAKE_LOCK is acquired for the duration of each
 * [dialPairedPeer] → [syncWithPeer] call (CopyPaste-y4xa). Foreground services
 * on Android 8+ implicitly prevent CPU sleep via the notification, but OEM schedulers
 * (Xiaomi MIUI, Oppo ColorOS, Samsung One UI) can suspend the CPU mid-handshake.
 * The lock is released in a finally block on every exit path.
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
     * Called AT MOST ONCE after a full Supabase catch-up drain or P2P batch
     * with the text of the NEWEST (highest wall_time) text clip that was stored.
     *
     * Intent: auto-apply the latest synced text clip to the system clipboard so
     * the user can paste it immediately — but only the newest one, not every clip
     * in the batch (which would spam the clipboard and could re-trigger capture
     * loops).
     *
     * Sensitive and private-mode guards in the store path already suppress the
     * text before it reaches here: this callback only fires for clips that were
     * actually stored.
     *
     * Null (default) means "no auto-apply" — used by unit tests and callers that
     * do not have a live system clipboard context.
     */
    private val onSyncedTextClip: ((text: String) -> Unit)? = null,
    /**
     * CopyPaste-yaip: Android context used to read [OutboundMutationQueue] during
     * P2P outbound selection. Required to bypass the wall-time high-water filter for
     * pin/reorder/delete mutations that only bump `lamport_ts`. Null when no context
     * is available (unit tests, stub mode) — in that case the queue-augmentation
     * step is silently skipped and only the wall-time filter is applied.
     */
    private val context: android.content.Context? = null,
) {
    private var job: Job? = null

    /**
     * The scope passed to [start] — kept so [storeSyncedItem] can launch
     * thumbnail generation tasks that are cancelled when the FGS is destroyed.
     * Null before [start] (unit tests, stub mode) → falls back to an ad-hoc
     * scope so callers that invoke [storeSyncedItem] directly do not crash.
     */
    private var fgsScope: CoroutineScope? = null

    /**
     * CopyPaste-mip2: CONFLATED channel used as an opportunistic P2P wake signal.
     *
     * CONFLATED capacity means at most one pending signal is buffered; if the
     * channel already has a signal queued the new signal is silently dropped —
     * this is the desired debounce behaviour (N captures within a single inter-dial
     * sleep result in exactly ONE extra dial, not N).
     *
     * Producers: [signalP2pWake] (called by ClipboardService on capture or mDNS
     * peer discovery).  Consumer: the inner inter-dial sleep in [start] via
     * `withTimeoutOrNull(chunk) { p2pWakeChannel.receive() }`.
     */
    private val p2pWakeChannel = Channel<Unit>(Channel.CONFLATED)

    /**
     * CopyPaste-44rq.41: set to true inside [dialPairedPeer] whenever at least
     * one item was sent or received across any peer.  Read and reset by the
     * [start] loop immediately after each dial round to update
     * [p2pConsecutiveEmpty] and compute the adaptive P2P interval.
     *
     * Not thread-safe by itself, but both the writer ([dialPairedPeer]) and
     * reader ([start]) run on [Dispatchers.IO] inside the same coroutine (the
     * P2P dial is always `suspend`-called inline, never as a separate launch),
     * so no extra synchronisation is needed.
     */
    private var lastP2pHadActivity: Boolean = false

    /**
     * CopyPaste-1t38: backoff state for the periodic relay/cloud mutation drain.
     *
     * [mutationDrainFailures] counts consecutive ticks where a drain left records
     * still pending (offline / transport down); it feeds [backoffMs] to compute
     * [mutationDrainBackoffUntilMs], the wall-clock instant before which the next
     * drain attempt is skipped. Both reset to 0 the moment the queue fully drains.
     *
     * Read/written only inside the single [start] coroutine, so no synchronisation.
     */
    private var mutationDrainFailures: Int = 0
    private var mutationDrainBackoffUntilMs: Long = 0L

    companion object {
        private const val TAG = "FgsSyncLoop"

        /**
         * Minimum P2P dial cadence. Delegates to [SyncLoopPolicy.P2P_DIAL_INTERVAL_MS]
         * (extracted CopyPaste-vp63.35). Kept here — MUST NOT change value — so
         * external callers ([ClipboardService] inbound-listener drain cadence) and
         * internal unqualified references are unaffected.
         */
        const val P2P_DIAL_INTERVAL_MS = SyncLoopPolicy.P2P_DIAL_INTERVAL_MS

        /**
         * CopyPaste-44rq.41: compute the effective P2P inter-dial sleep given
         * the number of consecutive idle dials.
         * Delegates to [SyncLoopPolicy.p2pDialIntervalMs] (CopyPaste-vp63.35).
         */
        fun p2pDialIntervalMs(consecutiveEmpty: Int): Long =
            SyncLoopPolicy.p2pDialIntervalMs(consecutiveEmpty)

        /**
         * WS-aware steady-state catch-up poll interval.
         * Delegates to [SyncLoopPolicy.pollIntervalMs] (CopyPaste-vp63.35).
         */
        fun pollIntervalMs(wsConnected: Boolean, consecutiveEmpty: Int): Long =
            SyncLoopPolicy.pollIntervalMs(wsConnected, consecutiveEmpty)

        /**
         * M6: pure exponential-backoff computation.
         * Delegates to [SyncLoopPolicy.backoffMs] (CopyPaste-vp63.35).
         */
        fun backoffMs(
            failures: Int,
            base: Long = 30_000L,
            max: Long = 480_000L,
        ): Long = SyncLoopPolicy.backoffMs(failures, base, max)

        /**
         * Legacy shim used by existing [FgsSyncLoopBackoffTest].
         * Delegates to [SyncLoopPolicy.intervalForEmptyStreak] (CopyPaste-vp63.35).
         */
        fun intervalForEmptyStreak(consecutiveEmpty: Int): Long =
            SyncLoopPolicy.intervalForEmptyStreak(consecutiveEmpty)

        /**
         * CopyPaste-1t38: should the periodic loop attempt a relay/cloud mutation
         * drain on this tick? Delegates to [SyncLoopPolicy.shouldAttemptDrain]
         * (CopyPaste-vp63.35).
         */
        fun shouldAttemptDrain(queueSize: Int, nowMs: Long, backoffUntilMs: Long): Boolean =
            SyncLoopPolicy.shouldAttemptDrain(queueSize, nowMs, backoffUntilMs)

        /**
         * Filter [allLocalItems] to only those items whose wallTimeMs is
         * STRICTLY GREATER than [outboundHighWater].
         * Delegates to [SyncLoopPolicy.filterByOutboundHighWater] (CopyPaste-vp63.35).
         */
        fun filterByOutboundHighWater(
            allLocalItems: List<Pair<String, Long>>,
            outboundHighWater: Long,
        ): List<Pair<String, Long>> =
            SyncLoopPolicy.filterByOutboundHighWater(allLocalItems, outboundHighWater)

        /**
         * Compute the max wallTimeMs from a list of (id, wallTimeMs) pairs.
         * Delegates to [SyncLoopPolicy.maxWallTime] (CopyPaste-vp63.35).
         */
        fun maxWallTime(items: List<Pair<String, Long>>): Long =
            SyncLoopPolicy.maxWallTime(items)

        /**
         * CopyPaste-yaip (P2P gap): select mutation-queue records for P2P outbound,
         * BYPASSING the wall-time high-water filter.
         * Delegates to [SyncLoopPolicy.filterQueuedMutationsForP2P] (CopyPaste-vp63.35).
         */
        fun filterQueuedMutationsForP2P(
            pending: List<OutboundMutationQueue.MutationRecord>,
            outboundHighWater: Long,
        ): List<OutboundMutationQueue.MutationRecord> =
            SyncLoopPolicy.filterQueuedMutationsForP2P(pending, outboundHighWater)

        /**
         * CopyPaste-yaip (P2P dedup): merge the item IDs of locally-stored items
         * with the item IDs of queued mutations, deduplicating on item ID.
         * Delegates to [SyncLoopPolicy.mergeQueuedItemIdsWithLocal] (CopyPaste-vp63.35).
         */
        fun mergeQueuedItemIdsWithLocal(
            localItemIds: Set<String>,
            queuedMutations: List<OutboundMutationQueue.MutationRecord>,
        ): Set<String> = SyncLoopPolicy.mergeQueuedItemIdsWithLocal(localItemIds, queuedMutations)

        /**
         * Select the newest text clip from a list of (text, wallTime) pairs
         * accumulated across a bulk-sync batch drain.
         * Delegates to [SyncLoopPolicy.newestTextClip] (CopyPaste-vp63.35).
         */
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
                // M6: poll FIRST, then delay. The previous loop delayed a full
                // POLL_INTERVAL_MS *before* the first poll, so incoming sync was
                // dead for the first minute after the FGS started.
                //
                // Skip the network call when sync is disabled/unconfigured, but
                // still apply the normal interval (treated as an "empty" tick).
                //
                // CopyPaste-26zi: gate on isSupabaseConfigured directly — NOT on
                // syncBackend == SUPABASE. The syncBackend enum is a UI hint for the
                // settings screen; the Supabase poll should run whenever Supabase is
                // fully configured, regardless of which backend the enum points to.
                // macOS daemon runs relay + cloud additively; Android must match.
                //
                // CopyPaste-agde: enforce syncOnWifiOnly — when the user enables
                // "Sync on Wi-Fi only", skip the network call while the device is on
                // cellular or offline. isOnWifi() checks the active-network transport
                // via ConnectivityManager; it returns true on Wi-Fi AND Ethernet, but
                // false on cellular, VPN-only, and unavailable networks.
                // The guard treats unavailable connectivity as cellular-equivalent
                // (safe: skip rather than push sensitive data on metered networks).
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
                        poll()
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
                // delay above. Whenever we hold a complete set of persisted
                // pairing credentials we dial the paired peer so a one-time pair
                // keeps syncing unattended. The P2P link is the priority
                // transport, so we dial it on an adaptive cadence — short
                // ([P2P_DIAL_INTERVAL_MS]) while active, growing to
                // [P2P_IDLE_DIAL_INTERVAL_MS] after [P2P_IDLE_THRESHOLD]
                // consecutive empty dials (CopyPaste-44rq.41) — even while the
                // Supabase poll is backed off to the idle interval. We sleep out
                // `nextDelay` in per-chunk slices: dial, sleep one chunk, repeat,
                // until the poll is due again. Failures are logged, never fatal.
                // CopyPaste-lwnz: gate the SYNCING badge around the P2P dial too.
                // CopyPaste-agde: re-check wifi gate before P2P dial — the transport
                // could change between the poll check above and the dial below.
                // CopyPaste-1t38: periodic relay/cloud mutation drain. Previously
                // the OutboundMutationQueue was drained ONLY from UI mutation hooks
                // and service startup; the periodic loop only peeked it for P2P
                // augmentation and never drained relay/cloud. A mutation enqueued
                // while offline therefore stayed unsent until a new UI action or a
                // restart. Draining here means the queue flushes within one loop
                // interval after connectivity returns — no UI action required.
                // Bounded (skips when empty), backoff-governed, and single-flight
                // inside SyncManager.drainOutboundMutationQueue.
                if (isWifi) {
                    drainMutationsPeriodic()
                }

                if (isWifi) {
                    lastP2pHadActivity = false
                    DevicesOnlineState.setSyncing(true)
                    try {
                        dialPairedPeer()
                    } finally {
                        DevicesOnlineState.setSyncing(false)
                    }
                    // CopyPaste-44rq.41: update P2P idle counter based on activity
                    // signalled by dialPairedPeer() via lastP2pHadActivity.
                    if (lastP2pHadActivity) {
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
                        lastP2pHadActivity = false
                        DevicesOnlineState.setSyncing(true)
                        try {
                            dialPairedPeer()
                        } finally {
                            DevicesOnlineState.setSyncing(false)
                        }
                        // CopyPaste-44rq.41: update idle counter after each inner dial.
                        if (lastP2pHadActivity) {
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
     * both. p2p acks are applied separately in [dialPairedPeer].
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
     * Perform one poll cycle using the compound keyset cursor.
     *
     * For every row in the batch (Tasks 4/5/6):
     *   1. Advance the (wall_time, id) cursor BEFORE any continue — so a batch
     *      of only own-device rows still moves the cursor forward.
     *   2. Skip self-echo rows (own deviceId).
     *   3. Decrypt; skip if decryption fails.
     *   4. Skip blank plaintext.
     *   5. LWW replace: if item_id exists locally with an older lamport_ts,
     *      replace it; otherwise skip as a dup.
     *
     * Returns the number of new/replaced items stored.
     */
    private suspend fun poll(): Int = withContext(Dispatchers.IO) {
        // Drain loop: a full batch (size == POLL_LIMIT) almost certainly means
        // the server has more rows waiting. Re-poll IMMEDIATELY in that case
        // instead of returning and waiting the idle delay — otherwise a backlog
        // of N rows would drain at only POLL_LIMIT rows per poll interval
        // (~20/min). On a SHORT batch (< POLL_LIMIT) we have caught up, so we
        // break and let the caller apply the normal idle delay.
        //
        // Each iteration runs the original single-cycle logic unchanged (LWW,
        // compound (wall_time, id) cursor, self-echo skip). The cursor is
        // persisted after every cycle, so a re-poll continues from where the
        // previous cycle left off.
        var totalNewCount = 0
        // Accumulate (text, wallTime) for every text clip stored across ALL
        // batch cycles in this drain. After the full drain, we apply only the
        // NEWEST text clip once — not one per item (which would spam the system
        // clipboard and could re-trigger the capture loop).
        val storedTextClips = mutableListOf<Pair<String, Long>>()
        while (isActive) {
            val batch = syncManager.pollFromSupabase(
                sinceWallTime = settings.lastSupabasePollWallTime,
                sinceId = settings.lastSupabasePollId,
            ) ?: break

            var newCount = 0
            val startWallTime = settings.lastSupabasePollWallTime
            val startId = settings.lastSupabasePollId
            var cursorWallTime = startWallTime
            var cursorId = startId
            // CopyPaste-44rq.36: collect (storedId, imageBytes) pairs during the
            // batch loop; thumbnail generation is deferred to AFTER the loop so the
            // cursor is advanced for ALL items before the CPU-bound decode/compress
            // work starts. Thumbnails are then generated in parallel on Dispatchers.Default.
            val pendingThumbnails = mutableListOf<Pair<String, ByteArray>>()

            for (row in batch.rows) {
                // Task 6: advance cursor for EVERY row before any continue.
                if (row.wallTime > cursorWallTime ||
                    (row.wallTime == cursorWallTime && row.id > cursorId)) {
                    cursorWallTime = row.wallTime
                    cursorId = row.id
                }

                // Skip own-device rows (self-echo from our push).
                if (row.deviceId == settings.deviceId) continue

                // Decrypt; skip rows that fail (wrong key, tampered blob).
                val item = batch.client.decryptRow(row, batch.syncKey) ?: continue

                // CopyPaste-up1c: tombstone fast-path — mirrors daemon cloud.rs ~line 2659.
                // A deleted row carries deleted=true and empty plaintext; route to
                // applyInboundTombstoneWithLww (handles ghost tombstone for delete-before-create).
                if (item.deleted) {
                    val tombstoned = repository.applyInboundTombstoneWithLww(
                        itemId = item.itemId,
                        lamportTs = item.lamportTs,
                    )
                    if (tombstoned) newCount++
                    continue
                }

                val isImage = item.contentType == "image" ||
                    item.contentType.startsWith("image/")
                val isFile = item.contentType == "file"

                val stored = if (isImage) {
                    // Image row: store a placeholder entry then persist raw bytes.
                    // storeItem deduplicates via overrideId so re-polls are no-ops.
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val storedId = repository.storeItem(
                            plaintext = "[image]",
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                            originDeviceId = item.deviceId,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, item.plaintext)
                            // CopyPaste-44rq.36: defer thumbnail generation — queue the pair
                            // so all cursors advance before any CPU-bound decode/compress work.
                            pendingThumbnails.add(storedId to item.plaintext)
                            true
                        } else {
                            false
                        }
                    }
                } else if (isFile) {
                    // File row: store actual bytes so the user can save/copy them.
                    // CopyPaste-1jms.35: decryptRow decodes the in-band file-identity
                    // header, so DecryptedItem.fileName/fileMime ARE populated for
                    // cloud-polled files — pass them through (like the relay/P2P path)
                    // so the row shows "[file: report.pdf]" with its real MIME instead
                    // of "[file]" + null metadata.
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val label = SyncFileHelper.buildFileLabel(item.fileName)
                        val storedId = repository.storeItem(
                            plaintext = label,
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                            originDeviceId = item.deviceId,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeFileBytes(storedId, item.plaintext)
                            repository.storeFileMeta(storedId, item.fileName, item.fileMime)
                            true
                        } else {
                            false
                        }
                    }
                } else {
                    // Text row: LWW replace — replace only when incoming lamport_ts
                    // is strictly newer than the locally stored row for the same item_id.
                    val text = item.plaintext.toString(Charsets.UTF_8)
                    if (text.isBlank()) {
                        false
                    } else {
                        val didStore = repository.storeItemWithLww(
                            plaintext = text,
                            key = settings.encryptionKey,
                            itemId = item.itemId,
                            incomingLamportTs = item.lamportTs,
                            wallTimeMs = item.wallTime,
                            originDeviceId = item.deviceId,
                        )
                        // Track this text clip for the post-drain auto-apply
                        // selection; we only apply the newest one at the end.
                        if (didStore) storedTextClips.add(text to row.wallTime)
                        didStore
                    }
                }

                // lcmq: apply authoritative pin state (pin/unpin/reorder) from cloud row.
                // Uses applyAuthoritativePinState — not setPinned — so authoritative unpins
                // and pin_order convergence work without minting a new local mutation.
                if (stored) {
                    repository.applyAuthoritativePinState(item.itemId, item.pinned, item.pinOrder)
                }

                if (stored) newCount++
            }

            // Persist the advanced cursor after processing the full batch.
            // advanceSupabaseCursor is monotonic and holds supabaseCursorLock so
            // a concurrent SupabasePollWorker run cannot interleave and lose an advance.
            settings.advanceSupabaseCursor(cursorWallTime, cursorId)

            // CopyPaste-44rq.36: generate thumbnails for all images in this batch in
            // parallel AFTER the cursor is advanced. Cursor advancement is the critical
            // path; thumbnail generation (50–200 ms per image) is not.
            if (pendingThumbnails.isNotEmpty()) {
                coroutineScope {
                    pendingThumbnails.map { (storedId, imageBytes) ->
                        async(Dispatchers.Default) {
                            SyncThumbnailHelper.generateAndStore(imageBytes) { thumbBytes ->
                                repository.storeThumbnailBytes(storedId, thumbBytes)
                            }
                        }
                    }.awaitAll()
                }
                pendingThumbnails.clear()
            }

            totalNewCount += newCount

            // Short batch → caught up. Stop draining and return.
            if (batch.rows.size < SupabaseClient.POLL_LIMIT) break

            // Safety: if a full batch somehow failed to advance the cursor,
            // break rather than spin forever re-fetching the same window.
            if (cursorWallTime == startWallTime && cursorId == startId) break
        }

        // Auto-apply: after the full drain, apply only the NEWEST text clip once.
        // This prevents N clipboard overwrites for a batch of N items and avoids
        // re-triggering the capture loop for intermediate clips.
        newestTextClip(storedTextClips)?.let { text ->
            onSyncedTextClip?.invoke(text)
        }

        totalNewCount
    }

    /**
     * One background P2P dial against the paired macOS peer (Android-as-initiator),
     * reusing the credentials persisted by [PairActivity] at pairing time.
     *
     * Gated by [P2pDialerGate.shouldDial]: only runs when the peer address,
     * fingerprint, and the KEK-wrapped PAKE session key are all present. The FFI
     * call mirrors `PairActivity.runPairAndSync` exactly, minus the
     * `bootstrapPairInitiator` step (that produced the now-persisted session key).
     *
     * All failures (no LAN route, peer asleep, TLS/handshake error) are caught
     * and logged — the loop must never crash the foreground service.
     *
     * NOTE: this only drives the Android→macOS direction. macOS→Android still
     * requires an Android-side mTLS listener, which does not exist yet (see the
     * note in PairActivity.runPairAndSync).
     */
    private suspend fun dialPairedPeer() = withContext(Dispatchers.IO) {
        // Gate on both syncEnabled and p2pSyncEnabled so the user's toggle is honoured.
        // Without this guard P2P dials fire even when the user disabled P2P (HW-A9 inert).
        if (!settings.syncEnabled || !settings.p2pSyncEnabled) return@withContext

        val peers = settings.pairedPeers
        if (peers.isEmpty()) return@withContext

        // A device cert is mandatory for mTLS; if pairing never generated one
        // there is nothing to dial with.
        val cert = deviceKeyStore.peek() ?: run {
            Log.w(TAG, "P2P dial skipped: no device cert (never paired?)")
            return@withContext
        }

        val key = settings.encryptionKey

        // Load the local denylist ONCE per pass. It is used twice:
        //   (a) to skip dialing any peer we have locally revoked, and
        //   (b) passed into syncWithPeer so the native side refuses to ingest
        //       items from any revoked fingerprint (server-side enforcement).
        //
        // SECURITY (fail-closed): if we cannot load the revoked-fingerprint list
        // we MUST NOT proceed with an empty denylist — doing so would allow a sync
        // to a previously-revoked peer.  Log at ERROR and abort the entire dial
        // pass; the next tick will retry.
        val revoked = try {
            listRevokedFingerprints(settings.dbPath, key)
        } catch (e: Exception) {
            Log.e(
                TAG,
                "dialPairedPeer: ABORTING dial pass — listRevokedFingerprints failed " +
                    "and proceeding with an empty denylist would allow sync to revoked peers: ${e.message}",
                e,
            )
            return@withContext
        }

        // Load ALL local items once; each peer's outbound high-water cursor
        // is applied per-peer below to avoid re-loading for every peer.
        val allLocalItems = repository.localItemsForSync(key)

        // Snapshot the LAN discovery table ONCE per pass. Used by the per-peer
        // mDNS IP-correlation fallback below. listDiscovered can throw if the
        // native side is not yet started; treat that as "no peers discovered".
        val discovered = runCatching {
            listDiscovered(peers.map { it.fingerprint })
        }.getOrElse { e ->
            Log.d(TAG, "listDiscovered unavailable during dial pass: ${e.message}")
            emptyList()
        }

        // Iterate every paired peer. Per-peer try/catch so one unreachable or
        // failing peer does not abort dials to the others.
        for (peer in peers) {
            val peerFingerprint = peer.fingerprint
            val sessionKey = settings.sessionKeyFor(peerFingerprint)

            // (a) Local denylist: never dial a peer we revoked.
            if (peerFingerprint in revoked) {
                Log.i(TAG, "P2P dial: skipping revoked peer ${peerFingerprint.take(8)}")
                // CopyPaste-ah3i: zero sessionKey even on early skip to minimize heap exposure.
                sessionKey.fill(0)
                continue
            }

            // Resolve the best available dial address. Start with the persisted
            // syncAddr, then apply a proactive mDNS IP-correlation refresh so we
            // use the peer's current ephemeral port even on the FIRST dial attempt
            // after a Mac daemon restart.  This mirrors the Mac-side
            // `resolve_addr_from_discovery_by_ip` fix.  The mDNS `device_id` is a
            // per-device UUID — it never equals the cert fingerprint — so we
            // correlate by the LAN IP instead.
            val persistedAddr = peer.syncAddr
            val peerAddr = resolveAddrByIp(persistedAddr, discovered) ?: persistedAddr

            // If mDNS gave us a fresher address, persist it so the next tick starts
            // from the correct port without re-correlating every time.
            if (peerAddr != persistedAddr && peerAddr.isNotBlank()) {
                Log.i(
                    TAG,
                    "P2P dial ${peerFingerprint.take(8)}: mDNS pre-refresh " +
                        "$persistedAddr → $peerAddr — persisting",
                )
                runCatching {
                    settings.upsertPeer(peer.copy(syncAddr = peerAddr))
                }.onFailure { e ->
                    Log.w(TAG, "Failed to persist refreshed addr for ${peerFingerprint.take(8)}: ${e.message}")
                }
            }

            if (!P2pDialerGate.shouldDial(peerAddr, peerFingerprint, sessionKey)) {
                // CopyPaste-ah3i: zero sessionKey on gate skip to minimize heap exposure.
                sessionKey.fill(0)
                continue
            }

            // P2P outbound high-water cursor: only send items NEWER than the
            // last successfully-synced wall_time for this peer.  On the first
            // dial (cursor == 0) all local items are included.  A partial/failed
            // dial leaves the cursor unchanged so the next dial retransmits the
            // same window — no data is lost.
            val outboundHw = settings.p2pOutboundHighWater(peerFingerprint)
            val hwFiltered = if (outboundHw == 0L) {
                allLocalItems
            } else {
                allLocalItems.filter { it.wallTimeMs > outboundHw }
            }

            // CopyPaste-yaip (P2P gap): augment the wall-time-filtered list with
            // any items from the outbound mutation queue that were excluded because
            // their wallTimeMs == outboundHw (pin/reorder mutations only bump
            // lamport_ts, not wallTime). Without this, pin/reorder/delete mutations
            // are silently dropped on every P2P dial after the first full sync.
            //
            // Strategy: read the pending queue, find the subset whose itemId exists
            // in allLocalItems (so we have the actual encrypted bytes to send), and
            // union those items into the outbound set. Items in the queue whose itemId
            // does NOT exist locally (e.g. the item was physically deleted after the
            // mutation was queued) are skipped — they propagate via the tombstone
            // path already present in allLocalItems (isDeletedBlob rows).
            //
            // The union deduplicates by identity: allLocalItems items are UniFFI
            // structs that don't implement equals(), so we build a Set<String> of
            // already-included itemIds and skip duplicates. Tombstone rows (deleted=true)
            // are already in allLocalItems from localItemsForSync — no extra handling needed.
            val localItems = if (outboundHw == 0L || context == null) {
                // First dial or no context: full set already; no queue augmentation needed.
                hwFiltered
            } else {
                val pendingMutations = runCatching {
                    OutboundMutationQueue.peekQueue(context)
                }.getOrElse { e ->
                    Log.w(TAG, "dialPairedPeer: could not read mutation queue for P2P augment: ${e.message}")
                    emptyList()
                }
                if (pendingMutations.isEmpty()) {
                    hwFiltered
                } else {
                    // Build an index of allLocalItems by itemId for O(1) lookup.
                    val localIndex: Map<String, uniffi.copypaste_android.LocalItem> =
                        allLocalItems.associateBy { it.itemId }
                    // IDs already selected by the HW filter (to avoid double-sending).
                    val alreadySelected = hwFiltered.map { it.itemId }.toHashSet()
                    // Select items from the queue that are present locally but excluded by HW filter.
                    val queueAugments = pendingMutations
                        .filter { it.itemId !in alreadySelected }
                        .mapNotNull { localIndex[it.itemId] }
                    if (queueAugments.isNotEmpty()) {
                        Log.d(
                            TAG,
                            "dialPairedPeer ${peerFingerprint.take(8)}: augmenting P2P outbound " +
                                "with ${queueAugments.size} mutation-queue item(s) bypassing HW filter",
                        )
                        hwFiltered + queueAugments
                    } else {
                        hwFiltered
                    }
                }
            }

            // CopyPaste-yaip (P2P durable ack): the (itemId, lamportTs) keys of any
            // queued mutations whose item is actually being SENT in this dial. After
            // a successful syncWithPeer we ack the p2p transport for these records so
            // the durable queue can finally drop them once relay + cloud have also
            // acked. Without this, p2p only ever PEEKED the queue and never confirmed
            // delivery, so a record needing p2p would never converge.
            val p2pAckKeys: List<Pair<String, Long>> = if (context == null) {
                emptyList()
            } else {
                val sentItemIds = localItems.map { it.itemId }.toHashSet()
                runCatching { OutboundMutationQueue.peekQueue(context) }
                    .getOrDefault(emptyList())
                    .filter { it.itemId in sentItemIds }
                    .map { it.itemId to it.lamportTs }
            }

            // CopyPaste-y4xa: acquire a PARTIAL_WAKE_LOCK for the duration of the
            // mTLS handshake + data exchange. An FGS notification keeps the CPU on
            // under normal conditions, but OEM schedulers (Xiaomi MIUI, Oppo ColorOS,
            // Samsung One UI Doze) can suspend the CPU when the screen turns off even
            // inside a foreground service. A mid-handshake suspend orphans the TLS
            // connection and causes the next restart to find a failed/stale socket.
            // The lock is always released in the finally block — no leak risk.
            //
            // Tag format follows Android convention: "<package>/ClassName:purpose".
            // The timeout (60 000 ms) is a safety net only — syncWithPeer should
            // complete well within 30 s on a LAN; this prevents a hung native thread
            // from holding the lock indefinitely.
            val wakeLock = context?.let { ctx ->
                val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE)
                    as? android.os.PowerManager
                pm?.newWakeLock(
                    android.os.PowerManager.PARTIAL_WAKE_LOCK,
                    "com.copypaste.android/FgsSyncLoop:p2pDial",
                )?.apply { acquire(60_000L) }
            }
            try {
            val result = syncWithPeer(
                peerAddr = peerAddr,
                peerFingerprint = peerFingerprint,
                sessionKey = sessionKey,
                certDer = cert.certDer,
                keyDer = cert.keyDer,
                localItems = localItems,
                revokedFingerprints = revoked,
                deviceId = settings.deviceId,
            )
            // 8i3q: stamp the contact time immediately after a successful
            // TCP/TLS handshake — syncWithPeer returning without throwing IS
            // the handshake proof, regardless of item count. This keeps the
            // 60s ONLINE_WINDOW alive on every 30s dial tick even when there
            // are zero items to exchange. Best-effort: a write failure here
            // must not abort item processing or the remaining peers.
            runCatching {
                settings.updatePeerLastSync(peerFingerprint, System.currentTimeMillis())
            }.onFailure { e ->
                Log.w(TAG, "Failed to stamp lastSyncMs for ${peerFingerprint.take(8)}: ${e.message}")
            }
            var stored = 0
            // Accumulate text clips from this P2P batch; apply only the newest
            // after the full set is stored — mirrors the Supabase drain logic.
            val p2pTextClips = mutableListOf<Pair<String, Long>>()
            // Track the highest wallTimeMs received from the peer so we can
            // advance the inbound high-water cursor after a successful sync.
            var maxInboundWallTime = settings.p2pInboundHighWater(peerFingerprint)
            for (item in result.items) {
                // Store-mapping shared with the inbound listener poll (Android-as-
                // responder). LWW dedup on item_id makes a re-dial / re-receipt a
                // no-op, so no extra dedup is needed across the two paths.
                val didStore = storeSyncedItem(item)
                if (didStore) {
                    stored += 1
                    val isText = item.contentType != "image" &&
                        !item.contentType.startsWith("image/") &&
                        item.contentType != "file"
                    if (isText) {
                        val text = String(
                            ByteArray(item.plaintext.size) { item.plaintext[it].toByte() },
                            Charsets.UTF_8,
                        )
                        if (text.isNotBlank()) p2pTextClips.add(text to item.wallTimeMs)
                    }
                }
                // Advance inbound high-water regardless of whether the item was
                // stored: a deduped item still proves we've seen this wall_time.
                if (item.wallTimeMs > maxInboundWallTime) {
                    maxInboundWallTime = item.wallTimeMs
                }
            }
            if (result.itemsReceived > 0uL || result.itemsSent > 0uL) {
                Log.i(
                    TAG,
                    "P2P dial ${peerFingerprint.take(8)}: received ${result.itemsReceived} " +
                        "(stored $stored), sent ${result.itemsSent}",
                )
                // CopyPaste-44rq.41: signal activity so the start() loop can
                // reset the P2P idle backoff counter for this dial round.
                lastP2pHadActivity = true
            }
            // Auto-apply the newest P2P text clip once (not per item).
            newestTextClip(p2pTextClips)?.let { text ->
                onSyncedTextClip?.invoke(text)
            }

            // Advance the outbound high-water cursor to the max wallTimeMs among
            // items we just sent.  Only advance when we actually sent something —
            // an empty localItems list means the cursor is already correct.
            if (localItems.isNotEmpty()) {
                val maxSentWallTime = localItems.maxOf { it.wallTimeMs }
                settings.advanceP2pOutboundHighWater(peerFingerprint, maxSentWallTime)
                Log.d(
                    TAG,
                    "P2P dial ${peerFingerprint.take(8)}: advanced outbound HW → $maxSentWallTime " +
                        "(sent ${localItems.size} items)",
                )
            }

            // Advance the inbound high-water cursor to the max wallTimeMs received.
            settings.advanceP2pInboundHighWater(peerFingerprint, maxInboundWallTime)

            // CopyPaste-yaip (P2P durable ack): syncWithPeer returned without
            // throwing → the handshake + item exchange succeeded. Ack the p2p
            // transport for the queued mutations whose item we just sent, so the
            // durable queue can drop them once relay + cloud have also acked. The
            // ack is per-transport: relay/cloud acks come from SyncManager's drain,
            // p2p from here — applyAcks removes a record only when ALL enabled
            // transports have confirmed it.
            if (p2pAckKeys.isNotEmpty()) {
                context?.let { ctx ->
                    val enabled = OutboundMutationQueue.enabledTransports(
                        relay = settings.isRelayConfigured,
                        supabase = settings.isSupabaseConfigured,
                        p2p = settings.p2pSyncEnabled && settings.pairedPeers.isNotEmpty(),
                    )
                    val p2pAcks = p2pAckKeys.associateWith {
                        setOf(OutboundMutationQueue.TRANSPORT_P2P)
                    }
                    runCatching {
                        OutboundMutationQueue.applyAcks(ctx, p2pAcks, enabled)
                    }.onFailure { e ->
                        Log.w(TAG, "dialPairedPeer: p2p applyAcks failed: ${e.message}")
                    }
                }
            }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.w(TAG, "P2P dial to peer ${peerFingerprint.take(8)} failed: ${e.message}")

                // mDNS post-failure IP-correlation: on dial failure (most commonly
                // "Connection refused" from a stale port), consult the discovery
                // snapshot for a fresher port from the same IP.  Only update when
                // the discovered address actually differs — avoids a no-op write.
                val freshAddr = resolveAddrByIp(peerAddr, discovered)
                if (freshAddr != null && freshAddr != peerAddr) {
                    Log.i(
                        TAG,
                        "P2P dial ${peerFingerprint.take(8)}: mDNS post-failure refresh " +
                            "$peerAddr → $freshAddr — persisting",
                    )
                    runCatching {
                        settings.upsertPeer(peer.copy(syncAddr = freshAddr))
                    }.onFailure { e2 ->
                        Log.w(TAG, "Failed to persist post-failure addr for ${peerFingerprint.take(8)}: ${e2.message}")
                    }
                }
            } finally {
                // CopyPaste-y4xa: release the PARTIAL_WAKE_LOCK acquired before
                // syncWithPeer. The finally block guarantees release on every exit
                // path: success, exception, and CancellationException (rethrown above
                // before this finally, but the inner try also has its own cancel path).
                if (wakeLock?.isHeld == true) wakeLock.release()
                // CopyPaste-ah3i: zero the unwrapped PAKE session key bytes now that
                // syncWithPeer has consumed them (or we skipped/failed). The bytes were
                // passed into Rust via syncWithPeer; zeroing here shrinks the window
                // during which a heap dump could recover the plaintext session key.
                sessionKey.fill(0)
            }
        }
    }

    /**
     * IP-correlation helper: mirror the Mac's `resolve_addr_from_discovery_by_ip`.
     *
     * Given a [currentAddr] in `"host:port"` form, find the [DiscoveredPeer] in
     * [discovered] whose [DiscoveredPeer.ipAddrs] list contains the same host IP.
     * If found and the discovered port differs from [currentAddr]'s port, return
     * `"<host>:<freshPort>"`; otherwise return null (no actionable update).
     *
     * Self-heals the stale-port failure mode: both peers bind an EPHEMERAL
     * sync-listener port that drifts on every daemon/app restart, so the port
     * persisted at pairing time goes stale.  LAN IP is stable enough to act as
     * the correlation key.  The mDNS `device_id` is a per-device UUID that never
     * equals the cert fingerprint, so direct device_id matching is skipped.
     */
    private fun resolveAddrByIp(
        currentAddr: String,
        discovered: List<DiscoveredPeer>,
    ): String? {
        if (currentAddr.isBlank() || discovered.isEmpty()) return null

        // Parse host from "host:port".  Handle plain IPv4 ("1.2.3.4:port") and
        // bracketed-IPv6 ("[::1]:port") by stripping brackets from the host part.
        val colonIdx = currentAddr.lastIndexOf(':')
        if (colonIdx <= 0) return null
        val host = currentAddr.substring(0, colonIdx).trimStart('[').trimEnd(']')
        if (host.isBlank()) return null

        // Find the first discovered peer that advertises this IP.
        val match = discovered.firstOrNull { dp ->
            dp.ipAddrs.any { it == host }
        } ?: return null

        val freshPort = match.port.toInt()
        if (freshPort <= 0) return null

        // Reconstruct "host:port" — keep the original host string (no bracket changes).
        val hostPart = currentAddr.substring(0, colonIdx)
        val refreshed = "$hostPart:$freshPort"
        return if (refreshed != currentAddr) refreshed else null
    }

    /**
     * Store one [SyncedItem] received over P2P, mapping it to the right local
     * storage path by content type. Shared by BOTH the Android→macOS dialer
     * ([dialPairedPeer]) and the macOS→Android inbound listener poll
     * ([ClipboardService] → [pollP2pListener]).
     *
     * Persists under the peer's STABLE item_id ([SyncedItem.itemId]) as
     * `overrideId`, so a re-dial or a re-receipt from the listener is deduped by
     * [ClipboardRepository] (LWW on item_id) — no extra cross-path dedup needed.
     *
     * Advances the local Lamport clock past every received item (mirrors the
     * Supabase path) so future local pushes order correctly under LWW.
     *
     * Returns true when a new (or replaced) row was stored, false on a dedup /
     * empty / blank no-op.
     */

    suspend fun storeSyncedItem(item: uniffi.copypaste_android.SyncedItem): Boolean =
        withContext(Dispatchers.IO) {
            // Advance the local Lamport clock to stay causally after every received
            // item — without this the local clock lags behind the peer's, making
            // future local pushes appear "older" and breaking LWW ordering.
            settings.lamportClock.observe(item.wallTimeMs)

            // ABI 15: tombstone frame — apply via LWW so a newer remote delete wins
            // and a stale re-sync cannot resurrect a live item.
            if (item.deleted) {
                val tombstoned = repository.applyInboundTombstoneWithLww(
                    itemId = item.itemId,
                    lamportTs = item.wallTimeMs,
                )
                if (tombstoned) {
                    Log.d(TAG, "P2P: applied inbound tombstone for itemId=${item.itemId.take(8)}…")
                }
                return@withContext tombstoned
            }

            val key = settings.encryptionKey

            // UniFFI maps `sequence<u8>` to List<UByte>; storeImageBytes and the
            // UTF-8 text decode below both want a ByteArray.
            val plaintextBytes = ByteArray(item.plaintext.size) { item.plaintext[it].toByte() }

            val stored = when {
                contentTypeIsImage(item.contentType) -> {
                    // Image frame: store a placeholder row under the peer's STABLE
                    // item_id, then persist the raw image bytes so HistoryActivity
                    // can render them. Re-dials dedup via overrideId.
                    if (plaintextBytes.isEmpty()) {
                        false
                    } else {
                        val storedId = repository.storeItem(
                            plaintext = "[image]",
                            key = key,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, plaintextBytes)
                            // CopyPaste-44rq.36: fire-and-forget thumbnail generation on
                            // Dispatchers.Default so the P2P sync result is returned
                            // immediately and the next item can be processed without
                            // waiting 50–200 ms for the decode/compress step.
                            val capturedId = storedId
                            val capturedBytes = plaintextBytes
                            // Use the FGS-bound scope so this task is cancelled when the
                            // service is destroyed; fall back to an ad-hoc scope only in
                            // unit tests where start() was never called.
                            (fgsScope ?: CoroutineScope(Dispatchers.Default)).launch(Dispatchers.Default) {
                                SyncThumbnailHelper.generateAndStore(capturedBytes) { thumbBytes ->
                                    repository.storeThumbnailBytes(capturedId, thumbBytes)
                                }
                            }
                            true
                        } else {
                            false
                        }
                    }
                }
                contentTypeIsFile(item.contentType) -> {
                    // File frame: store actual bytes so the user can save/copy them.
                    // file_name/mime are carried in-band so the label shows the real
                    // name ("[file: report.pdf]") instead of "[file]".
                    if (plaintextBytes.isEmpty()) {
                        false
                    } else {
                        val label = SyncFileHelper.buildFileLabel(item.fileName)
                        val storedId = repository.storeItem(
                            plaintext = label,
                            key = key,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeFileBytes(storedId, plaintextBytes)
                            repository.storeFileMeta(storedId, item.fileName, item.mime)
                            true
                        } else {
                            false
                        }
                    }
                }
                else -> {
                    // Text frame: LWW-replace under the peer's STABLE item_id so an
                    // EDITED clip replaces the prior local row instead of being
                    // deduped/dropped (AB-17 — parity with the cloud/relay paths,
                    // which already use storeItemWithLww). SyncedItem carries no
                    // lamport field over the frozen P2P ABI, so wall_time_ms is the
                    // causal basis — the same value already observed into the local
                    // Lamport clock above (line ~535) and the same basis the macOS
                    // daemon's P2P LWW uses.
                    val plaintext = String(plaintextBytes, Charsets.UTF_8)
                    val didStore = repository.storeItemWithLww(
                        plaintext = plaintext,
                        key = key,
                        itemId = item.itemId,
                        incomingLamportTs = item.wallTimeMs,
                    )
                    // Auto-apply is intentionally NOT done per-frame here. The BATCH
                    // callers (dialPairedPeer / the Supabase drain) apply only the
                    // NEWEST stored text clip once via onSyncedTextClip — applying
                    // per-frame would spam the system clipboard and re-trigger the
                    // capture loop during a multi-item catch-up.
                    didStore
                }
            }

            // lcmq: apply authoritative pin state (pin/unpin/reorder) from the P2P item.
            // Uses applyAuthoritativePinState — not setPinned — so authoritative unpins and
            // pin_order convergence work without minting a new local mutation.
            if (stored) {
                repository.applyAuthoritativePinState(item.itemId, item.pinned, item.pinOrder)
            }

            stored
        }
}

/**
 * CopyPaste-agde: returns true when the device has an active Wi-Fi (or Ethernet)
 * network connection, false when on cellular or when connectivity is unavailable.
 *
 * Used by [FgsSyncLoop.start] to enforce the [Settings.syncOnWifiOnly] preference:
 * Supabase poll and P2P dials are skipped while on a metered (cellular) connection.
 *
 * Null [context] → returns true (no skip) so unit tests and stub mode are unaffected.
 *
 * Implementation uses [android.net.ConnectivityManager.getNetworkCapabilities] on
 * API 23+ (required by our minSdk), which is the only reliable way to query transport
 * type. The legacy [android.net.ConnectivityManager.activeNetworkInfo] path is
 * deprecated since API 29 and omitted.
 */
internal fun isOnWifi(context: android.content.Context?): Boolean {
    context ?: return true // no context → don't block (unit-test safe)
    val cm = context.getSystemService(android.content.Context.CONNECTIVITY_SERVICE)
        as? android.net.ConnectivityManager ?: return false
    val network = cm.activeNetwork ?: return false
    val caps = cm.getNetworkCapabilities(network) ?: return false
    // TRANSPORT_WIFI covers both station and tethered Wi-Fi.
    // TRANSPORT_ETHERNET covers wired Ethernet (also unmetered — honour it).
    return caps.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI) ||
        caps.hasTransport(android.net.NetworkCapabilities.TRANSPORT_ETHERNET)
}
