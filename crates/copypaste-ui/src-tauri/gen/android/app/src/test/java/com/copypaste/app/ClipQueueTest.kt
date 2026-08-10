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

    private fun reset() {
        ClipQueue.setPrivateMode(true)
        ClipQueue.setPrivateMode(false)
    }
}
