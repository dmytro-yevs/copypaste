package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for the origin-device filter feature added to the Android history list.
 *
 * These are pure-JVM tests (no Android SDK or emulator required), runnable via
 * `./gradlew :app:testDebugUnitTest`.
 *
 * Covers:
 *  1. [ClipboardItem] carries an [ClipboardItem.originDeviceId] field.
 *  2. [filterByDevice] helper correctly filters a list by device id.
 *  3. "all" sentinel keeps the full list.
 *  4. [distinctOriginDeviceIds] returns unique non-blank device ids.
 *  5. [deviceDisplayName] returns a short label for own device vs. peer devices.
 */
class OriginDeviceFilterTest {

    private val ownId = "device-aaa"
    private val peerIdB = "device-bbb"
    private val peerIdC = "device-ccc"

    private fun makeItem(id: String, originDeviceId: String?) = ClipboardItem(
        id = id,
        contentType = "text/plain",
        isSensitive = false,
        wallTimeMs = 1_000L,
        originDeviceId = originDeviceId,
    )

    private val items = listOf(
        makeItem("1", ownId),
        makeItem("2", peerIdB),
        makeItem("3", peerIdB),
        makeItem("4", peerIdC),
        makeItem("5", null),   // legacy item — no origin recorded
    )

    // ─────────────────────────────────────────────────────────────────────────
    // 1. ClipboardItem has an originDeviceId field
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    fun `ClipboardItem has originDeviceId field`() {
        val item = makeItem("test", ownId)
        assertNotNull("originDeviceId must be present", item.originDeviceId)
        assertEquals(ownId, item.originDeviceId)
    }

    @Test
    fun `ClipboardItem originDeviceId defaults to null`() {
        val item = ClipboardItem(
            id = "x",
            contentType = "text/plain",
            isSensitive = false,
            wallTimeMs = 1L,
        )
        assertNull("originDeviceId must default to null", item.originDeviceId)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 2–3. filterByDevice helper
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    fun `filterByDevice with 'all' returns full list`() {
        val result = filterByDevice(items, "all")
        assertEquals(items.size, result.size)
    }

    @Test
    fun `filterByDevice with specific id returns only matching items`() {
        val result = filterByDevice(items, peerIdB)
        assertEquals(2, result.size)
        assertTrue(result.all { it.originDeviceId == peerIdB })
    }

    @Test
    fun `filterByDevice with own id returns own items`() {
        val result = filterByDevice(items, ownId)
        assertEquals(1, result.size)
        assertEquals("1", result.first().id)
    }

    @Test
    fun `filterByDevice with unknown id returns empty list`() {
        val result = filterByDevice(items, "device-zzz")
        assertTrue(result.isEmpty())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 4. distinctOriginDeviceIds
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    fun `distinctOriginDeviceIds returns unique non-blank ids`() {
        val ids = distinctOriginDeviceIds(items)
        assertEquals(3, ids.size)
        assertTrue(ids.contains(ownId))
        assertTrue(ids.contains(peerIdB))
        assertTrue(ids.contains(peerIdC))
        // null item must not appear
        assertTrue(ids.none { it.isBlank() })
    }

    @Test
    fun `distinctOriginDeviceIds on empty list returns empty`() {
        assertTrue(distinctOriginDeviceIds(emptyList()).isEmpty())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 5. deviceDisplayName
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    fun `deviceDisplayName returns 'This device' for own id`() {
        val label = deviceDisplayName(ownId, ownId, emptyList())
        assertEquals("This device", label)
    }

    @Test
    fun `deviceDisplayName returns peer name from roster`() {
        val peer = PairedPeer(fingerprint = peerIdB, syncAddr = "", name = "MacBook Pro")
        val label = deviceDisplayName(peerIdB, ownId, listOf(peer))
        assertEquals("MacBook Pro", label)
    }

    @Test
    fun `deviceDisplayName returns short id fallback when peer not in roster`() {
        val label = deviceDisplayName(peerIdC, ownId, emptyList())
        // Should be the first 8 chars of the id
        assertEquals(peerIdC.take(8), label)
    }
}
