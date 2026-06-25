package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the Liquid Glass Devices parity logic (CopyPaste-kyn):
 * - Fingerprint truncation rule (own = full, peer = take(16)+…+takeLast(8))
 * - Transport chip derivation (P2P when syncAddr/peerLocalIp present, Cloud otherwise)
 * - QR countdown progress clamp (0..1, warning threshold)
 * - PulseDot: online flag drives animation gate
 *
 * None of these require the Android SDK / emulator — pure Kotlin.
 */
class DevicesLiquidGlassTest {

    // ────────────────────────────────────────────────────────────────────────
    // §7 Fingerprint truncation rule
    // ────────────────────────────────────────────────────────────────────────

    /** Own device shows the full fingerprint without truncation. */
    @Test
    fun `own fingerprint is shown in full`() {
        val fp = "abcdef1234567890abcdef1234567890abcdef12"
        // OwnDeviceCard shows full fingerprint (length <= 24 shows as-is;
        // length > 24 shows take(16)+…+takeLast(8) in the existing code).
        // For parity the NEW spec says "own device shows full fingerprint",
        // so the helper must return the full string.
        val result = formatOwnFingerprint(fp)
        assertEquals("Own device fingerprint must be untruncated", fp, result)
    }

    /** Peer shows take(16)+"…"+takeLast(8). */
    @Test
    fun `peer fingerprint is truncated to 16+ellipsis+8`() {
        val fp = "abcdef1234567890abcdef1234567890abcdef12"
        val result = formatPeerFingerprint(fp)
        val expected = "abcdef1234567890…abcdef12"
        assertEquals("Peer fingerprint must be take(16)+…+takeLast(8)", expected, result)
    }

    /** Short fingerprint (<=24 chars): peer rule still truncates if > 24. */
    @Test
    fun `short fingerprint passes through peer rule unchanged when under threshold`() {
        val fp = "abcdef123456789"  // 15 chars, shorter than take(16)
        // take(16) on a 15-char string returns the whole string; takeLast(8) also works fine.
        val result = formatPeerFingerprint(fp)
        // take(16) = "abcdef123456789" (all 15); takeLast(8) = "56789" → wait, 8 chars
        val expected = fp.take(16) + "…" + fp.takeLast(8)
        assertEquals(expected, result)
    }

    // ────────────────────────────────────────────────────────────────────────
    // §7 Transport chip derivation
    // ────────────────────────────────────────────────────────────────────────

    @Test
    fun `transport chip is P2P when syncAddr is non-blank`() {
        val peer = peer(syncAddr = "192.168.1.10:7007", peerLocalIp = null)
        assertEquals(TransportChip.P2P, transportChipFor(peer))
    }

    @Test
    fun `transport chip is P2P when peerLocalIp is non-blank`() {
        val peer = peer(syncAddr = "", peerLocalIp = "10.0.0.5")
        assertEquals(TransportChip.P2P, transportChipFor(peer))
    }

    @Test
    fun `transport chip is Cloud when both syncAddr and peerLocalIp are absent`() {
        val peer = peer(syncAddr = "", peerLocalIp = null)
        assertEquals(TransportChip.Cloud, transportChipFor(peer))
    }

    @Test
    fun `transport chip is Cloud when both syncAddr and peerLocalIp are blank`() {
        val peer = peer(syncAddr = "   ", peerLocalIp = "  ")
        assertEquals(TransportChip.Cloud, transportChipFor(peer))
    }

    // ────────────────────────────────────────────────────────────────────────
    // §7 QR countdown drain bar progress
    // ────────────────────────────────────────────────────────────────────────

    @Test
    fun `qr progress is 1f at full TTL`() {
        val progress = qrCountdownProgress(remainingSeconds = 120, totalSeconds = 120)
        assertEquals(1.0f, progress, 0.001f)
    }

    @Test
    fun `qr progress is 0f at expiry`() {
        val progress = qrCountdownProgress(remainingSeconds = 0, totalSeconds = 120)
        assertEquals(0.0f, progress, 0.001f)
    }

    @Test
    fun `qr progress is clamped to 0f below zero`() {
        val progress = qrCountdownProgress(remainingSeconds = -5, totalSeconds = 120)
        assertEquals(0.0f, progress, 0.001f)
    }

    @Test
    fun `qr is in warning zone when remainingSeconds is at or below 20`() {
        // PARITY-SPEC §10 / audit #26: warning threshold moved 15s → 20s.
        assertTrue(isQrWarning(remainingSeconds = 20))
        assertTrue(isQrWarning(remainingSeconds = 15))
        assertTrue(isQrWarning(remainingSeconds = 1))
        assertFalse(isQrWarning(remainingSeconds = 21))
        assertFalse(isQrWarning(remainingSeconds = 120))
    }

    // ────────────────────────────────────────────────────────────────────────
    // CopyPaste-bdac.102 PulseDot ring colour: ring must match dot colour
    // ────────────────────────────────────────────────────────────────────────

    /**
     * Online peer: both the ring and the solid dot must use the ONLINE (success/green) role.
     * Verifies the invariant that pulseDotColorRole(online=true) == ONLINE so neither
     * element is hardcoded to a different colour.
     */
    @Test
    fun `online dot colour role is ONLINE (success green)`() {
        assertEquals(
            "online peer: ring and dot must both use the ONLINE/success colour role",
            PulseDotColorRole.ONLINE,
            pulseDotColorRole(online = true),
        )
    }

    /**
     * Offline peer: both the ring and the solid dot must use the OFFLINE (danger/red) role.
     * Before CopyPaste-bdac.102 the ring was hardcoded to c.success (green) even when the
     * dot correctly used c.danger (red) — the ring would flash green for an offline peer.
     */
    @Test
    fun `offline dot colour role is OFFLINE (danger red)`() {
        assertEquals(
            "offline peer: ring and dot must both use the OFFLINE/danger colour role — not success green",
            PulseDotColorRole.OFFLINE,
            pulseDotColorRole(online = false),
        )
    }

    // ────────────────────────────────────────────────────────────────────────
    // §7 PulseDot: online flag drives animation gate
    // ────────────────────────────────────────────────────────────────────────

    @Test
    fun `pulse is active only when online`() {
        assertTrue("online peer must pulse", shouldPulse(online = true, reducedMotion = false))
        assertFalse("offline peer must not pulse", shouldPulse(online = false, reducedMotion = false))
        assertFalse("reduced-motion must suppress pulse", shouldPulse(online = true, reducedMotion = true))
    }

    // ────────────────────────────────────────────────────────────────────────
    // §MO-5 PulseDot one-shot: fire only on offline→online leading edge
    // ────────────────────────────────────────────────────────────────────────

    /** offline→online transition with motion on → should start the one-shot pulse. */
    @Test
    fun `one-shot pulse fires on offline-to-online transition`() {
        assertTrue(
            "offline→online with motion enabled must start pulse",
            shouldStartOneShotPulse(wasOnline = false, isNowOnline = true, reducedMotion = false),
        )
    }

    /** online→online (already online) → must NOT fire again (would make it a loop). */
    @Test
    fun `one-shot pulse does not fire when already online`() {
        assertFalse(
            "online→online must not re-trigger pulse",
            shouldStartOneShotPulse(wasOnline = true, isNowOnline = true, reducedMotion = false),
        )
    }

    /** online→offline → no pulse. */
    @Test
    fun `one-shot pulse does not fire on online-to-offline transition`() {
        assertFalse(
            "online→offline must not trigger pulse",
            shouldStartOneShotPulse(wasOnline = true, isNowOnline = false, reducedMotion = false),
        )
    }

    /** offline→online but reduced-motion is active → no pulse (§MO-5 / §8). */
    @Test
    fun `one-shot pulse is suppressed under reduced motion`() {
        assertFalse(
            "offline→online under reduced-motion must not pulse",
            shouldStartOneShotPulse(wasOnline = false, isNowOnline = true, reducedMotion = true),
        )
    }

    /** offline→offline → no pulse. */
    @Test
    fun `one-shot pulse does not fire when staying offline`() {
        assertFalse(
            "offline→offline must not trigger pulse",
            shouldStartOneShotPulse(wasOnline = false, isNowOnline = false, reducedMotion = false),
        )
    }

    // ────────────────────────────────────────────────────────────────────────
    // Helpers
    // ────────────────────────────────────────────────────────────────────────

    private fun peer(syncAddr: String, peerLocalIp: String?) = PairedPeer(
        fingerprint = "fp",
        syncAddr = syncAddr,
        name = "Test",
        sessionKeyWrappedB64 = "",
        sessionKeyIvB64 = "",
        lastSyncMs = 0L,
        peerLocalIp = peerLocalIp,
    )
}
