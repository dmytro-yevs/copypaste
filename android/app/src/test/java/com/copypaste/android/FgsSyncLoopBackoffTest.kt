package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-logic JVM unit tests for [FgsSyncLoop]'s backoff / interval math (M6).
 *
 * These exercise the companion functions only — they never construct an
 * [FgsSyncLoop], never touch `android.util.Log`, and need no Android runtime,
 * so they run on the plain JVM via `./gradlew test`.
 */
class FgsSyncLoopBackoffTest {

    private val base = 30_000L
    private val max = 480_000L

    @Test
    fun zeroOrNegativeFailures_isZero() {
        assertEquals(0L, FgsSyncLoop.backoffMs(0, base, max))
        assertEquals(0L, FgsSyncLoop.backoffMs(-3, base, max))
    }

    @Test
    fun firstFailure_isBase() {
        assertEquals(base, FgsSyncLoop.backoffMs(1, base, max))
    }

    @Test
    fun backoffDoublesEachFailure() {
        assertEquals(base, FgsSyncLoop.backoffMs(1, base, max))          // 30s
        assertEquals(base * 2, FgsSyncLoop.backoffMs(2, base, max))      // 60s
        assertEquals(base * 4, FgsSyncLoop.backoffMs(3, base, max))      // 120s
        assertEquals(base * 8, FgsSyncLoop.backoffMs(4, base, max))      // 240s
    }

    @Test
    fun backoffClampsToMax() {
        // 30s * 2^4 = 480s == max, then stays clamped.
        assertEquals(max, FgsSyncLoop.backoffMs(5, base, max))
        assertEquals(max, FgsSyncLoop.backoffMs(6, base, max))
        assertEquals(max, FgsSyncLoop.backoffMs(50, base, max))
    }

    @Test
    fun largeFailureCount_doesNotOverflow() {
        // Guards the (1L shl exponent) shift against overflow / negative values.
        val v = FgsSyncLoop.backoffMs(1000, base, max)
        assertTrue("expected clamped, got $v", v == max)
    }

    @Test
    fun intervalUsesNormalUntilThresholdThenIdle() {
        // WS-primary: active catch-up when WS is down is 60s.
        assertEquals(60_000L, FgsSyncLoop.intervalForEmptyStreak(0))
        assertEquals(60_000L, FgsSyncLoop.intervalForEmptyStreak(1))
        // Idle (300s) once the empty streak reaches the threshold.
        assertTrue(FgsSyncLoop.intervalForEmptyStreak(100) >= 300_000L)
    }

    @Test
    fun activeCadenceIsWsDown_60s() {
        // With WS as primary push channel, the catch-up interval while WS is
        // down must be 60s (not the old 3s stopgap).
        assertEquals(60_000L, FgsSyncLoop.intervalForEmptyStreak(0))
    }

    @Test
    fun idleIntervalIs300s() {
        // Idle interval is 300s (5 min) in the WS-primary world. The old
        // <= 15s constraint was a stopgap; now the WS delivers in ~1s.
        val idle = FgsSyncLoop.intervalForEmptyStreak(IDLE_FAR_STREAK)
        assertEquals("idle interval must be 300s", 300_000L, idle)
    }

    private companion object {
        const val IDLE_FAR_STREAK = 100
    }
}
