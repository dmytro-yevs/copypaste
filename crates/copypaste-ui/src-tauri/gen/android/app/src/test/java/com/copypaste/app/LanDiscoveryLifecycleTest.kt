package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LanDiscoveryLifecycleTest {
    @Test
    fun waitsForThePlatformCallbackBeforeReportingBrowseReady() {
        val lifecycle = BrowseLifecycle()

        assertEquals(BrowseStart.START, lifecycle.begin())
        assertEquals(BrowseStart.WAITING, lifecycle.begin())
        assertTrue(lifecycle.finish(available = true))
        assertEquals(BrowseStart.READY, lifecycle.begin())
    }

    @Test
    fun startFailureReturnsToRetryableStoppedState() {
        val lifecycle = BrowseLifecycle()

        assertEquals(BrowseStart.START, lifecycle.begin())
        assertTrue(lifecycle.finish(available = false))
        assertEquals(BrowseStart.START, lifecycle.begin())
        assertTrue(lifecycle.stop())
        assertFalse(lifecycle.stop())
    }
}
