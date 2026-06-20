package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pure-JVM unit tests for CopyPaste-mgkr (NG-3): explicit Verified / Unverified
 * trust label on paired device cards (Android platform).
 *
 * All peers persisted to the PairedPeer roster have completed SAS confirmation
 * before being added — so the trust label is always "Verified" for a persisted
 * peer. The [trustLabel] helper encodes this contract so the composable PeerRow
 * can call it without hard-coding the string.
 *
 * These tests run on the JVM with no Compose runtime or Android SDK.
 */
class DevicesTrustLabelTest {

    private fun peer(name: String = "Alice", fingerprint: String = "aabbccdd") = PairedPeer(
        fingerprint = fingerprint,
        syncAddr = "192.168.1.5:7007",
        name = name,
        sessionKeyWrappedB64 = "key",
        sessionKeyIvB64 = "iv",
    )

    /** A persisted peer is always Verified (SAS was completed to add it). */
    @Test
    fun `trustLabel returns Verified for a standard paired peer`() {
        assertEquals("Verified", trustLabel(peer()))
    }

    /** Even a nameless/legacy peer is Verified once it is in the roster. */
    @Test
    fun `trustLabel returns Verified for a nameless peer`() {
        assertEquals("Verified", trustLabel(peer(name = "")))
    }

    /** The label is exact-case "Verified" matching PARITY-SPEC §NG-3. */
    @Test
    fun `trustLabel text matches the exact case required by PARITY-SPEC NG-3`() {
        val label = trustLabel(peer())
        assertEquals(
            "Trust label must be exactly 'Verified' (PARITY-SPEC §NG-3)",
            "Verified",
            label,
        )
    }
}
