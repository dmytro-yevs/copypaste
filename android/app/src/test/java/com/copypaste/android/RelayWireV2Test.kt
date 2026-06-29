package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the CopyPaste-crh3.69 single-base64 V2 relay wire
 * framing and its version-gated backward compatibility with the legacy V1
 * double-base64 envelope.
 *
 * These exercise the base64-free framing core ([SyncManager.RelayEnvelope.buildV2FrameBytes]
 * / [parseV2FrameBytes]) directly, plus the legacy [SyncManager.RelayEnvelope.parse]
 * path, so they run under `:app:testDebugUnitTest` WITHOUT the unit-test-stubbed
 * `android.util.Base64`. The outer base64 boundary (identical in production) is
 * simulated here with `java.util.Base64` purely for the size comparison.
 */
class RelayWireV2Test {

    private fun sampleCt(n: Int): ByteArray = ByteArray(n) { (it % 251).toByte() }

    /** Build the legacy V1 wire JSON (the inner payload that V1 base64-wrapped). */
    private fun legacyV1Json(itemId: String, lamportTs: Long, ct: ByteArray): String {
        val ctB64 = java.util.Base64.getEncoder().encodeToString(ct)
        return SyncManager.RelayEnvelope(
            itemId = itemId,
            lamportTs = lamportTs,
            ctB64 = ctB64,
            pinned = true,
            pinOrder = 3.5,
            wallTime = 1_700_000_000_123L,
            originDeviceId = "dev-origin",
        ).encode()
    }

    // ── V2 framing round-trip ─────────────────────────────────────────────────

    @Test
    fun v2Frame_roundTrips_metadataAndRawCiphertext() {
        val ct = sampleCt(512)
        val frame = SyncManager.RelayEnvelope.buildV2FrameBytes(
            itemId = "item-abc-123",
            lamportTs = 42L,
            deleted = false,
            pinned = true,
            pinOrder = 3.5,
            wallTime = 1_700_000_000_123L,
            originDeviceId = "dev-origin",
            ct = ct,
        )

        // Leading byte is the V2 marker, NOT a JSON brace.
        assertEquals(SyncManager.RelayEnvelope.RELAY_WIRE_V2, frame[0])
        assertNotEquals('{'.code.toByte(), frame[0])

        val parsed = SyncManager.RelayEnvelope.parseV2FrameBytes(frame)!!
        assertEquals("item-abc-123", parsed.itemId)
        assertEquals(42L, parsed.lamportTs)
        assertEquals(false, parsed.deleted)
        assertEquals(true, parsed.pinned)
        assertEquals(3.5, parsed.pinOrder!!, 0.0001)
        assertEquals(1_700_000_000_123L, parsed.wallTime)
        assertEquals("dev-origin", parsed.originDeviceId)
        assertArrayEquals(ct, parsed.ct)
    }

    @Test
    fun v2Frame_tombstone_roundTrips_emptyCiphertext() {
        val frame = SyncManager.RelayEnvelope.buildV2FrameBytes(
            itemId = "tomb-1",
            lamportTs = 9L,
            deleted = true,
            pinned = false,
            pinOrder = null,
            wallTime = 123L,
            originDeviceId = "dev-x",
            ct = ByteArray(0),
        )
        val parsed = SyncManager.RelayEnvelope.parseV2FrameBytes(frame)!!
        assertTrue(parsed.deleted)
        assertEquals(0, parsed.ct.size)
        assertEquals("tomb-1", parsed.itemId)
    }

    @Test
    fun parseV2FrameBytes_truncatedFrame_returnsNull() {
        // A 3-byte buffer cannot hold the 4-byte length prefix.
        assertNull(SyncManager.RelayEnvelope.parseV2FrameBytes(byteArrayOf(0x01, 0x00, 0x00)))
        // Claims a 1000-byte metadata that isn't present.
        val bogus = byteArrayOf(0x01, 0xE8.toByte(), 0x03, 0x00, 0x00, '{'.code.toByte())
        assertNull(SyncManager.RelayEnvelope.parseV2FrameBytes(bogus))
    }

    // ── Backward compatibility: legacy V1 still decodes ───────────────────────

    @Test
    fun legacyV1Envelope_stillDecodes() {
        val ct = sampleCt(300)
        val json = legacyV1Json("item-abc-123", 42L, ct)

        // The legacy inner payload is a JSON object (starts with '{').
        assertEquals('{'.code.toByte(), json.toByteArray(Charsets.UTF_8)[0])

        val env = SyncManager.RelayEnvelope.parse(json)!!
        assertEquals("item-abc-123", env.itemId)
        assertEquals(42L, env.lamportTs)
        assertTrue(env.pinned)
        assertEquals(3.5, env.pinOrder!!, 0.0001)
        assertEquals(1_700_000_000_123L, env.wallTime)
        assertEquals("dev-origin", env.originDeviceId)
        // ctB64 round-trips back to the original ciphertext.
        assertArrayEquals(ct, java.util.Base64.getDecoder().decode(env.ctB64))
    }

    // ── Golden wire size: V2 is materially smaller than legacy V1 ─────────────

    @Test
    fun v2_isSmallerThanLegacyDoubleBase64() {
        val ct = sampleCt(4096)

        // V2 wire = base64(frame).
        val frame = SyncManager.RelayEnvelope.buildV2FrameBytes(
            itemId = "item-abc-123",
            lamportTs = 42L,
            deleted = false,
            pinned = true,
            pinOrder = 3.5,
            wallTime = 1_700_000_000_123L,
            originDeviceId = "dev-origin",
            ct = ct,
        )
        val v2Wire = java.util.Base64.getEncoder().encodeToString(frame)

        // Legacy V1 wire = base64(JSON{..,ct_b64:base64(ct)}).
        val v1Json = legacyV1Json("item-abc-123", 42L, ct)
        val v1Wire = java.util.Base64.getEncoder().encodeToString(v1Json.toByteArray(Charsets.UTF_8))

        assertTrue(
            "v2 (${v2Wire.length}) must be smaller than legacy v1 (${v1Wire.length})",
            v2Wire.length < v1Wire.length,
        )
        val ratio = v1Wire.length.toDouble() / v2Wire.length.toDouble()
        assertTrue(
            "legacy must be >=1.3x the v2 size (got $ratio: v1=${v1Wire.length}, v2=${v2Wire.length})",
            ratio >= 1.3,
        )
    }
}
