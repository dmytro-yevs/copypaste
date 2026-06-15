package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the relay envelope delete/pin fixes (CopyPaste-rmuw)
 * and the Lamport timestamp helper (CopyPaste-up1c).
 *
 * No Android runtime, no NDK — runs under `:app:testDebugUnitTest`.
 */
class RelayEnvelopeDeletePinTest {

    // ── RelayEnvelope parse ───────────────────────────────────────────────────

    @Test
    fun parse_liveEnvelope_returnsEnvelopeWithDefaults() {
        val json = """{"item_id":"abc","lamport_ts":42,"ct_b64":"aGVsbG8="}"""
        val env = SyncManager.RelayEnvelope.parse(json)!!
        assertEquals("abc", env.itemId)
        assertEquals(42L, env.lamportTs)
        assertEquals("aGVsbG8=", env.ctB64)
        assertFalse(env.deleted)
        assertFalse(env.pinned)
        assertNull(env.pinOrder)
        assertEquals(0L, env.wallTime)
        assertEquals("", env.originDeviceId)
    }

    @Test
    fun parse_tombstoneEnvelope_allowsEmptyCtB64() {
        // A daemon-emitted tombstone carries deleted=true and empty ct_b64.
        val json = """{"item_id":"dead-item","lamport_ts":100,"ct_b64":"","deleted":true}"""
        val env = SyncManager.RelayEnvelope.parse(json)!!
        assertEquals("dead-item", env.itemId)
        assertEquals(100L, env.lamportTs)
        assertEquals("", env.ctB64)
        assertTrue(env.deleted)
    }

    @Test
    fun parse_emptyCtB64WithoutDeleted_returnsNull() {
        // Empty ct_b64 with deleted=false (or absent) is malformed — must reject.
        val json = """{"item_id":"xyz","lamport_ts":5,"ct_b64":""}"""
        assertNull(SyncManager.RelayEnvelope.parse(json))
    }

    @Test
    fun parse_pinnedEnvelope_readsPinFields() {
        val json = """{"item_id":"pinned-item","lamport_ts":200,"ct_b64":"aGVsbG8=",
            |"pinned":true,"pin_order":1.5,"wall_time":99999,"origin_device_id":"dev-A"}"""
            .trimMargin()
        val env = SyncManager.RelayEnvelope.parse(json)!!
        assertTrue(env.pinned)
        assertEquals(1.5, env.pinOrder!!, 0.0001)
        assertEquals(99999L, env.wallTime)
        assertEquals("dev-A", env.originDeviceId)
    }

    @Test
    fun parse_nullPinOrder_returnsNullPinOrder() {
        val json = """{"item_id":"x","lamport_ts":1,"ct_b64":"aGVsbG8=","pin_order":null}"""
        val env = SyncManager.RelayEnvelope.parse(json)!!
        assertNull(env.pinOrder)
    }

    // ── RelayEnvelope encode ─────────────────────────────────────────────────

    @Test
    fun encode_liveEnvelope_roundTrips() {
        val original = SyncManager.RelayEnvelope(
            itemId = "abc",
            lamportTs = 42L,
            ctB64 = "aGVsbG8=",
            deleted = false,
            pinned = true,
            pinOrder = 2.0,
            wallTime = 1234L,
            originDeviceId = "dev-1",
        )
        val parsed = SyncManager.RelayEnvelope.parse(original.encode())!!
        assertEquals("abc", parsed.itemId)
        assertEquals(42L, parsed.lamportTs)
        assertEquals("aGVsbG8=", parsed.ctB64)
        assertFalse(parsed.deleted)
        assertTrue(parsed.pinned)
        assertEquals(2.0, parsed.pinOrder!!, 0.0001)
        assertEquals(1234L, parsed.wallTime)
        assertEquals("dev-1", parsed.originDeviceId)
    }

    @Test
    fun encode_tombstoneEnvelope_roundTrips() {
        val tombstone = SyncManager.RelayEnvelope(
            itemId = "dead",
            lamportTs = 500L,
            ctB64 = "",
            deleted = true,
        )
        val json = tombstone.encode()
        val parsed = SyncManager.RelayEnvelope.parse(json)!!
        assertEquals("dead", parsed.itemId)
        assertEquals(500L, parsed.lamportTs)
        assertEquals("", parsed.ctB64)
        assertTrue(parsed.deleted)
    }

    // ── nextLamportTs ────────────────────────────────────────────────────────

    @Test
    fun nextLamportTs_prevPlusOneGreaterThanNow_returnsPrevPlusOne() {
        // When prev+1 >= nowMs, returns prev+1 (monotone increment wins).
        val nowMs = 1000L
        val prev = 1500L
        assertEquals(1501L, ClipboardRepository.nextLamportTs(prev, nowMs))
    }

    @Test
    fun nextLamportTs_nowMsGreaterThanPrevPlusOne_returnsNowMs() {
        // When nowMs > prev+1, returns nowMs (clock wins — time-ordered).
        val nowMs = 9999L
        val prev = 5L
        assertEquals(9999L, ClipboardRepository.nextLamportTs(prev, nowMs))
    }

    @Test
    fun nextLamportTs_prevZero_returnsMax1AndNowMs() {
        // Baseline: prev=0, nowMs=0 → returns max(1, 0) = 1.
        assertEquals(1L, ClipboardRepository.nextLamportTs(0L, 0L))
    }

    @Test
    fun nextLamportTs_alwaysStrictlyGreaterThanPrev() {
        val prev = 100L
        val nowMs = 50L  // nowMs < prev+1, so result = prev+1 = 101
        val result = ClipboardRepository.nextLamportTs(prev, nowMs)
        assertTrue("result must be > prev", result > prev)
    }

    // ── CloudRow delete/pin parse (SupabaseClient.fetchRows via CloudRow) ─────

    @Test
    fun cloudRow_deleted_defaultsFalse() {
        // CloudRow with no deleted field defaults to false.
        val row = SupabaseClient.CloudRow(
            id = "r1",
            itemId = "i1",
            contentType = "text",
            payloadCtWire = "\\x68656c6c6f",
            lamportTs = 1L,
            wallTime = 0L,
            expiresAt = null,
            appBundleId = null,
            deviceId = "d1",
        )
        assertFalse(row.deleted)
        assertFalse(row.pinned)
        assertNull(row.pinOrder)
    }

    @Test
    fun decryptRow_deletedRow_returnsDecryptedItemWithDeletedTrue() {
        // A deleted CloudRow (no payload to decrypt) must produce a DecryptedItem
        // with deleted=true and empty plaintext — no decryption attempt.
        val row = SupabaseClient.CloudRow(
            id = "del-1",
            itemId = "item-del-1",
            contentType = "text",
            payloadCtWire = "",   // tombstone: empty payload
            lamportTs = 99L,
            wallTime = 0L,
            expiresAt = null,
            appBundleId = null,
            deviceId = "d2",
            deleted = true,
            pinned = false,
            pinOrder = null,
        )
        // decryptRow with a dummy syncKey — must NOT attempt decryption for deleted rows.
        val dummyKey = ByteArray(32) { it.toByte() }
        val item = SupabaseClient(
            supabaseUrl = "https://example.supabase.co",
            anonKey = "anon",
        ).decryptRow(row, dummyKey)!!
        assertTrue(item.deleted)
        assertEquals(0, item.plaintext.size)
        assertEquals("item-del-1", item.itemId)
        assertEquals(99L, item.lamportTs)
    }
}
