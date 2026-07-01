package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * CopyPaste-vp63.34 — [CloudFilePayloadCodec] characterization tests.
 *
 * Restores coverage for the cloud file-identity envelope after its extraction
 * from [SyncManager]'s companion object into a standalone PURE, JVM-testable
 * object. Source of truth for the golden-bytes vector: the companion Rust test
 * `cloud_file_payload_golden_bytes` in `crates/copypaste-daemon/src/sync_common.rs`
 * — BOTH tests must be updated together if the wire format ever changes.
 *
 * Also verifies [SyncManager.encodeCloudFilePayload]/[SyncManager.decodeCloudFilePayload]
 * still forward to the same implementation (call-site compatibility for
 * [ClipboardService] and [SupabaseClient]).
 */
class CloudFilePayloadCodecTest {

    /**
     * Canonical golden bytes test — must be byte-for-byte identical to the Rust
     * golden test `cloud_file_payload_golden_bytes` in sync_common.rs.
     *
     * Test vector:
     *   name = "hello.txt"   (9 UTF-8 bytes)
     *   mime = "text/plain"  (10 UTF-8 bytes)
     *   body = "BODY"        (4 bytes)
     */
    @Test
    fun `encodeCloudFilePayload golden bytes match Rust wire format`() {
        val name = "hello.txt"
        val mime = "text/plain"
        val body = "BODY".toByteArray(Charsets.UTF_8)

        val encoded = CloudFilePayloadCodec.encodeCloudFilePayload(name, mime, body)

        val expected = byteArrayOf(
            0x01.toByte(), // version byte = 1
            0x00.toByte(), 0x09.toByte(), // name_len = 9 (big-endian u16)
        ) +
            "hello.txt".toByteArray(Charsets.UTF_8) +
            byteArrayOf(0x00.toByte(), 0x0A.toByte()) + // mime_len = 10 (big-endian u16)
            "text/plain".toByteArray(Charsets.UTF_8) +
            "BODY".toByteArray(Charsets.UTF_8)

        assertArrayEquals(
            "CloudFilePayloadCodec.encodeCloudFilePayload golden bytes mismatch — " +
                "if the wire format changed, update sync_common.rs :: " +
                "cloud_file_payload_golden_bytes (Rust) too",
            expected,
            encoded,
        )
    }

    @Test
    fun `decodeCloudFilePayload round-trips the encoded golden vector`() {
        val name = "hello.txt"
        val mime = "text/plain"
        val body = "BODY".toByteArray(Charsets.UTF_8)

        val encoded = CloudFilePayloadCodec.encodeCloudFilePayload(name, mime, body)
        val decoded = CloudFilePayloadCodec.decodeCloudFilePayload(encoded)

        assertEquals("round-trip name", name, decoded.name)
        assertEquals("round-trip mime", mime, decoded.mime)
        assertArrayEquals("round-trip body", body, decoded.body)
    }

    @Test
    fun `decodeCloudFilePayload falls back to legacy name and mime for headerless payload`() {
        // No header at all — a payload uploaded by an old daemon (pre-fix).
        val raw = "just raw file bytes, no header".toByteArray(Charsets.UTF_8)

        val decoded = CloudFilePayloadCodec.decodeCloudFilePayload(raw)

        assertEquals(CloudFilePayloadCodec.CLOUD_FILE_LEGACY_NAME, decoded.name)
        assertEquals(CloudFilePayloadCodec.CLOUD_FILE_LEGACY_MIME, decoded.mime)
        assertArrayEquals("entire buffer treated as body", raw, decoded.body)
    }

    @Test
    fun `decodeCloudFilePayload falls back to legacy on truncated length field`() {
        // Valid version byte + a name_len field that overruns the buffer.
        val malformed = byteArrayOf(0x01.toByte(), 0x00.toByte(), 0x7F.toByte(), 0x41.toByte())

        val decoded = CloudFilePayloadCodec.decodeCloudFilePayload(malformed)

        assertEquals(CloudFilePayloadCodec.CLOUD_FILE_LEGACY_NAME, decoded.name)
        assertEquals(CloudFilePayloadCodec.CLOUD_FILE_LEGACY_MIME, decoded.mime)
        assertArrayEquals(malformed, decoded.body)
    }

    @Test
    fun `SyncManager forwarding stubs delegate to CloudFilePayloadCodec`() {
        val name = "a.bin"
        val mime = "application/octet-stream"
        val body = byteArrayOf(1, 2, 3)

        val viaSyncManager = SyncManager.encodeCloudFilePayload(name, mime, body)
        val viaCodec = CloudFilePayloadCodec.encodeCloudFilePayload(name, mime, body)
        assertArrayEquals("SyncManager encode must match CloudFilePayloadCodec encode", viaCodec, viaSyncManager)

        val decodedViaSyncManager = SyncManager.decodeCloudFilePayload(viaSyncManager)
        assertEquals(name, decodedViaSyncManager.name)
        assertEquals(mime, decodedViaSyncManager.mime)
        assertArrayEquals(body, decodedViaSyncManager.body)

        assertEquals(CloudFilePayloadCodec.CLOUD_FILE_LEGACY_NAME, SyncManager.CLOUD_FILE_LEGACY_NAME)
        assertEquals(CloudFilePayloadCodec.CLOUD_FILE_LEGACY_MIME, SyncManager.CLOUD_FILE_LEGACY_MIME)
    }
}
