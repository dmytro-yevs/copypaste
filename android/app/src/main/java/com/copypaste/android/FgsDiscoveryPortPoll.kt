package com.copypaste.android

/**
 * CopyPaste-j2vf: pure port-poll decision helpers for
 * [ClipboardService.startFgsDiscovery] (extracted CopyPaste-vp63.32).
 *
 * `startFgsDiscovery()` waits for [ClipboardService.activeListenerPort] to
 * become non-zero (up to [PORT_POLL_TIMEOUT_MS]) before calling
 * `startDiscovery()`. Advertising `syncPort=0` causes macOS peers to see
 * "device unavailable". These two functions capture the two pure decisions
 * in that loop so they can be verified without instantiating the Android
 * service. [ClipboardService]'s companion keeps forwarding stubs (same
 * names/signatures/values) so [FgsDiscoveryPortPollTest] and any other
 * caller of `ClipboardService.portPollNextBackoffMs` / `.shouldAdvertisePort`
 * / the `PORT_POLL_*` constants are unaffected.
 *
 * NOTE: `startFgsDiscovery()`'s actual poll loop uses an inline
 * `(backoffMs * 2).coerceAtMost(500L)` expression (not a call into this
 * object) — these functions exist to pin/document that behaviour for JVM
 * unit tests, mirroring the pre-existing pattern in this codebase.
 */
object FgsDiscoveryPortPoll {

    /** Maximum total wait for the inbound listener to bind (safety timeout). */
    const val PORT_POLL_TIMEOUT_MS = 10_000L

    /** Initial backoff (ms) between port-poll retries. */
    const val PORT_POLL_INITIAL_BACKOFF_MS = 20L

    /** Maximum backoff (ms) between port-poll retries. */
    const val PORT_POLL_MAX_BACKOFF_MS = 500L

    /**
     * Compute the next exponential backoff delay (capped at [maxMs]).
     *
     * Mirrors the inline expression in `startFgsDiscovery`:
     * `backoffMs = (backoffMs * 2).coerceAtMost(500L)`
     *
     * Pure: no side-effects, no system-clock reads.
     *
     * @param currentMs current backoff duration.
     * @param maxMs     cap for the next backoff; must be > 0.
     * @return          next backoff, doubling from [currentMs], capped at [maxMs].
     */
    fun portPollNextBackoffMs(currentMs: Long, maxMs: Long): Long =
        (currentMs * 2L).coerceAtMost(maxMs)

    /**
     * True when [port] is non-zero and it is safe to advertise over mDNS.
     *
     * A syncPort=0 advertisement is WORSE than no advertisement: the macOS
     * peer dials :0 and immediately fails with "device unavailable" (j2vf).
     * This guard is the single place where that decision is expressed.
     *
     * Pure: no side-effects.
     *
     * @param port the value of [ClipboardService.activeListenerPort] after the poll loop.
     * @return true iff the port is ready to be published over mDNS.
     */
    fun shouldAdvertisePort(port: Int): Boolean = port > 0
}
