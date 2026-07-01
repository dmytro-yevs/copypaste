package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.34 — [RelayEnvelope] characterization tests, restored after
 * extraction from [SyncManager] into a standalone file.
 *
 * Exercises ONLY the PURE surface — the legacy V1 JSON envelope
 * ([RelayEnvelope.encode]/[RelayEnvelope.parse]) and the V2 raw framing
 * ([RelayEnvelope.Companion.buildV2FrameBytes]/[RelayEnvelope.Companion.parseV2FrameBytes]).
 * This module's plain JUnit4 unit tests run with `isReturnDefaultValues = true`
 * (no Robolectric) — `android.util.Base64` stubs return null, so
 * `encodeWireV2`/`decodeWire` (which cross that boundary) are NOT exercised
 * here; only on-device/instrumented tests can cover them.
 */
class RelayEnvelopeTest {

    // ── V1 JSON envelope round-trip ───────────────────────────────────────

    @Test
    fun `V1 encode then parse round-trips a live item envelope`() {
        val original = RelayEnvelope(
            itemId = "item-123",
            lamportTs = 42L,
            ctB64 = "c29tZS1jaXBoZXJ0ZXh0", // "some-ciphertext" base64
            deleted = false,
            pinned = true,
            pinOrder = 3.0,
            wallTime = 1_700_000_000_000L,
            originDeviceId = "device-abc",
        )

        val parsed = RelayEnvelope.parse(original.encode())

        assertEquals(original, parsed)
    }

    @Test
    fun `V1 encode then parse round-trips a tombstone with empty ct_b64`() {
        val tombstone = RelayEnvelope(
            itemId = "item-456",
            lamportTs = 7L,
            ctB64 = "",
            deleted = true,
        )

        val parsed = RelayEnvelope.parse(tombstone.encode())

        assertEquals(tombstone, parsed)
        assertNotNull(parsed)
        assertTrue(parsed!!.deleted)
        assertEquals("", parsed.ctB64)
    }

    @Test
    fun `V1 parse rejects empty ct_b64 for a live (non-deleted) item`() {
        val malformed = RelayEnvelope(
            itemId = "item-789",
            lamportTs = 1L,
            ctB64 = "",
            deleted = false,
        ).encode()

        assertNull(RelayEnvelope.parse(malformed))
    }

    @Test
    fun `V1 parse rejects blank item_id`() {
        assertNull(RelayEnvelope.parse("""{"item_id":"","ct_b64":"abc"}"""))
    }

    @Test
    fun `V1 parse returns null for malformed JSON`() {
        assertNull(RelayEnvelope.parse("not json at all"))
    }

    @Test
    fun `V1 parse defaults pin_order to null when absent`() {
        val parsed = RelayEnvelope.parse(
            """{"item_id":"item-1","ct_b64":"YQ==","lamport_ts":5}""",
        )
        assertNotNull(parsed)
        assertNull(parsed!!.pinOrder)
        assertEquals(false, parsed.pinned)
    }

    // ── V2 pure framing round-trip (buildV2FrameBytes / parseV2FrameBytes) ───

    @Test
    fun `V2 buildV2FrameBytes then parseV2FrameBytes round-trips a live item`() {
        val ct = byteArrayOf(1, 2, 3, 4, 5, 6, 7)
        val frame = RelayEnvelope.buildV2FrameBytes(
            itemId = "item-v2",
            lamportTs = 99L,
            deleted = false,
            pinned = true,
            pinOrder = 2.5,
            wallTime = 1_650_000_000_000L,
            originDeviceId = "device-xyz",
            ct = ct,
        )

        val parsed = RelayEnvelope.parseV2FrameBytes(frame)

        assertNotNull(parsed)
        assertEquals("item-v2", parsed!!.itemId)
        assertEquals(99L, parsed.lamportTs)
        assertEquals(false, parsed.deleted)
        assertEquals(true, parsed.pinned)
        assertEquals(2.5, parsed.pinOrder!!, 0.0)
        assertEquals(1_650_000_000_000L, parsed.wallTime)
        assertEquals("device-xyz", parsed.originDeviceId)
        assertArrayEquals(ct, parsed.ct)
    }

    @Test
    fun `V2 buildV2FrameBytes then parseV2FrameBytes round-trips a tombstone with empty ct`() {
        val frame = RelayEnvelope.buildV2FrameBytes(
            itemId = "item-tomb",
            lamportTs = 3L,
            deleted = true,
            pinned = false,
            pinOrder = null,
            wallTime = 0L,
            originDeviceId = "",
            ct = ByteArray(0),
        )

        val parsed = RelayEnvelope.parseV2FrameBytes(frame)

        assertNotNull(parsed)
        assertTrue(parsed!!.deleted)
        assertEquals(0, parsed.ct.size)
        assertNull(parsed.pinOrder)
    }

    @Test
    fun `V2 frame starts with the RELAY_WIRE_V2 marker byte`() {
        val frame = RelayEnvelope.buildV2FrameBytes(
            itemId = "item-marker",
            lamportTs = 1L,
            deleted = false,
            pinned = false,
            pinOrder = null,
            wallTime = 0L,
            originDeviceId = "",
            ct = byteArrayOf(9),
        )

        assertEquals(RelayEnvelope.RELAY_WIRE_V2, frame[0])
    }

    @Test
    fun `V2 parseV2FrameBytes returns null for too-short input`() {
        assertNull(RelayEnvelope.parseV2FrameBytes(byteArrayOf(RelayEnvelope.RELAY_WIRE_V2, 0, 0)))
    }

    @Test
    fun `V2 parseV2FrameBytes returns null when meta length overruns the buffer`() {
        // Marker byte + a metaLen (u32-LE) far larger than the remaining buffer.
        val bogus = byteArrayOf(
            RelayEnvelope.RELAY_WIRE_V2,
            0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0x7F.toByte(),
        )
        assertNull(RelayEnvelope.parseV2FrameBytes(bogus))
    }

    @Test
    fun `V2 parseV2FrameBytes returns null for wrong marker byte`() {
        val frame = RelayEnvelope.buildV2FrameBytes(
            itemId = "item-x",
            lamportTs = 1L,
            deleted = false,
            pinned = false,
            pinOrder = null,
            wallTime = 0L,
            originDeviceId = "",
            ct = byteArrayOf(1),
        )
        frame[0] = 0x02 // corrupt the version marker
        assertNull(RelayEnvelope.parseV2FrameBytes(frame))
    }
}
