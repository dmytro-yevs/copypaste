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
        // Stopgap: active cadence is 3s for streaks below IDLE_THRESHOLD_POLLS.
        assertEquals(3_000L, FgsSyncLoop.intervalForEmptyStreak(0))
        assertEquals(3_000L, FgsSyncLoop.intervalForEmptyStreak(1))
        // Idle (15s) once the empty streak reaches the threshold.
        assertTrue(FgsSyncLoop.intervalForEmptyStreak(100) >= 15_000L)
    }

    @Test
    fun activeCadenceIsThreeSecondStopgap() {
        // The stopgap active interval must be 3s so a foreground/FGS-active app
        // receives a Supabase clip within ~3s instead of up to a minute.
        assertEquals(3_000L, FgsSyncLoop.intervalForEmptyStreak(0))
    }

    @Test
    fun idleBackoffStaysResponsive() {
        // Idle backoff must not balloon back to the old 5-minute cadence while
        // the FGS is alive — keep it responsive (<= 15s).
        val idle = FgsSyncLoop.intervalForEmptyStreak(IDLE_FAR_STREAK)
        assertTrue("idle interval $idle should be <= 15s", idle <= 15_000L)
    }

    private companion object {
        const val IDLE_FAR_STREAK = 100
    }
}
