package com.copypaste.android

/**
 * Pure scheduling/backoff/filtering policy for [FgsSyncLoop] (CopyPaste-vp63.35).
 *
 * Extracted from [FgsSyncLoop]'s companion object — every function here is
 * stateless and Android-runtime-free (no Context, no coroutines), so it is
 * directly unit-testable on the plain JVM. [FgsSyncLoop]'s companion keeps
 * forwarding stubs (same names/signatures) so existing call sites —
 * internal unqualified calls inside [FgsSyncLoop] and external qualified
 * calls such as [SupabasePollWorker] / [ClipboardService] — are unaffected.
 */
object SyncLoopPolicy {
    /**
     * Catch-up poll interval while the Supabase Realtime WS is **connected**.
     * WS is the primary receive path; polling is only a safety net here.
     */
    private const val POLL_INTERVAL_WS_CONNECTED_MS = 120_000L // 2 min

    /**
     * Catch-up poll interval while the WS is **disconnected** (or not yet
     * joined). More frequent so incoming clips are not delayed while the WS
     * reconnects.
     */
    private const val POLL_INTERVAL_WS_DOWN_MS = 60_000L // 1 min

    /**
     * Idle catch-up interval after [IDLE_THRESHOLD_POLLS] consecutive empty
     * polls. Applied regardless of WS state — battery courtesy when nothing
     * is changing.
     */
    private const val IDLE_POLL_INTERVAL_MS = 300_000L // 5 min

    /** First retry delay after a transient network failure; doubled per
     *  consecutive failure up to [RETRY_BACKOFF_MAX_MS]. */
    private const val RETRY_BACKOFF_BASE_MS = 30_000L

    /** Upper bound on the exponential retry backoff. */
    private const val RETRY_BACKOFF_MAX_MS = 480_000L // 8 min

    /** How many consecutive empty polls before we slow down to the idle interval. */
    private const val IDLE_THRESHOLD_POLLS = 3

    /**
     * Minimum P2P dial cadence — the base interval used while activity is
     * occurring.  Also the value exposed to external callers (e.g.
     * [ClipboardService] inbound-listener drain cadence) — MUST NOT change.
     *
     * 30 s is short enough to deliver new clips promptly while avoiding the
     * "re-transmit entire history every 3 s" behaviour that the old 3 s value
     * produced.  The outbound high-water cursor (see [Settings.p2pOutboundHighWater])
     * further caps what is sent on each tick, so even at 30 s only NEW items
     * travel over the wire after the first dial.
     *
     * Also drives the inbound listener drain cadence in [ClipboardService].
     */
    const val P2P_DIAL_INTERVAL_MS = 30_000L

    /**
     * CopyPaste-44rq.41: maximum P2P dial interval after an idle streak.
     * After [P2P_IDLE_THRESHOLD] consecutive dials with zero items exchanged,
     * the inter-dial sleep grows linearly up to this cap.  A successful
     * exchange (any items sent or received) resets the interval to
     * [P2P_DIAL_INTERVAL_MS].  5 min matches [IDLE_POLL_INTERVAL_MS].
     */
    private const val P2P_IDLE_DIAL_INTERVAL_MS = 300_000L // 5 min

    /**
     * CopyPaste-44rq.41: how many consecutive empty P2P dials before the
     * inter-dial sleep starts to grow.  Mirrors [IDLE_THRESHOLD_POLLS] so
     * the two backoff mechanisms behave symmetrically.
     */
    private const val P2P_IDLE_THRESHOLD = 3

    /**
     * CopyPaste-mip2: debounce window for opportunistic P2P wake signals.
     *
     * When the wake channel receives a signal (clipboard capture or mDNS peer
     * discovery), the inner P2P sleep exits early and re-dials.  A CONFLATED
     * channel already collapses multiple concurrent signals into one, so this
     * constant is informational only — it documents the intended semantics.
     * The channel's capacity=CONFLATED property is the actual debounce.
     */
    const val P2P_WAKE_DEBOUNCE_MS = 500L

    /**
     * CopyPaste-44rq.41: compute the effective P2P inter-dial sleep given
     * the number of consecutive idle dials.
     *
     * Grows linearly from [P2P_DIAL_INTERVAL_MS] to [P2P_IDLE_DIAL_INTERVAL_MS]
     * over [P2P_IDLE_THRESHOLD] empty dials, then stays capped.  Linear growth
     * (not exponential) is intentional: P2P peers may come online at any time
     * and a gentler ramp avoids a multi-minute gap on the first post-idle dial.
     *
     * Pure function — no Android runtime — safe to call in JVM unit tests.
     *
     * @param consecutiveEmpty Number of consecutive P2P dials with zero items
     *   exchanged (including the most recent one).
     */
    fun p2pDialIntervalMs(consecutiveEmpty: Int): Long {
        if (consecutiveEmpty < P2P_IDLE_THRESHOLD) return P2P_DIAL_INTERVAL_MS
        return P2P_IDLE_DIAL_INTERVAL_MS
    }

    /**
     * WS-aware steady-state catch-up poll interval.
     *
     * - WS connected + active streak   → [POLL_INTERVAL_WS_CONNECTED_MS] (120 s)
     * - WS disconnected + active streak → [POLL_INTERVAL_WS_DOWN_MS] (60 s)
     * - Either state + idle streak      → [IDLE_POLL_INTERVAL_MS] (300 s)
     *
     * Pure for unit testing.
     */
    fun pollIntervalMs(wsConnected: Boolean, consecutiveEmpty: Int): Long {
        if (consecutiveEmpty >= IDLE_THRESHOLD_POLLS) return IDLE_POLL_INTERVAL_MS
        return if (wsConnected) POLL_INTERVAL_WS_CONNECTED_MS else POLL_INTERVAL_WS_DOWN_MS
    }

    /**
     * M6: pure exponential-backoff computation, extracted so it can be unit
     * tested on the JVM without Android. [failures] is the number of
     * consecutive failures *including* the one that just occurred (>= 1).
     *
     * Returns base * 2^(failures-1), clamped to [RETRY_BACKOFF_MAX_MS].
     * Guards against shift overflow for large failure counts.
     */
    fun backoffMs(
        failures: Int,
        base: Long = RETRY_BACKOFF_BASE_MS,
        max: Long = RETRY_BACKOFF_MAX_MS,
    ): Long {
        if (failures <= 0) return 0L
        // Cap the exponent so the shift cannot overflow Long; once the
        // unclamped value would exceed `max` the result is `max` anyway.
        val exponent = (failures - 1).coerceAtMost(40)
        val scaled = base.toDouble() * (1L shl exponent).toDouble()
        return if (scaled >= max.toDouble()) max else scaled.toLong()
    }

    /**
     * Legacy shim used by existing [FgsSyncLoopBackoffTest].
     * Returns [pollIntervalMs] with wsConnected=false for backward compat.
     */
    fun intervalForEmptyStreak(consecutiveEmpty: Int): Long =
        pollIntervalMs(wsConnected = false, consecutiveEmpty = consecutiveEmpty)

    /**
     * CopyPaste-1t38: should the periodic loop attempt a relay/cloud mutation
     * drain on this tick?
     *
     * True only when there is at least one pending record AND the backoff window
     * from the previous failed/partial drain has elapsed. This keeps the periodic
     * drain bounded (no work when the queue is empty) and prevents hammering the
     * network while offline (a failed drain sets [backoffUntilMs] via [backoffMs]).
     *
     * Pure — no Android runtime — so the offline→online transition is unit-testable
     * with a fake clock. The drain itself is single-flight inside
     * [SyncManager.drainOutboundMutationQueue].
     */
    fun shouldAttemptDrain(queueSize: Int, nowMs: Long, backoffUntilMs: Long): Boolean =
        queueSize > 0 && nowMs >= backoffUntilMs

    /**
     * Filter [allLocalItems] to only those items whose [wallTimeMs] is
     * STRICTLY GREATER than [outboundHighWater].
     *
     * When [outboundHighWater] is 0 (never synced), returns all items
     * unchanged — the first dial always sends the full history.
     *
     * Pure function — no Android runtime, no coroutines — intentionally kept
     * so it can be unit-tested on the plain JVM.
     */
    fun filterByOutboundHighWater(
        allLocalItems: List<Pair<String, Long>>,
        outboundHighWater: Long,
    ): List<Pair<String, Long>> {
        if (outboundHighWater == 0L) return allLocalItems
        return allLocalItems.filter { (_, wallTimeMs) -> wallTimeMs > outboundHighWater }
    }

    /**
     * Compute the max wallTimeMs from a list of (id, wallTimeMs) pairs.
     * Returns 0L for an empty list (cursor stays unchanged — no items sent).
     *
     * Pure function for JVM-testability.
     */
    fun maxWallTime(items: List<Pair<String, Long>>): Long =
        if (items.isEmpty()) 0L else items.maxOf { it.second }

    /**
     * CopyPaste-yaip (P2P gap): select mutation-queue records for P2P outbound,
     * BYPASSING the wall-time high-water filter.
     *
     * [filterByOutboundHighWater] uses `wallTimeMs > highWater` to select items.
     * A pin/reorder/delete mutation only bumps `lamport_ts` — `wallTimeMs` is
     * unchanged, so those mutations are silently dropped by the wall-time filter
     * after the initial sync when `highWater == wallTimeMs`. The queue-based path
     * must send them regardless of wall-time.
     *
     * This function returns ALL [pending] mutations unconditionally (no wall-time
     * filter). The caller merges the result with the regular localItems list before
     * calling syncWithPeer. Both lists carry distinct item semantics (LocalItem vs.
     * MutationRecord), so the merge step is handled in [mergeQueuedItemIdsWithLocal].
     *
     * Pure function — no Android runtime — intentionally kept so it can be
     * unit-tested on the plain JVM.
     *
     * @param pending         Pending mutation records from [OutboundMutationQueue].
     * @param outboundHighWater Current P2P outbound high-water cursor (ignored for
     *                          filtering — documented here for call-site clarity).
     * @return All [pending] records, unchanged.
     */
    fun filterQueuedMutationsForP2P(
        pending: List<OutboundMutationQueue.MutationRecord>,
        @Suppress("UNUSED_PARAMETER") outboundHighWater: Long,
    ): List<OutboundMutationQueue.MutationRecord> = pending

    /**
     * CopyPaste-yaip (P2P dedup): merge the item IDs of locally-stored items
     * ([localItemIds]) with the item IDs of queued mutations ([queuedMutations]),
     * deduplicating on item ID.
     *
     * An item may appear in BOTH lists when it exists locally AND has a pending
     * pin/delete mutation. Returning the UNION ensures:
     *   - Regular items: selected by the wall-time filter (existing behaviour).
     *   - Queued-only mutations: items whose wallTimeMs == highWater or older
     *     but with a new lamport bump (pin/reorder) are included via the queue.
     *   - No double-send: a single item ID appears at most once in the merged set.
     *
     * The caller uses the merged set to drive syncWithPeer, which dedups by
     * item_id on the wire so even if both sources somehow produced the same
     * item, the peer applies LWW and ignores the lower-lamport copy.
     *
     * Pure function — no Android runtime — intentionally kept so it can be
     * unit-tested on the plain JVM.
     */
    fun mergeQueuedItemIdsWithLocal(
        localItemIds: Set<String>,
        queuedMutations: List<OutboundMutationQueue.MutationRecord>,
    ): Set<String> = localItemIds + queuedMutations.map { it.itemId }.toSet()

    /**
     * Select the newest text clip from a list of (text, wallTime) pairs
     * accumulated across a bulk-sync batch drain.
     *
     * "Newest" = highest wall_time. When two items share the same wall_time,
     * the one that arrived LAST in batch order wins (latest row processed).
     *
     * Pure function — no Android runtime, no coroutines — intentionally kept
     * so it can be unit-tested on the plain JVM.
     *
     * @return the text of the newest clip, or null when [clips] is empty.
     */
    fun newestTextClip(clips: List<Pair<String, Long>>): String? {
        if (clips.isEmpty()) return null
        var bestText = clips[0].first
        var bestWallTime = clips[0].second
        for (i in 1 until clips.size) {
            val (text, wallTime) = clips[i]
            // >= so that a later item at the SAME wall_time replaces the
            // current winner (last-in-order wins on ties).
            if (wallTime >= bestWallTime) {
                bestText = text
                bestWallTime = wallTime
            }
        }
        return bestText
    }
}
