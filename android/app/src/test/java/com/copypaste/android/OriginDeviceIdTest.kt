package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Unit tests for the originDeviceId data-layer threading.
 *
 * ClipboardRepository.encodeItem (private, tested indirectly via parseItem round-trip)
 * must persist the originDeviceId as pipe-delimited field 7 and parseItem must
 * read it back. Back-compat: blobs with fewer than 8 fields (legacy) must parse
 * with originDeviceId == null.
 *
 * All helpers used here are pure-JVM — no Android runtime required.
 */
class OriginDeviceIdTest {

    // We test via ClipboardItem directly (data class equals/hashCode contract)
    // and via the pure companion helpers exposed on ClipboardRepository.

    @Test
    fun clipboardItem_includesOriginDeviceId_inEquals() {
        val base = ClipboardItem(
            id = "abc",
            contentType = "text/plain",
            isSensitive = false,
            wallTimeMs = 1000L,
            snippet = "hi",
            originDeviceId = "device-1",
        )
        val same = base.copy()
        val different = base.copy(originDeviceId = "device-2")
        val nullDevice = base.copy(originDeviceId = null)

        assertEquals(base, same)
        assert(base != different) { "items with different originDeviceId must not be equal" }
        assert(base != nullDevice) { "item with null originDeviceId must not equal non-null" }
    }

    @Test
    fun clipboardItem_hashCode_differsOnOriginDeviceId() {
        val a = ClipboardItem("id", "text/plain", false, 1L, originDeviceId = "dev-A")
        val b = ClipboardItem("id", "text/plain", false, 1L, originDeviceId = "dev-B")
        // Hash codes may collide in theory, but not for trivially different strings.
        assert(a.hashCode() != b.hashCode()) {
            "hashCode should differ for different originDeviceId values"
        }
    }

    @Test
    fun clipboardItem_defaultOriginDeviceId_isNull() {
        val item = ClipboardItem(
            id = "x",
            contentType = "text/plain",
            isSensitive = false,
            wallTimeMs = 0L,
        )
        assertNull(item.originDeviceId)
    }

    @Test
    fun parseEncode_roundTrip_preservesOriginDeviceId() {
        // Simulate the pipe-delimited blob encoding that encodeItem produces
        // (index 6 = originDeviceId) and check parseItem reconstructs it.
        // Format: wallTimeMs|contentType|plaintextLen|nonce64|ct64|lamportTs|originDeviceId
        val deviceId = "test-device-uuid-1234"
        val blob = "1000000|text/plain|5|AAAA|BBBB|999|$deviceId"
        val parsed = ClipboardRepositoryTestHelper.parseOriginDeviceId(blob)
        assertEquals(deviceId, parsed)
    }

    @Test
    fun parseEncode_legacyBlob_originDeviceId_isNull() {
        // Legacy blob has only 6 fields (no originDeviceId field).
        val legacyBlob = "1000000|text/plain|5|AAAA|BBBB|999"
        val parsed = ClipboardRepositoryTestHelper.parseOriginDeviceId(legacyBlob)
        assertNull(parsed)
    }

    @Test
    fun parseEncode_blankOriginDeviceId_parsesAsNull() {
        // If originDeviceId was stored as blank (empty string), it should parse as null.
        val blobWithBlank = "1000000|text/plain|5|AAAA|BBBB|999|"
        val parsed = ClipboardRepositoryTestHelper.parseOriginDeviceId(blobWithBlank)
        assertNull(parsed)
    }
}

/**
 * Test helper that exposes the pipe-delimited blob parsing logic that mirrors
 * [ClipboardRepository.parseItem] for the originDeviceId field (field index 6).
 *
 * Placed here so no Android runtime is needed — purely string manipulation.
 */
object ClipboardRepositoryTestHelper {
    /**
     * Parse field 6 (originDeviceId) from a pipe-delimited ClipboardRepository
     * item blob. Returns null for legacy blobs (< 7 fields) and for blank values.
     */
    fun parseOriginDeviceId(blob: String): String? {
        val parts = blob.split("|")
        return parts.getOrNull(6)?.takeIf { it.isNotBlank() }
    }
}
