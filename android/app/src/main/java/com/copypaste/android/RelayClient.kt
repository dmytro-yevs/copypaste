package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

/**
 * HTTP client for the CopyPaste relay server.
 * Uses standard Java HttpURLConnection — no third-party HTTP lib needed.
 */
class RelayClient(private val baseUrl: String) {

    data class Device(val deviceId: String, val token: String)

    /**
     * Register (or co-register) this device's shared-account inbox with the
     * relay. Matches the daemon's contract (relay.rs): `POST /devices`
     * `{device_id, device_name, public_key_b64, pop_b64}` → 201 `{auth_token}`.
     *
     * [deviceId] MUST be the shared-account inbox id (`relayInboxId(syncKey)`),
     * NOT the per-install device id, so this device shares the daemon's inbox.
     * [publicKeyBase64] MUST be `relayPublicKeyB64(syncKey)`. [popB64] MUST be
     * the base64 of `relay_registration_pop(syncKey, deviceId)` — proves the
     * registrant holds the sync key (fixes CopyPaste-kmcr inbox-theft risk). A
     * fresh register always returns 201 with a new independent token, whether or
     * not the inbox was already co-registered by another device.
     *
     * SECURITY: never logs the inbox id, public key, PoP, or token.
     */
    suspend fun registerDevice(
        deviceId: String,
        publicKeyBase64: String,
        deviceName: String,
        popB64: String,
    ): Device? =
        withContext(Dispatchers.IO) {
            try {
                val body = buildRegisterBody(
                    deviceId = deviceId,
                    deviceName = deviceName,
                    publicKeyBase64 = publicKeyBase64,
                    popB64 = popB64,
                )

                val resp = post("/devices", body, null)
                if (resp.code in 200..201) {
                    val json = JSONObject(resp.body)
                    Device(
                        deviceId = json.optString("device_id", deviceId),
                        // Daemon contract: response field is `auth_token`. Accept
                        // the legacy `token` alias too for forward/backward safety.
                        token = json.optString("auth_token").takeIf { it.isNotBlank() }
                            ?: json.optString("token"),
                    ).takeIf { it.token.isNotBlank() }
                } else {
                    Log.w(TAG, "registerDevice: HTTP ${resp.code}")
                    null
                }
            } catch (e: Exception) {
                // L11: log relay errors so outages are diagnosable (was a silent swallow).
                Log.w(TAG, "registerDevice failed: ${e.javaClass.simpleName}: ${e.message}", e)
                null
            }
        }

    /**
     * Push one already-built envelope to the shared inbox. Matches the daemon's
     * push contract (relay.rs): `POST /devices/{deviceId}/items`
     * `{content_type, content_b64, wall_time}` with `Authorization: Bearer`.
     *
     * [deviceId] is the shared inbox id; [contentB64] is the base64-std envelope
     * (`base64(JSON{item_id, lamport_ts, ct_b64})`) the receiver decodes. The
     * item_id lives INSIDE the envelope — the relay never sees it.
     *
     * @return 401 if the token was rejected (caller should re-register and
     *   retry), otherwise true on 2xx / false on any other failure. The 401 is
     *   distinguished via [PushResult] so the caller can re-register exactly once.
     */
    suspend fun pushEnvelope(
        deviceId: String,
        token: String,
        contentType: String,
        contentB64: String,
        wallTime: Long,
    ): PushResult =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("content_type", contentType)
                    put("content_b64", contentB64)
                    put("wall_time", wallTime)
                }.toString()

                val resp = post("/devices/$deviceId/items", body, token)
                when {
                    resp.code in 200..201 -> PushResult.OK
                    resp.code == 401 -> PushResult.UNAUTHORIZED
                    else -> {
                        Log.w(TAG, "pushEnvelope: HTTP ${resp.code}")
                        PushResult.FAILED
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "pushEnvelope failed: ${e.javaClass.simpleName}: ${e.message}", e)
                PushResult.FAILED
            }
        }

    /** Outcome of [pushEnvelope]; `UNAUTHORIZED` signals a stale token (re-register). */
    enum class PushResult { OK, UNAUTHORIZED, FAILED }

    /**
     * One parsed SSE `item` frame from `GET /devices/{id}/subscribe`.
     *
     * Mirrors one element of the relay pull JSON array (the relay never decrypts):
     *   - [id]          — per-device ascending inbox id (the SSE `id:` line; the
     *                     `(wall_time, id)` cursor companion for reconnect resume)
     *   - [contentType] — "text" / "image" / "file"
     *   - [contentB64]  — opaque ciphertext, base64-standard (decrypted downstream)
     *   - [wallTime]    — sender wall-clock time (Unix epoch ms)
     */
    data class SseItem(
        val id: Long,
        val contentType: String,
        val contentB64: String,
        val wallTime: Long,
    )

    /**
     * Open the relay SSE stream and deliver each `item` event to [onItem] until
     * the stream ends, the connection drops, or [shouldContinue] returns false.
     *
     * Wire contract (issue #26, server side shipped):
     *   - `GET /devices/{deviceId}/subscribe?since=<wallTime>&since_id=<sinceId>`
     *   - `Authorization: Bearer <token>` (same token as the poll route; 401 on miss)
     *   - On connect: backfills inbox items past the cursor, then streams new ones.
     *   - Framing: `event: item` / `id: <inbox id>` /
     *     `data: {"id","content_type","content_b64","wall_time"}`.
     *   - `:`-prefixed lines are keepalive comments (25 s) and are ignored.
     *
     * This is a MANUAL buffered line-reader over a streaming OkHttp GET — the
     * `okhttp-sse` EventSource artifact is not on the classpath (only base
     * `okhttp` is), and the plan explicitly permits a hand-rolled reader. The
     * call blocks on [Dispatchers.IO] for the lifetime of the stream; the caller
     * owns reconnect/backoff (see [RelaySubscriptionClient]).
     *
     * [readTimeoutMs] bounds a silent socket: the relay sends a keepalive comment
     * every 25 s, so a read gap longer than this (default 60 s) means the
     * connection is dead and we return to let the caller reconnect.
     *
     * @return the HTTP status code observed (200 on a clean stream that ended;
     *   401 on auth failure so the caller can re-register; -1 on a transport
     *   exception). Never throws.
     */
    suspend fun subscribe(
        deviceId: String,
        token: String,
        sinceWallTime: Long,
        sinceId: Long,
        readTimeoutMs: Long = 60_000L,
        shouldContinue: () -> Boolean,
        onItem: suspend (SseItem) -> Unit,
    ): Int = withContext(Dispatchers.IO) {
        val url = buildString {
            append(baseUrl)
            append("/devices/")
            append(deviceId)
            append("/subscribe?since=")
            append(sinceWallTime)
            if (sinceId > 0) {
                append("&since_id=")
                append(sinceId)
            }
        }
        val client = OkHttpClient.Builder()
            .connectTimeout(10, TimeUnit.SECONDS)
            // Per-read timeout: the 25 s keepalive comment resets it; a longer
            // gap means a dead socket so the read throws and we reconnect.
            .readTimeout(readTimeoutMs, TimeUnit.MILLISECONDS)
            .build()
        val request = Request.Builder()
            .url(url)
            .header("Authorization", "Bearer $token")
            .header("Accept", "text/event-stream")
            .build()

        var statusCode = -1
        try {
            client.newCall(request).execute().use { response ->
                statusCode = response.code
                if (!response.isSuccessful) {
                    Log.w(TAG, "subscribe: HTTP ${response.code} for device=$deviceId")
                    return@use
                }
                val body = response.body ?: run {
                    Log.w(TAG, "subscribe: empty body")
                    return@use
                }
                val reader = body.source()
                // SSE frame accumulator: collect event/id/data lines until a blank
                // line dispatches the event. Mirrors the W3C SSE line protocol.
                var evType: String? = null
                var evId: Long = 0
                var evData: String? = null
                while (shouldContinue()) {
                    // readUtf8Line blocks until a line or EOF; the readTimeout
                    // above turns a stalled socket into a SocketTimeoutException.
                    val line = reader.readUtf8Line() ?: break // EOF → stream ended
                    when {
                        line.isEmpty() -> {
                            // Blank line: dispatch the accumulated frame.
                            val data = evData
                            if (evType == "item" && data != null) {
                                val parsed = parseSseItem(evId, data)
                                if (parsed != null) onItem(parsed)
                            }
                            evType = null
                            evId = 0
                            evData = null
                        }
                        line.startsWith(":") -> {
                            // Keepalive / comment frame — ignore.
                        }
                        line.startsWith("event:") -> evType = line.substring(6).trim()
                        line.startsWith("id:") -> evId = line.substring(3).trim().toLongOrNull() ?: evId
                        line.startsWith("data:") -> {
                            val chunk = line.substring(5).trim()
                            // Multi-line data fields concatenate with '\n'; relay
                            // emits a single line but handle the general case.
                            evData = if (evData == null) chunk else "$evData\n$chunk"
                        }
                        // Unknown field (retry:, etc.) — ignore.
                    }
                }
            }
        } catch (e: Exception) {
            // Transport error / read timeout / cancellation-driven close: log and
            // let the caller reconnect. Never surface ciphertext or tokens.
            Log.w(TAG, "subscribe stream ended: ${e.javaClass.simpleName}: ${e.message}")
        }
        statusCode
    }

    /**
     * Parse one SSE `data:` JSON payload into an [SseItem]. The `id:` SSE line is
     * authoritative for the inbox id (passed as [eventId]); the `data.id` field is
     * the same value and used as a fallback. Returns null on malformed JSON.
     */
    private fun parseSseItem(eventId: Long, data: String): SseItem? {
        return try {
            val json = JSONObject(data)
            val id = if (eventId > 0) eventId else json.optLong("id", 0L)
            if (id <= 0) return null
            val contentB64 = json.optString("content_b64").takeIf { it.isNotBlank() } ?: return null
            SseItem(
                id = id,
                contentType = json.optString("content_type", "text"),
                contentB64 = contentB64,
                wallTime = json.optLong("wall_time", 0L),
            )
        } catch (e: Exception) {
            Log.w(TAG, "subscribe: malformed SSE data frame: ${e.message}")
            null
        }
    }

    /**
     * Catch-up backstop poll over the SSE wire contract: `GET
     * /devices/{deviceId}/items?since=<wallTime>&since_id=<sinceId>`. Returns the
     * inbox items past the `(wall_time, id)` cursor as [SseItem]s — the SAME shape
     * the SSE stream delivers — so the caller can ingest them through the same
     * path. The response is a bare JSON array of `{id, content_type, content_b64,
     * wall_time}`. Returns empty on any error.
     */
    suspend fun pollSseBacklog(
        deviceId: String,
        token: String,
        sinceWallTime: Long,
        sinceId: Long,
    ): List<SseItem> = withContext(Dispatchers.IO) {
        try {
            val path = buildString {
                append("/devices/")
                append(deviceId)
                append("/items?since=")
                append(sinceWallTime)
                if (sinceId > 0) {
                    append("&since_id=")
                    append(sinceId)
                }
            }
            val resp = get(path, token)
            if (resp.code != 200) {
                Log.w(TAG, "pollSseBacklog: HTTP ${resp.code}")
                return@withContext emptyList()
            }
            val arr = org.json.JSONArray(resp.body)
            (0 until arr.length()).mapNotNull { i ->
                val o = arr.getJSONObject(i)
                val id = o.optLong("id", 0L)
                val contentB64 = o.optString("content_b64").takeIf { it.isNotBlank() }
                if (id <= 0 || contentB64 == null) return@mapNotNull null
                SseItem(
                    id = id,
                    contentType = o.optString("content_type", "text"),
                    contentB64 = contentB64,
                    wallTime = o.optLong("wall_time", 0L),
                )
            }
        } catch (e: Exception) {
            Log.w(TAG, "pollSseBacklog failed: ${e.javaClass.simpleName}: ${e.message}", e)
            emptyList()
        }
    }

    suspend fun health(): Boolean = withContext(Dispatchers.IO) {
        try {
            get("/health", null).code == 200
        } catch (e: Exception) {
            Log.w(TAG, "health check failed: ${e.javaClass.simpleName}: ${e.message}", e)
            false
        }
    }

    // --- HTTP helpers ---

    private data class HttpResponse(val code: Int, val body: String)

    private fun get(path: String, token: String?): HttpResponse {
        val url = URL("$baseUrl$path")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "GET"
        token?.let { conn.setRequestProperty("Authorization", "Bearer $it") }
        conn.connectTimeout = 10_000
        conn.readTimeout = 10_000
        return readResponse(conn)
    }

    private fun post(path: String, body: String, token: String?): HttpResponse {
        val url = URL("$baseUrl$path")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "POST"
        conn.doOutput = true
        conn.setRequestProperty("Content-Type", "application/json")
        token?.let { conn.setRequestProperty("Authorization", "Bearer $it") }
        conn.connectTimeout = 10_000
        conn.readTimeout = 10_000
        // Explicit Charsets.UTF_8: OutputStreamWriter(stream) uses the platform
        // default charset which varies by device/locale — always write UTF-8.
        OutputStreamWriter(conn.outputStream, Charsets.UTF_8).use { it.write(body) }
        return readResponse(conn)
    }

    private fun readResponse(conn: HttpURLConnection): HttpResponse {
        return try {
            val code = conn.responseCode
            // errorStream can be null (e.g. no response body on some error codes or
            // when the connection was dropped). Fall back to inputStream to avoid NPE.
            val stream = if (code in 200..299) conn.inputStream
                         else (conn.errorStream ?: conn.inputStream)
            val body = BufferedReader(InputStreamReader(stream, Charsets.UTF_8)).use { it.readText() }
            HttpResponse(code, body)
        } finally {
            conn.disconnect()
        }
    }

    companion object {
        private const val TAG = "RelayClient"

        /** Derive bearer token from public key bytes (first 32 hex chars of SHA-256) */
        fun tokenFromPublicKey(publicKeyBytes: ByteArray): String {
            val digest = MessageDigest.getInstance("SHA-256").digest(publicKeyBytes)
            return digest.joinToString("") { "%02x".format(it) }.take(32)
        }

        /**
         * Build the JSON request body for `POST /devices`, including the
         * proof-of-possession field (`pop_b64`) required by the relay since
         * CopyPaste-kmcr. Extracted as a pure function so it can be unit-tested
         * without a running relay server.
         *
         * SECURITY: [popB64] and [publicKeyBase64] are secret-derived — callers
         * MUST NOT log the returned string.
         */
        fun buildRegisterBody(
            deviceId: String,
            deviceName: String,
            publicKeyBase64: String,
            popB64: String,
        ): String = JSONObject().apply {
            put("device_id", deviceId)
            put("device_name", deviceName)
            put("public_key_b64", publicKeyBase64)
            // CopyPaste-kmcr: PoP proves the registrant holds the sync key that
            // deterministically maps to this inbox id. Relay enforces this field.
            // SECURITY: never log this field or its value.
            put("pop_b64", popB64)
        }.toString()
    }
}
