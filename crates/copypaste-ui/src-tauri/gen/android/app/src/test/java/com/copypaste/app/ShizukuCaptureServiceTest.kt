package com.copypaste.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class ShizukuCaptureServiceTest {
    @Test
    fun forwardsOnlyClipboardServiceEventsForThisPackage() {
        assertTrue(
            isClipboardAccessEvent(
                "09-27 12:00:00.000 E ClipboardService: read com.copypaste.app",
                "com.copypaste.app",
            ),
        )
        assertFalse(isClipboardAccessEvent("ClipboardService: read com.other.app", "com.copypaste.app"))
        assertFalse(isClipboardAccessEvent("ActivityTaskManager: com.copypaste.app", "com.copypaste.app"))
    }

    @Test
    fun repeatedSignalsAreSuppressedUntilOneSecondHasElapsed() {
        var now = 10_000L
        val signals = ClipboardAccessSignals { now }

        assertTrue(signals.shouldForward())
        assertFalse(signals.shouldForward())
        now += 999L
        assertFalse(signals.shouldForward())
        now += 1L
        assertTrue(signals.shouldForward())
    }

    @Test
    fun lateCleanupFromStoppedReaderCannotClearItsReplacement() {
        val runs = ShizukuCaptureRuns()
        val first = runs.begin(listener())!!
        assertTrue(runs.attachReader(first, Thread()))
        assertSame(first, runs.stop())

        val replacement = runs.begin(listener())!!
        assertTrue(runs.attachReader(replacement, Thread()))

        assertNull(runs.finish(first))
        assertSame(replacement, runs.activeForTest())
        assertFalse(runs.attachReader(first, Thread()))
    }

    @Test
    fun unexpectedReaderExitReportsLossOnlyForTheActiveRun() {
        val runs = ShizukuCaptureRuns()
        val run = runs.begin(listener())!!

        assertSame(run.listener, runs.finish(run))
        assertNull(runs.activeForTest())
    }

    @Test
    fun explicitStopMakesReaderExitSilent() {
        val runs = ShizukuCaptureRuns()
        val run = runs.begin(listener())!!

        assertSame(run, runs.stop())
        assertNull(runs.finish(run))
    }

    private fun listener() = object : IClipCascadeCaptureListener.Stub() {
        override fun onClipboardAccess() = Unit
        override fun onCaptureStopped() = Unit
    }
}
