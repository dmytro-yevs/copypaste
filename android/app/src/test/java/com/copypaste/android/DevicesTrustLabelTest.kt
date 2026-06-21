package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * Pure-JVM unit tests for [trustLabel] — CopyPaste-1jms.4 / CopyPaste-mgkr (NG-3).
 *
 * trustLabel() reflects [PairedPeer.sasVerified]:
 *  - sasVerified=true (default for all SAS-paired peers) → "Verified"
 *  - sasVerified=false (cloud-imported or non-SAS peer) → "Unverified"
 *
 * These tests run on the JVM with no Compose runtime or Android SDK.
 */
class DevicesTrustLabelTest {

    private fun peer(
        name: String = "Alice",
        fingerprint: String = "aabbccdd",
        sasVerified: Boolean = true,
    ) = PairedPeer(
        fingerprint = fingerprint,
        syncAddr = "192.168.1.5:7007",
        name = name,
        sessionKeyWrappedB64 = "key",
        sessionKeyIvB64 = "iv",
        sasVerified = sasVerified,
    )

    /** A peer admitted via SAS (the normal P2P pairing flow) is "Verified". */
    @Test
    fun `trustLabel returns Verified for a SAS-confirmed peer`() {
        assertEquals("Verified", trustLabel(peer(sasVerified = true)))
    }

    /** A peer admitted by a non-SAS mechanism (cloud-import, future admin flow) is "Unverified". */
    @Test
    fun `trustLabel returns Unverified for a non-SAS peer`() {
        assertEquals("Unverified", trustLabel(peer(sasVerified = false)))
    }

    /** The labels for verified and unverified peers must be distinct. */
    @Test
    fun `trustLabel verified and unverified are different strings`() {
        assertNotEquals(
            "Verified and Unverified labels must differ",
            trustLabel(peer(sasVerified = true)),
            trustLabel(peer(sasVerified = false)),
        )
    }

    /** Even a nameless/legacy peer is Verified once it is in the roster (sasVerified defaults true). */
    @Test
    fun `trustLabel returns Verified for a nameless SAS peer`() {
        assertEquals("Verified", trustLabel(peer(name = "", sasVerified = true)))
    }

    /** The label is exact-case "Verified" matching PARITY-SPEC §NG-3. */
    @Test
    fun `trustLabel text matches the exact case required by PARITY-SPEC NG-3`() {
        val label = trustLabel(peer(sasVerified = true))
        assertEquals(
            "Trust label must be exactly 'Verified' (PARITY-SPEC §NG-3)",
            "Verified",
            label,
        )
    }
}
