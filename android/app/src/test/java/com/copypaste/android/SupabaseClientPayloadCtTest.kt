package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * JVM unit tests for the Supabase `payload_ct` bytea wire encoding.
 *
 * These mirror the macOS daemon's `encode_payload_ct_hex` / `decode_payload_ct`
 * (crates/copypaste-daemon/src/cloud.rs): the `payload_ct` column is a Postgres
 * `bytea`, so it must be written as the hex-input literal `\x<hex>` and read as
 * the hex-output form `\x<hex>` (with bare-base64 accepted for legacy rows).
 *
 * Pure-JVM (no android.util.* / no NDK), so they run under
 * `:app:testDebugUnitTest` without an emulator.
 */
class SupabaseClientPayloadCtTest {

    @Test
    fun encode_producesLowercaseBackslashXHex() {
        // 0x00 0x0F 0xA0 0xFF 0x10 → "\x000fa0ff10"
        val bytes = byteArrayOf(0x00, 0x0F, 0xA0.toByte(), 0xFF.toByte(), 0x10)
        assertEquals("\\x000fa0ff10", SupabaseClient.encodePayloadCt(bytes))
    }

    @Test
    fun encode_emptyBlob_isJustPrefix() {
        assertEquals("\\x", SupabaseClient.encodePayloadCt(ByteArray(0)))
    }

    @Test
    fun decode_hexLiteral_returnsOriginalBytes() {
        val original = byteArrayOf(0x00, 0x0F, 0xA0.toByte(), 0xFF.toByte(), 0x10)
        val decoded = SupabaseClient.decodePayloadCt("\\x000fa0ff10")
        assertArrayEquals(original, decoded)
    }

    @Test
    fun decode_base64_backCompat_returnsOriginalBytes() {
        val original = byteArrayOf(0x00, 0x0F, 0xA0.toByte(), 0xFF.toByte(), 0x10)
        val base64 = java.util.Base64.getEncoder().encodeToString(original)
        val decoded = SupabaseClient.decodePayloadCt(base64)
        assertArrayEquals(original, decoded)
    }

    @Test
    fun roundTrip_encodeThenDecode_isIdentity() {
        val original = ByteArray(256) { it.toByte() } // every byte value 0..255
        val wire = SupabaseClient.encodePayloadCt(original)
        assertEquals("\\x", wire.substring(0, 2))
        assertArrayEquals(original, SupabaseClient.decodePayloadCt(wire))
    }

    @Test
    fun encode_matchesExpectedHexForKnownBytes() {
        val bytes = "hello".toByteArray(Charsets.UTF_8) // 68 65 6c 6c 6f
        assertEquals("\\x68656c6c6f", SupabaseClient.encodePayloadCt(bytes))
    }
}
