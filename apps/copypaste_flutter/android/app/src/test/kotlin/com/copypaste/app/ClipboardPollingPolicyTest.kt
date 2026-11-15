package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ClipboardPollingPolicyTest {
    @Test
    fun darkOrLockedScreenAndVisibleAppPause() {
        val policy = ClipboardPollingPolicy(lastSeenTimestamp = 0L)
        assertEquals(PollingStep.PAUSE, policy.step(interactive = false, locked = false, appVisible = false))
        assertEquals(PollingStep.PAUSE, policy.step(interactive = true, locked = true, appVisible = false))
        assertEquals(PollingStep.PAUSE, policy.step(interactive = true, locked = false, appVisible = true))
        assertEquals(PollingStep.READ, policy.step(interactive = true, locked = false, appVisible = false))
    }

    @Test
    fun onlyANewTimestampCountsAsAChange() {
        val policy = ClipboardPollingPolicy(lastSeenTimestamp = 100L)
        assertFalse(policy.changed(null))
        assertFalse(policy.changed(100L))
        assertTrue(policy.changed(200L))
        assertFalse(policy.changed(200L))
    }

    @Test(expected = IllegalArgumentException::class)
    fun intervalIsBounded() {
        ClipboardPollingPolicy(lastSeenTimestamp = 0L, intervalMs = 100L)
    }
}
