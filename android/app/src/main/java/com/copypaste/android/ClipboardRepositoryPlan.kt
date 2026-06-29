package com.copypaste.android

/**
 * Pure planner and LWW-clock helpers for [ClipboardRepository].
 *
 * Extracted from the companion object of [ClipboardRepository] (CopyPaste-ra15.4).
 * All functions are stateless — no SharedPreferences, no Context, no side effects.
 *
 * [ClipboardRepository.planCountCapEvictions] and [ClipboardRepository.nextLamportTs]
 * are kept as companion-object forwarding stubs so existing call sites are unchanged.
 */

/**
 * PURE count-cap planner — single source of truth for continuous enforcement in
 * [ClipboardRepository.pruneToLimitsImpl] and for the "Maximum stored items"
 * Settings confirmation dialog count in [ClipboardRepository.countPrunableByMaxItems].
 *
 * Given the LIVE (non-tombstone) item ids oldest-first, the pinned set, and the
 * configured cap, returns the ids the count-cap pass would evict — the OLDEST UNPINNED
 * items first — to bring the live item count down to [maxItems]. PINNED ids count toward
 * the cap but are NEVER evicted, so the result is always disjoint from [pinned].
 *
 * See [ClipboardRepository.planCountCapEvictions] for full KDoc.
 */
internal fun planCountCapEvictions(
    liveIds: List<String>,
    pinned: Set<String>,
    maxItems: Int,
): List<String> {
    // coerceAtLeast(1): a persisted 0 must never evict the entire store
    // (which would also wipe pinned items by exhausting the unpinned pool).
    val cap = maxItems.coerceAtLeast(1)
    val unpinned = liveIds.filter { it !in pinned }.toMutableList()
    var liveCount = liveIds.size
    val evicted = ArrayList<String>()
    while (unpinned.isNotEmpty() && liveCount > cap) {
        evicted.add(unpinned.removeAt(0)) // oldest unpinned first
        liveCount--
    }
    return evicted
}

/**
 * Compute the next Lamport timestamp, mirroring `next_lamport_ts` in
 * copypaste-core/src/storage/items.rs:
 *   `max(prev + 1, now_ms)`
 *
 * This guarantees two properties:
 *  - **monotonic**: always strictly greater than [prevLamport].
 *  - **time-ordered**: at least [nowMs], so the newest writer across devices wins.
 *
 * See [ClipboardRepository.nextLamportTs] for full KDoc.
 */
internal fun nextLamportTs(prevLamport: Long, nowMs: Long): Long =
    maxOf(prevLamport + 1L, nowMs)
