package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * #10 — Cloud-file payload header parity: golden bytes test (Android side).
 *
 * Source of truth: `SyncManager.encodeCloudFilePayload` in
 *   android/app/src/main/java/com/copypaste/android/SyncManager.kt
 *
 * Wire format (all multi-byte integers big-endian) — byte-for-byte identical to
 * `encode_cloud_file_payload` / `decode_cloud_file_payload` in
 * `crates/copypaste-daemon/src/sync_common.rs`:
 *   [1 byte  version = 1]
 *   [2 bytes name_len][name_len bytes UTF-8 file name]
 *   [2 bytes mime_len][mime_len bytes UTF-8 MIME type]
 *   [file bytes ...]
 *
 * The companion Rust test lives at:
 *   crates/copypaste-daemon/src/sync_common.rs :: cloud_file_payload_golden_bytes
 *
 * BOTH tests use the SAME canonical test vector and SAME expected byte sequence.
 * If either test fails after a wire-format change, update BOTH.
 */
class CloudFilePayloadParityTest {

    /**
     * Canonical golden bytes test — must be byte-for-byte identical to the Rust
     * golden test `cloud_file_payload_golden_bytes` in sync_common.rs.
     *
     * Test vector:
     *   name = "hello.txt"   (9 UTF-8 bytes)
     *   mime = "text/plain"  (10 UTF-8 bytes)
     *   body = "BODY"        (4 bytes)
     *
     * Expected wire layout:
     *   [0x01]              — version byte = 1
     *   [0x00, 0x09]        — name_len = 9 (big-endian u16)
     *   "hello.txt" (9 B)
     *   [0x00, 0x0A]        — mime_len = 10 (big-endian u16)
     *   "text/plain" (10 B)
     *   "BODY" (4 B)
     */
    @Test
    fun `encodeCloudFilePayload golden bytes match Rust wire format`() {
        val name = "hello.txt"   // 9 UTF-8 bytes
        val mime = "text/plain"  // 10 UTF-8 bytes
        val body = "BODY".toByteArray(Charsets.UTF_8) // 4 bytes

        val encoded = SyncManager.encodeCloudFilePayload(name, mime, body)

        // Build expected bytes from the documented wire format (same as Rust test).
        val expected = byteArrayOf(
            // version byte = 1
            0x01.toByte(),
            // name_len = 9 (big-endian u16)
            0x00.toByte(), 0x09.toByte(),
        ) +
            "hello.txt".toByteArray(Charsets.UTF_8) +
            byteArrayOf(
                // mime_len = 10 (big-endian u16)
                0x00.toByte(), 0x0A.toByte(),
            ) +
            "text/plain".toByteArray(Charsets.UTF_8) +
            "BODY".toByteArray(Charsets.UTF_8)

        assertArrayEquals(
            "SyncManager.encodeCloudFilePayload golden bytes mismatch — " +
                "if the wire format changed, update sync_common.rs :: " +
                "cloud_file_payload_golden_bytes (Rust) too",
            expected,
            encoded,
        )
    }

    /**
     * Cross-check: decode must round-trip what encode produced.
     * Mirrors the Rust cross-check in the same golden test.
     */
    @Test
    fun `decodeCloudFilePayload round-trips the encoded golden vector`() {
        val name = "hello.txt"
        val mime = "text/plain"
        val body = "BODY".toByteArray(Charsets.UTF_8)

        val encoded = SyncManager.encodeCloudFilePayload(name, mime, body)
        val decoded = SyncManager.decodeCloudFilePayload(encoded)

        assertEquals("round-trip name", name, decoded.name)
        assertEquals("round-trip mime", mime, decoded.mime)
        assertArrayEquals("round-trip body", body, decoded.body)
    }
}
