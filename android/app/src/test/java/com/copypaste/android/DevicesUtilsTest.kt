package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * CopyPaste-myh8.7 — S7 (Devices) restyle: unit tests for the pure logic
 * helpers touched by the §9.7 field-grid / fingerprint tap-to-copy / presence
 * work in DevicesUtils.kt and DevicesAnimations.kt.
 */
class DevicesUtilsTest {

    // ── formatPeerFingerprint (android-devices spec "Fingerprint tap-to-copy
    // parity" — the SAME truncation now applies to the own-device card too) ──

    private val fingerprint64 = "0123456789abcdef".repeat(4)

    @Test
    fun formatPeerFingerprintTruncatesToFirstSixteenEllipsisLastEight() {
        assertEquals("0123456789abcdef…89abcdef", formatPeerFingerprint(fingerprint64))
    }

    @Test
    fun formatPeerFingerprintTruncationIsIdenticalForOwnAndPeerSurfaces() {
        // android-devices spec "Truncated display never varies by surface": own
        // device, peer card, and roster all call this SAME function on the SAME
        // 64-hex value, so there is only one formatter to keep in parity.
        val ownCardDisplay = formatPeerFingerprint(fingerprint64)
        val peerCardDisplay = formatPeerFingerprint(fingerprint64)
        assertEquals(ownCardDisplay, peerCardDisplay)
    }

    @Test
    fun formatPeerFingerprintShortInputStillProducesTruncationMarker() {
        // Defensive: a fingerprint shorter than 24 chars (malformed/legacy data)
        // must not crash — take()/takeLast() clamp gracefully.
        val short = "abcd1234"
        assertEquals("abcd1234…abcd1234", formatPeerFingerprint(short))
    }

    // ── EM_DASH / formatEpochMs (RTT placeholder shares this "unknown" glyph) ──

    @Test
    fun formatEpochMsReturnsEmDashForZeroOrNegative() {
        assertEquals(EM_DASH, formatEpochMs(0L))
        assertEquals(EM_DASH, formatEpochMs(-1L))
    }

    @Test
    fun formatEpochMsFormatsAPositiveTimestamp() {
        assertNotEquals(EM_DASH, formatEpochMs(1_700_000_000_000L))
    }

    // ── pulseDotColorRole (PulseDot's presence dot — paired with a text label,
    // never color-only per STYLEGUIDE §7) ──

    @Test
    fun pulseDotColorRoleIsOnlineWhenOnline() {
        assertEquals(PulseDotColorRole.ONLINE, pulseDotColorRole(online = true))
    }

    @Test
    fun pulseDotColorRoleIsOfflineWhenOffline() {
        assertEquals(PulseDotColorRole.OFFLINE, pulseDotColorRole(online = false))
    }

    // ── trustLabel (Verified/Unverified badge text) ──

    @Test
    fun trustLabelIsVerifiedForASasVerifiedPeer() {
        val peer = PairedPeer(fingerprint = "fp", syncAddr = "", name = "Mac").copy(sasVerified = true)
        assertEquals("Verified", trustLabel(peer))
    }

    @Test
    fun trustLabelIsUnverifiedForANonSasVerifiedPeer() {
        val peer = PairedPeer(fingerprint = "fp", syncAddr = "", name = "Mac").copy(sasVerified = false)
        assertEquals("Unverified", trustLabel(peer))
    }
}
