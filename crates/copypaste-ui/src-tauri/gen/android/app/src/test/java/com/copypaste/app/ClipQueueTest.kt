package com.copypaste.app

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class ClipQueueTest {
    @Before
    fun resetBefore() = reset()

    @After
    fun resetAfter() = reset()

    @Test
    fun privateClipIsNotReplayedWhenDrainRunsAfterModeTurnsOff() {
        ClipQueue.setPrivateMode(true)
        ClipQueue.offer("private", CaptureSource.IN_APP)
        ClipQueue.setPrivateMode(false)

        assertTrue(ClipQueue.drain().first.isEmpty())
    }

    @Test
    fun persistedPrivateModeDropsAnythingQueuedBeforeRustRestarts() {
        ClipQueue.offer("private while Rust was down", CaptureSource.IN_APP)

        ClipQueue.setPrivateMode(true)
        ClipQueue.setPrivateMode(false)

        assertTrue(ClipQueue.drain().first.isEmpty())
    }

    @Test
    fun oneEligibleCaptureIsDrainedExactlyOnce() {
        ClipQueue.offer("tile capture", CaptureSource.TILE)

        val first = ClipQueue.drain().first

        assertEquals(listOf("tile capture"), first.map(CapturedClip::text))
        assertTrue(ClipQueue.drain().first.isEmpty())
    }

    /**
     * `intake::Buffer::discard_all` counts the clips Rust throws away for the
     * same reason, and this queue used to zero its tally instead. That erased
     * drops which happened before private mode and had never been reported, so
     * history had a hole nobody was told about.
     */
    @Test
    fun enteringPrivateModeCountsWhatItDiscardsRatherThanZeroingTheTally() {
        ClipQueue.offer("waiting for the drain", CaptureSource.IN_APP)

        ClipQueue.setPrivateMode(true)
        ClipQueue.setPrivateMode(false)

        assertEquals(1L, ClipQueue.drain().second)
    }

    private fun reset() {
        ClipQueue.setPrivateMode(true)
        ClipQueue.setPrivateMode(false)
        // The tally now survives private mode, so it is the drain that clears
        // it between tests.
        ClipQueue.drain()
    }
}
