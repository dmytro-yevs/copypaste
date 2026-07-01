package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.32 — direct [FgsDiscoveryPortPoll] tests (the extracted pure
 * object), complementing [FgsDiscoveryPortPollTest] which exercises the same
 * behaviour via [ClipboardService]'s forwarding stubs.
 */
class FgsDiscoveryPortPollUnitTest {

    @Test
    fun `shouldAdvertisePort false for zero, true for positive`() {
        assertFalse(FgsDiscoveryPortPoll.shouldAdvertisePort(0))
        assertTrue(FgsDiscoveryPortPoll.shouldAdvertisePort(1))
        assertTrue(FgsDiscoveryPortPoll.shouldAdvertisePort(52_000))
    }

    @Test
    fun `shouldAdvertisePort false for negative port`() {
        assertFalse(FgsDiscoveryPortPoll.shouldAdvertisePort(-1))
    }

    @Test
    fun `portPollNextBackoffMs doubles and clamps to max`() {
        assertEquals(40L, FgsDiscoveryPortPoll.portPollNextBackoffMs(20L, 500L))
        assertEquals(500L, FgsDiscoveryPortPoll.portPollNextBackoffMs(10_000L, 500L))
    }

    @Test
    fun `constants match ClipboardService forwarding stubs`() {
        assertEquals(ClipboardService.PORT_POLL_TIMEOUT_MS, FgsDiscoveryPortPoll.PORT_POLL_TIMEOUT_MS)
        assertEquals(ClipboardService.PORT_POLL_INITIAL_BACKOFF_MS, FgsDiscoveryPortPoll.PORT_POLL_INITIAL_BACKOFF_MS)
        assertEquals(ClipboardService.PORT_POLL_MAX_BACKOFF_MS, FgsDiscoveryPortPoll.PORT_POLL_MAX_BACKOFF_MS)
    }
}
