package com.copypaste.android

import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for [SupabaseRealtimeClient] wire-protocol helpers.
 *
 * No OkHttp / Android runtime — only the companion-object parsing functions
 * that are safe to call on the host JVM via `./gradlew testDebugUnitTest`.
 */
class SupabaseRealtimeClientTest {

    // ── Wire format: 5-element JSON array ────────────────────────────────────

    @Test
    fun buildJoinFrame_produces5ElementArray() {
        val frame = SupabaseRealtimeClient.buildJoinFrame(
            joinRef = "1",
            ref = "2",
            topic = "realtime:clipboard_items",
            accessToken = "jwt-token",
            userUuid = "user-uuid-123",
        )
        val arr = JSONArray(frame)
        assertEquals("Phoenix frame must have 5 elements", 5, arr.length())
    }

    @Test
    fun buildJoinFrame_hasCorrectEventAndTopic() {
        val frame = SupabaseRealtimeClient.buildJoinFrame(
            joinRef = "1",
            ref = "2",
            topic = "realtime:clipboard_items",
            accessToken = "tok",
            userUuid = "uuid",
        )
        val arr = JSONArray(frame)
        assertEquals("1", arr.optString(0))      // join_ref
        assertEquals("2", arr.optString(1))      // ref
        assertEquals("realtime:clipboard_items", arr.optString(2)) // topic
        assertEquals("phx_join", arr.optString(3)) // event
    }

    @Test
    fun buildJoinFrame_embedsAccessTokenAndFilter() {
        val frame = SupabaseRealtimeClient.buildJoinFrame(
            joinRef = "1",
            ref = "2",
            topic = "realtime:clipboard_items",
            accessToken = "test-jwt",
            userUuid = "test-user",
        )
        val arr = JSONArray(frame)
        val payload = arr.getJSONObject(4)
        val config = payload.getJSONObject("config")
        assertEquals("test-jwt", config.getString("access_token"))
        val changes = config.getJSONArray("postgres_changes")
        assertEquals(1, changes.length())
        val filter = changes.getJSONObject(0).getString("filter")
        assertTrue("filter must reference user uuid", filter.contains("test-user"))
    }

    @Test
    fun buildHeartbeatFrame_nullJoinRef() {
        val frame = SupabaseRealtimeClient.buildHeartbeatFrame(ref = "42")
        val arr = JSONArray(frame)
        assertEquals(5, arr.length())
        assertTrue("join_ref must be null", arr.isNull(0))
        assertEquals("42", arr.optString(1))
        assertEquals("phoenix", arr.optString(2))
        assertEquals("heartbeat", arr.optString(3))
    }

    @Test
    fun buildLeaveFrame_produces5ElementArray() {
        val frame = SupabaseRealtimeClient.buildLeaveFrame(
            joinRef = "1",
            ref = "3",
            topic = "realtime:clipboard_items",
        )
        val arr = JSONArray(frame)
        assertEquals(5, arr.length())
        assertEquals("phx_leave", arr.optString(3))
    }

    // ── parseFrame: extracting event / payload ────────────────────────────────

    @Test
    fun parseFrame_extractsEventAndPayload() {
        val raw = """[null,"1","realtime:clipboard_items","phx_reply",{"status":"ok","response":{}}]"""
        val parsed = SupabaseRealtimeClient.parseFrame(raw)
        assertNotNull(parsed)
        assertEquals("phx_reply", parsed!!.event)
        assertEquals("realtime:clipboard_items", parsed.topic)
    }

    @Test
    fun parseFrame_nullJoinRef_isNull() {
        val raw = """[null,"1","realtime:clipboard_items","phx_reply",{"status":"ok","response":{}}]"""
        val parsed = SupabaseRealtimeClient.parseFrame(raw)!!
        assertNull(parsed.joinRef)
    }

    @Test
    fun parseFrame_postgresChanges_extractsRecord() {
        val raw = """["1","2","realtime:clipboard_items","postgres_changes",{"data":{"type":"INSERT","table":"clipboard_items","record":{"id":"abc","item_id":"item-1","content_type":"text","payload_ct":"\\x0102","lamport_ts":5,"wall_time":1000,"device_id":"dev-1"}}}]"""
        val parsed = SupabaseRealtimeClient.parseFrame(raw)!!
        assertEquals("postgres_changes", parsed.event)
        val record = SupabaseRealtimeClient.extractRecord(parsed)
        assertNotNull(record)
        assertEquals("abc", record!!.getString("id"))
        assertEquals("item-1", record.getString("item_id"))
        assertEquals("INSERT", SupabaseRealtimeClient.extractChangeType(parsed))
    }

    @Test
    fun parseFrame_returnsNullForMalformed() {
        assertNull(SupabaseRealtimeClient.parseFrame("{not an array}"))
        assertNull(SupabaseRealtimeClient.parseFrame("""["only","three"]"""))
    }

    // ── WS-aware poll intervals ───────────────────────────────────────────────

    @Test
    fun wsConnectedInterval_is120s() {
        assertEquals(120_000L, FgsSyncLoop.pollIntervalMs(wsConnected = true, consecutiveEmpty = 0))
    }

    @Test
    fun wsDownInterval_is60s() {
        assertEquals(60_000L, FgsSyncLoop.pollIntervalMs(wsConnected = false, consecutiveEmpty = 0))
    }

    @Test
    fun idleInterval_is300s_whenConnected() {
        val idle = FgsSyncLoop.pollIntervalMs(wsConnected = true, consecutiveEmpty = 100)
        assertEquals(300_000L, idle)
    }

    @Test
    fun idleInterval_is300s_whenDisconnected() {
        val idle = FgsSyncLoop.pollIntervalMs(wsConnected = false, consecutiveEmpty = 100)
        assertEquals(300_000L, idle)
    }

    // ── JWT sub (user UUID) extraction ────────────────────────────────────────

    @Test
    fun extractJwtSub_parsesSubClaim() {
        val payload = java.util.Base64.getUrlEncoder().withoutPadding()
            .encodeToString("""{"sub":"test-uuid","exp":9999999999}""".toByteArray())
        val token = "header.$payload.sig"
        val sub = SupabaseRealtimeClient.extractJwtSub(token)
        assertEquals("test-uuid", sub)
    }

    @Test
    fun extractJwtSub_returnsNullForMalformed() {
        assertNull(SupabaseRealtimeClient.extractJwtSub("not-a-jwt"))
        assertNull(SupabaseRealtimeClient.extractJwtSub(""))
    }

    // ── Reconnect backoff: 1s→60s exp ────────────────────────────────────────

    @Test
    fun wsReconnectBackoff_doublesAndClamps() {
        // reconnectDelayMs applies ±20% jitter (factor 0.8..1.2) for thundering-herd
        // avoidance, so pre-clamp values land in a window around the doubling
        // sequence, not on exact powers. Assert the jittered window; run several
        // iterations so an unlucky single draw can't mask a broken implementation.
        // The final clamp to BACKOFF_MAX_MS is exact (base*2^30 >> max).
        repeat(25) {
            assertTrue(SupabaseRealtimeClient.reconnectDelayMs(1) in 800L..1_200L)
            assertTrue(SupabaseRealtimeClient.reconnectDelayMs(2) in 1_600L..2_400L)
            assertTrue(SupabaseRealtimeClient.reconnectDelayMs(3) in 3_200L..4_800L)
            assertEquals(60_000L, SupabaseRealtimeClient.reconnectDelayMs(100))
        }
    }
}
