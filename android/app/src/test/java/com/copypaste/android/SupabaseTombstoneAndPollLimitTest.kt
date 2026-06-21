package com.copypaste.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM regression tests for CopyPaste-gh1h:
 *   1. pushMutationRow tombstone MUST set payload_ct = null explicitly so the
 *      Supabase row's ciphertext is wiped on delete (mirrors daemon mark_deleted).
 *   2. POLL_LIMIT must be raised from 20 to 100 so a full tombstone/recovery
 *      catch-up drains in at most ceil(N/100) polls instead of ceil(N/20).
 *
 * These are pure-JVM tests (no Android SDK, no coroutines, no HTTP) that verify
 * the constants and JSON shape produced by [SupabaseClient.buildTombstonePatchBody].
 *
 * Run with: ./gradlew :app:testDebugUnitTest
 */
class SupabaseTombstoneAndPollLimitTest {

    // ── POLL_LIMIT ────────────────────────────────────────────────────────────

    /**
     * POLL_LIMIT was 20 — matching the daemon's original limit=20.  After gh1h the
     * Android client raises it to 100 so a recovery catch-up requires at most
     * ceil(N/100) polls rather than ceil(N/20), converging 5× faster.
     */
    @Test
    fun `POLL_LIMIT is at least 100 for fast catch-up recovery`() {
        assertTrue(
            "POLL_LIMIT must be >= 100 so catch-up polls converge quickly (was 20, CopyPaste-gh1h)",
            SupabaseClient.POLL_LIMIT >= 100,
        )
    }

    @Test
    fun `POLL_LIMIT is exactly 100`() {
        assertEquals(
            "POLL_LIMIT must be exactly 100 (raised from 20 per CopyPaste-gh1h)",
            100,
            SupabaseClient.POLL_LIMIT,
        )
    }

    // ── Tombstone patch body: payload_ct must be explicit null ─────────────────

    /**
     * The tombstone PATCH body MUST include an explicit null for payload_ct so
     * Supabase (PostgREST) writes NULL into the column, wiping the ciphertext.
     * Without this, a PATCH that omits payload_ct leaves the old ciphertext in
     * place — it remains decryptable until the server-side TTL evicts the row,
     * creating an information-disclosure window.
     *
     * Mirrors the macOS daemon's cloud.rs mark_deleted which explicitly nulls the
     * column: `UPDATE clipboard_items SET deleted=true, payload_ct=NULL WHERE …`.
     */
    @Test
    fun `buildTombstonePatchBody includes payload_ct as null`() {
        val body = SupabaseClient.buildTombstonePatchBody(
            lamportTs = 42L,
            wallTime = 1_000L,
        )
        val json = JSONObject(body)
        // payload_ct must be present and explicitly JSONObject.NULL
        assertTrue(
            "tombstone patch body must contain a 'payload_ct' key to wipe the column",
            json.has("payload_ct"),
        )
        assertTrue(
            "tombstone patch body 'payload_ct' must be JSON null to wipe the ciphertext",
            json.isNull("payload_ct"),
        )
    }

    @Test
    fun `buildTombstonePatchBody sets deleted=true`() {
        val body = SupabaseClient.buildTombstonePatchBody(lamportTs = 1L, wallTime = 2L)
        val json = JSONObject(body)
        assertTrue("deleted must be true in tombstone body", json.getBoolean("deleted"))
    }

    @Test
    fun `buildTombstonePatchBody sets pinned=false`() {
        val body = SupabaseClient.buildTombstonePatchBody(lamportTs = 1L, wallTime = 2L)
        val json = JSONObject(body)
        assertTrue("pinned must be false in tombstone body", !json.getBoolean("pinned"))
    }

    @Test
    fun `buildTombstonePatchBody sets pin_order to null`() {
        val body = SupabaseClient.buildTombstonePatchBody(lamportTs = 10L, wallTime = 20L)
        val json = JSONObject(body)
        assertTrue("pin_order must be null in tombstone body", json.isNull("pin_order"))
    }

    @Test
    fun `buildTombstonePatchBody encodes lamport_ts`() {
        val body = SupabaseClient.buildTombstonePatchBody(lamportTs = 777L, wallTime = 888L)
        val json = JSONObject(body)
        assertEquals(777L, json.getLong("lamport_ts"))
    }

    @Test
    fun `buildTombstonePatchBody encodes wall_time`() {
        val body = SupabaseClient.buildTombstonePatchBody(lamportTs = 1L, wallTime = 12345L)
        val json = JSONObject(body)
        assertEquals(12345L, json.getLong("wall_time"))
    }

    // ── Pin-state patch: payload_ct must NOT be touched ───────────────────────

    /**
     * A pin-state update PATCH must NOT include payload_ct — it should only update
     * pinned/pin_order/lamport_ts/wall_time and leave the ciphertext untouched.
     */
    @Test
    fun `buildPinStatePatchBody does NOT include payload_ct`() {
        val body = SupabaseClient.buildPinStatePatchBody(
            lamportTs = 5L,
            wallTime = 100L,
            pinned = true,
            pinOrder = 1.0,
        )
        val json = JSONObject(body)
        assertTrue(
            "pin-state patch must NOT touch payload_ct (leave ciphertext intact)",
            !json.has("payload_ct"),
        )
    }

    @Test
    fun `buildPinStatePatchBody sets deleted=false`() {
        val body = SupabaseClient.buildPinStatePatchBody(
            lamportTs = 5L, wallTime = 100L, pinned = true, pinOrder = 1.0,
        )
        val json = JSONObject(body)
        assertTrue("pin-state patch must set deleted=false", !json.getBoolean("deleted"))
    }
}
