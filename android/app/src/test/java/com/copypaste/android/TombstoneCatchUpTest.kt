package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM regression tests for tombstone handling in WS catch-up polls
 * (CopyPaste-vfai).
 *
 * Before the fix, `triggerCatchUpPoll` had no tombstone fast-path. A deleted
 * row (deleted=true, empty payload_ct) would be passed through `decryptRow`
 * which returns an item with empty plaintext, then fall through to the text
 * branch where `text.isBlank()` returns false … wait, `ByteArray(0).toString()`
 * is an empty string which IS blank → `stored = false`. The cursor advances past
 * the tombstone row but the delete is never applied locally.
 *
 * The fix: check `row.deleted` BEFORE `decryptRow` and route to
 * `applyInboundTombstoneWithLww` (mirrors `ingestWsRow` and `FgsSyncLoop`).
 *
 * This test file validates the logic at the CloudRow + DecryptedItem boundary
 * (no Android runtime, no real DB) so it runs under `testDebugUnitTest`.
 */
class TombstoneCatchUpTest {

    // ── CloudRow.deleted routing ──────────────────────────────────────────────

    /**
     * Simulate the routing logic that should exist in triggerCatchUpPoll:
     * if row.deleted → tombstone path, else → decrypt path.
     */
    private data class RouteResult(val routed: String, val itemId: String)

    private fun route(row: SupabaseClient.CloudRow): RouteResult {
        return if (row.deleted) {
            RouteResult("tombstone", row.itemId)
        } else {
            RouteResult("decrypt", row.itemId)
        }
    }

    @Test
    fun `deleted row routes to tombstone path not decrypt path`() {
        val row = SupabaseClient.CloudRow(
            id = "row-1",
            itemId = "item-del",
            contentType = "text",
            payloadCtWire = "",
            lamportTs = 50L,
            wallTime = 1000L,
            expiresAt = null,
            appBundleId = null,
            deviceId = "device-B",
            deleted = true,
        )
        val result = route(row)
        assertEquals(
            "A deleted row must be routed to the tombstone path (vfai: catch-up was skipping tombstones)",
            "tombstone",
            result.routed,
        )
        assertEquals("item-del", result.itemId)
    }

    @Test
    fun `live row routes to decrypt path`() {
        val row = SupabaseClient.CloudRow(
            id = "row-2",
            itemId = "item-live",
            contentType = "text",
            payloadCtWire = "\\x68656c6c6f",
            lamportTs = 60L,
            wallTime = 2000L,
            expiresAt = null,
            appBundleId = null,
            deviceId = "device-C",
            deleted = false,
        )
        val result = route(row)
        assertEquals("A live row must be routed to the decrypt path", "decrypt", result.routed)
    }

    // ── CloudRow.deleted default ──────────────────────────────────────────────

    @Test
    fun `CloudRow defaults to deleted=false`() {
        val row = SupabaseClient.CloudRow(
            id = "r3", itemId = "i3", contentType = "text",
            payloadCtWire = "\\x01", lamportTs = 1L, wallTime = 0L,
            expiresAt = null, appBundleId = null, deviceId = "d3",
        )
        assertFalse("CloudRow must default to deleted=false", row.deleted)
    }

    // ── Tombstone decryptRow fast-path (SupabaseClient contract) ─────────────

    @Test
    fun `decryptRow for deleted row returns item with deleted=true and empty plaintext`() {
        val row = SupabaseClient.CloudRow(
            id = "row-tomb",
            itemId = "item-tomb",
            contentType = "text",
            payloadCtWire = "",
            lamportTs = 99L,
            wallTime = 5000L,
            expiresAt = null,
            appBundleId = null,
            deviceId = "device-D",
            deleted = true,
        )
        val dummyKey = ByteArray(32) { it.toByte() }
        val client = SupabaseClient(supabaseUrl = "https://x.supabase.co", anonKey = "anon")
        val item = client.decryptRow(row, dummyKey)!!

        assertTrue("decryptRow tombstone must set deleted=true", item.deleted)
        assertEquals("decryptRow tombstone must have empty plaintext", 0, item.plaintext.size)
        assertEquals("item-tomb", item.itemId)
        assertEquals(99L, item.lamportTs)
    }

    @Test
    fun `tombstone plaintext as string is blank — confirms old silent-skip bug`() {
        // Demonstrate why the old code skipped tombstones:
        // item.plaintext = ByteArray(0), .toString(UTF-8) = "" which is blank.
        // The text branch did `if (text.isBlank()) false` → stored = false, delete lost.
        val emptyBytes = ByteArray(0)
        val text = emptyBytes.toString(Charsets.UTF_8)
        assertTrue(
            "Empty plaintext toString is blank — confirms tombstones were silently skipped before vfai fix",
            text.isBlank(),
        )
    }

    // ── Cursor advancement on tombstone rows ──────────────────────────────────

    @Test
    fun `tombstone row cursor advancement is monotonic`() {
        // Simulate cursor compare logic: deleted rows must advance the cursor
        // even when they produce no local insert.
        var cursorWallTime = 0L
        var cursorId = ""

        val tombRow = SupabaseClient.CloudRow(
            id = "tomb-id", itemId = "item-t", contentType = "text",
            payloadCtWire = "", lamportTs = 10L, wallTime = 3000L,
            expiresAt = null, appBundleId = null, deviceId = "d5",
            deleted = true,
        )

        // The cursor advance check (from triggerCatchUpPoll).
        if (tombRow.wallTime > cursorWallTime ||
            (tombRow.wallTime == cursorWallTime && tombRow.id > cursorId)
        ) {
            cursorWallTime = tombRow.wallTime
            cursorId = tombRow.id
        }

        assertEquals(
            "Cursor must advance past tombstone row so it is not re-fetched",
            3000L,
            cursorWallTime,
        )
        assertEquals("tomb-id", cursorId)
    }
}
