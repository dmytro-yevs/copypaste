package com.copypaste.android

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
}
