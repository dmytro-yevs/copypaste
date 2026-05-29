package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for [P2pDialerGate]. No Android/FFI dependencies, so these run
 * under `./gradlew testDebugUnitTest` without an emulator.
 */
class P2pDialerGateTest {

    private val key = ByteArray(32) { 1 }

    @Test
    fun dialsWhenAllCredentialsPresent() {
        assertTrue(
            P2pDialerGate.shouldDial(
                peerSyncAddr = "192.168.1.20:7777",
                peerFingerprint = "ab:cd:ef",
                sessionKey = key,
            )
        )
    }

    @Test
    fun doesNotDialWhenAddrBlank() {
        assertFalse(P2pDialerGate.shouldDial("", "ab:cd", key))
        assertFalse(P2pDialerGate.shouldDial("   ", "ab:cd", key))
    }

    @Test
    fun doesNotDialWhenFingerprintBlank() {
        assertFalse(P2pDialerGate.shouldDial("192.168.1.20:7777", "", key))
        assertFalse(P2pDialerGate.shouldDial("192.168.1.20:7777", "  ", key))
    }

    @Test
    fun doesNotDialWhenSessionKeyEmpty() {
        assertFalse(
            P2pDialerGate.shouldDial("192.168.1.20:7777", "ab:cd", ByteArray(0))
        )
    }

    @Test
    fun normalIntervalAfterSuccessfulDial() {
        assertEquals(
            60_000L,
            P2pDialerGate.nextDelayMs(
                attemptedAndSucceeded = true,
                normalIntervalMs = 60_000L,
                errorBackoffMs = 30_000L,
            )
        )
    }

    @Test
    fun backoffAfterFailureOrClosedGate() {
        assertEquals(
            30_000L,
            P2pDialerGate.nextDelayMs(
                attemptedAndSucceeded = false,
                normalIntervalMs = 60_000L,
                errorBackoffMs = 30_000L,
            )
        )
    }
}
