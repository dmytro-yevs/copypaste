package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL

/**
 * PostgREST + GoTrue client for Supabase clipboard sync.
 *
 * Speaks the SAME wire protocol as the macOS daemon's cloud.rs:
 *   - Table   : `clipboard_items` (PostgREST `/rest/v1/clipboard_items`)
 *   - Column  : `payload_ct`  — Postgres `bytea` holding nonce[24] ||
 *               XChaCha20-Poly1305 ciphertext, sent/received as the hex-input
 *               literal `\x<hex>` (see [encodePayloadCt]/[decodePayloadCt])
 *   - AAD     : "{item_id}|5"  (CLOUD_AAD_SCHEMA_VERSION = 5)
 *   - Auth    : `apikey` header + `Authorization: Bearer <token>` header
 *
 * Encryption/decryption is delegated to the native Rust FFI ([cloud_encrypt],
 * [cloud_decrypt], [derive_cloud_sync_key]) which use identical Argon2id KDF
 * parameters and XChaCha20-Poly1305 AEAD as the daemon. This guarantees that
 * a ciphertext produced on Android can be decrypted on macOS and vice-versa.
 *
 * Usage:
 * ```
 * val client = SupabaseClient("https://xyz.supabase.co", anonKey = "...")
 * val token  = client.signIn("user@example.com", "password") ?: anonKey
 * val syncKey = derive_cloud_sync_key("shared passphrase")
 * client.push(token, syncKey, item)
 * val rows   = client.poll(token, syncKey, sinceWallTime = 0L)
 * ```
 */
class SupabaseClient(
    private val supabaseUrl: String,
    private val anonKey: String,
) {
    companion object {
        private const val TAG = "SupabaseClient"

        /**
         * Maximum rows to fetch in a single poll (matches daemon's `limit=20`).
         *
         * Public so callers can drain a backlog: a returned batch whose size
         * equals POLL_LIMIT means the server very likely has more rows waiting,
         * so the caller re-polls immediately instead of waiting the idle delay.
         */
        const val POLL_LIMIT = 20

        /** Connect / read timeout for every HTTP call, ms. */
        private const val TIMEOUT_MS = 15_000

        /** Lowercase hex digits for [encodePayloadCt]. */
        private val HEX_DIGITS = "0123456789abcdef".toCharArray()

        /**
         * Encode a raw cloud ciphertext blob (nonce[24] || ciphertext_with_tag)
         * as a Postgres `bytea` hex-input literal `\x<hex>` (lowercase).
         *
         * `payload_ct` is a Postgres `bytea` column. PostgREST stores a string
         * assigned to it via Postgres' INPUT formats — a bare base64 string is
         * NOT one of them (it is stored as the literal ASCII bytes of the base64
         * text), so the daemon could never decode it. The canonical hex input
         * form `\x<hex>` makes the column hold the true ciphertext bytes and the
         * read-back round-trips. Mirrors `encode_payload_ct_hex` in cloud.rs.
         */
        @JvmStatic
        fun encodePayloadCt(blob: ByteArray): String {
            val sb = StringBuilder(2 + blob.size * 2)
            sb.append("\\x")
            for (b in blob) {
                val v = b.toInt() and 0xFF
                sb.append(HEX_DIGITS[v ushr 4])
                sb.append(HEX_DIGITS[v and 0x0F])
            }
            return sb.toString()
        }

        /**
         * Decode a `payload_ct` value as returned by PostgREST into the raw
         * ciphertext blob. PostgREST renders `bytea` in hex output form
         * (`\x<hex>`); we also accept a bare base64 string for backward
         * compatibility with rows written by the pre-fix Android/daemon (where
         * the base64 text was stored verbatim). Mirrors `decode_payload_ct`.
         *
         * @throws IllegalArgumentException on malformed hex or base64.
         */
        @JvmStatic
        fun decodePayloadCt(payloadCt: String): ByteArray {
            if (payloadCt.startsWith("\\x")) {
                val hex = payloadCt.substring(2)
                require(hex.length % 2 == 0) { "odd-length hex in payload_ct" }
                val out = ByteArray(hex.length / 2)
                var i = 0
                while (i < hex.length) {
                    val hi = Character.digit(hex[i], 16)
                    val lo = Character.digit(hex[i + 1], 16)
                    require(hi >= 0 && lo >= 0) { "invalid hex digit in payload_ct" }
                    out[i / 2] = ((hi shl 4) or lo).toByte()
                    i += 2
                }
                return out
            }
            // Back-compat: bare base64 (pre-fix rows). java.util.Base64 is pure
            // JVM (available in unit tests); the std alphabet matches NO_WRAP.
            return java.util.Base64.getDecoder().decode(payloadCt)
        }
    }

    // ── Data types ──────────────────────────────────────────────────────────

    /**
     * A clipboard row as returned by the PostgREST GET query.
     * Field names match the `clipboard_items` table schema.
     */
    data class CloudRow(
        val id: String,
        val itemId: String,
        val contentType: String,
        /**
         * Raw `payload_ct` wire string as returned by PostgREST. For a `bytea`
         * column this is the hex-output form `\x<hex>`; legacy rows may instead
         * hold a bare base64 string. Decode via [decodePayloadCt].
         */
        val payloadCtWire: String,
        val lamportTs: Long,
        val wallTime: Long,
        val expiresAt: Long?,
        val appBundleId: String?,
        val deviceId: String,
    )

    /**
     * A decrypted clipboard item ready to store locally.
     */
    data class DecryptedItem(
        val id: String,
        val itemId: String,
        val contentType: String,
        val plaintext: ByteArray,
        val lamportTs: Long,
        val wallTime: Long,
        val expiresAt: Long?,
        val appBundleId: String?,
        val deviceId: String,
    )

    // ── Auth ─────────────────────────────────────────────────────────────────

    /**
     * Sign in with email + password via GoTrue.
     * Returns the `access_token` JWT string on success, or `null` on failure.
     * On failure, callers should fall back to using [anonKey] as the bearer.
     *
     * This mirrors `sign_in_with_password` in the macOS daemon's cloud.rs —
     * same GoTrue endpoint (`POST /auth/v1/token?grant_type=password`), same
     * `apikey` header, same JSON body shape.
     */
    suspend fun signIn(email: String, password: String): String? =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("email", email)
                    put("password", password)
                }.toString()
                val resp = post("/auth/v1/token?grant_type=password", body, bearerToken = null)
                if (resp.code in 200..299) {
                    JSONObject(resp.body).optString("access_token").takeIf { it.isNotBlank() }
                } else {
                    Log.w(TAG, "signIn failed (${resp.code}): ${resp.body.take(200)}")
                    null
                }
            } catch (e: Exception) {
                Log.w(TAG, "signIn exception: ${e.message}")
                null
            }
        }

    // ── Push ─────────────────────────────────────────────────────────────────

    /**
     * Encrypt [plaintext] with the cross-device [syncKeyBytes] and push it to
     * Supabase as a new `clipboard_items` row.
     *
     * [itemId] MUST be pre-generated by the caller (use `UUID.randomUUID()`)
     * and bound into the AEAD AAD at encrypt time. The same value is stored in
     * the `item_id` column so the receiver can rebuild the AAD for decryption.
     *
     * Returns `true` on 2xx, `false` on any error (network or server-side).
     * Callers should retry failed pushes independently (e.g. a work queue).
     *
     * Column mapping matches `clipboard_item_to_json` in the macOS cloud.rs:
     *   id, item_id, content_type, payload_ct (base64), lamport_ts, wall_time,
     *   expires_at, app_bundle_id, device_id.
     */
    suspend fun push(
        bearerToken: String,
        syncKeyBytes: ByteArray,
        id: String,
        itemId: String,
        plaintext: ByteArray,
        contentType: String = "text",
        lamportTs: Long = System.currentTimeMillis(),
        wallTime: Long = System.currentTimeMillis(),
        expiresAt: Long? = null,
        appBundleId: String? = null,
        deviceId: String = "",
    ): Boolean = withContext(Dispatchers.IO) {
        try {
            // Encrypt with the cross-device sync key (XChaCha20-Poly1305, AAD = itemId|5).
            val blob = cloud_encrypt(itemId, plaintext, syncKeyBytes)
            // `payload_ct` is a Postgres `bytea` column — send the hex-input
            // literal `\x<hex>` (NOT bare base64, which Postgres would store as
            // the ASCII of the base64 text). Mirrors daemon `encode_payload_ct_hex`.
            val payloadCtHex = encodePayloadCt(blob)

            val body = JSONObject().apply {
                put("id", id)
                put("item_id", itemId)
                put("content_type", contentType)
                put("payload_ct", payloadCtHex)
                put("lamport_ts", lamportTs)
                put("wall_time", wallTime)
                if (expiresAt != null) put("expires_at", expiresAt) else put("expires_at", JSONObject.NULL)
                if (appBundleId != null) put("app_bundle_id", appBundleId) else put("app_bundle_id", JSONObject.NULL)
                put("device_id", deviceId)
            }.toString()

            val resp = post(
                "/rest/v1/clipboard_items",
                body,
                bearerToken = bearerToken,
                extraHeaders = mapOf("Prefer" to "return=minimal"),
            )
            val ok = resp.code in 200..299
            if (!ok) Log.w(TAG, "push failed (${resp.code}): ${resp.body.take(200)}")
            ok
        } catch (e: Exception) {
            Log.w(TAG, "push exception: ${e.message}")
            false
        }
    }

    // ── Poll ─────────────────────────────────────────────────────────────────

    /**
     * Poll for rows since the compound keyset cursor ([sinceWallTime], [sinceId])
     * and decrypt them.
     *
     * Uses an ascending compound keyset cursor (order=wall_time.asc,id.asc) that
     * mirrors the macOS daemon's `build_poll_url`. The PostgREST filter is:
     *   or=(wall_time.gt.W,and(wall_time.eq.W,id.gt.ID))
     * This correctly handles >POLL_LIMIT rows sharing the same wall_time — no rows
     * are skipped between polls regardless of burst size.
     *
     * Returns all raw rows (including own-device rows and blank rows) so the
     * caller can advance the cursor for every row before applying filters.
     * Rows that fail to decrypt are returned as `null` plaintext (caller skips).
     *
     * Callers MUST advance [sinceWallTime]/[sinceId] for EVERY row in the
     * result — including self-echo and blank rows — before applying `continue`.
     */
    suspend fun poll(
        bearerToken: String,
        syncKeyBytes: ByteArray,
        sinceWallTime: Long = 0L,
        sinceId: String = "",
    ): List<DecryptedItem> = withContext(Dispatchers.IO) {
        try {
            val rows = fetchRows(bearerToken, sinceWallTime, sinceId)
            rows.mapNotNull { row -> decryptRow(row, syncKeyBytes) }
        } catch (e: Exception) {
            Log.w(TAG, "poll exception: ${e.message}")
            emptyList()
        }
    }

    /**
     * Fetch raw rows from PostgREST using the ascending compound keyset cursor.
     *
     * Query mirrors `build_poll_url` in the macOS daemon's cloud.rs exactly —
     * three-way branch on (sinceWallTime, sinceId):
     *   (a) wall==0 && id blank  → no filter (full table scan from the start)
     *   (b) wall>0  && id blank  → wall_time=gte.W  (inclusive, re-offers
     *       boundary-ms rows; per-row item_id dedup drops already-ingested ones)
     *   (c) wall>0  && id present → strict compound keyset:
     *       or=(wall_time.gt.W,and(wall_time.eq.W,id.gt.ID))
     *   order=wall_time.asc,id.asc
     *   limit=POLL_LIMIT
     *
     * Returns rows in ascending wall_time order so the caller can advance the
     * cursor by iterating front-to-back. Returns empty list on any error.
     */
    private fun fetchRows(
        bearerToken: String,
        sinceWallTime: Long,
        sinceId: String,
    ): List<CloudRow> {
        val path = buildString {
            append("/rest/v1/clipboard_items")
            append("?select=id,item_id,content_type,payload_ct,lamport_ts,wall_time,expires_at,app_bundle_id,device_id")
            // Ascending compound keyset: same order as daemon's build_poll_url.
            append("&order=wall_time.asc,id.asc")
            append("&limit=$POLL_LIMIT")
            // Three-way branch mirroring build_poll_url in cloud.rs:
            //   (a) wall==0 → no filter (case handled by omitting the block)
            //   (b) wall>0, id blank → inclusive gte so boundary-ms rows are
            //       re-offered; dedup by item_id drops already-ingested ones.
            //   (c) wall>0, id present → strict (wall,id) compound keyset.
            if (sinceWallTime > 0) {
                if (sinceId.isBlank()) {
                    // Case (b): cold-start with a persisted wall-only watermark.
                    append("&wall_time=gte.$sinceWallTime")
                } else {
                    // Case (c): full keyset — a later ms, OR same ms with larger id.
                    append("&or=(wall_time.gt.$sinceWallTime,and(wall_time.eq.$sinceWallTime,id.gt.$sinceId))")
                }
            }
        }
        val resp = get(path, bearerToken)
        if (resp.code !in 200..299) {
            Log.w(TAG, "fetchRows failed (${resp.code}): ${resp.body.take(200)}")
            return emptyList()
        }
        return try {
            val arr = JSONArray(resp.body)
            (0 until arr.length()).mapNotNull { i ->
                val obj = arr.getJSONObject(i)
                val id = obj.optString("id").takeIf { it.isNotBlank() } ?: return@mapNotNull null
                val itemId = obj.optString("item_id").takeIf { it.isNotBlank() } ?: return@mapNotNull null
                val payloadCt = obj.optString("payload_ct").takeIf { it.isNotBlank() } ?: return@mapNotNull null
                CloudRow(
                    id = id,
                    itemId = itemId,
                    contentType = obj.optString("content_type", "text"),
                    payloadCtWire = payloadCt,
                    lamportTs = obj.optLong("lamport_ts", 0L),
                    wallTime = obj.optLong("wall_time", 0L),
                    expiresAt = if (obj.isNull("expires_at")) null else obj.optLong("expires_at"),
                    appBundleId = if (obj.isNull("app_bundle_id")) null else obj.optString("app_bundle_id"),
                    deviceId = obj.optString("device_id", ""),
                )
            }
        } catch (e: Exception) {
            Log.w(TAG, "fetchRows parse error: ${e.message}")
            emptyList()
        }
    }

    /**
     * Fetch ALL raw rows for the cursor batch (including self-echo rows) so the
     * caller can advance the cursor for every row. Returns the full [CloudRow]
     * list; callers filter out own-device rows after advancing the cursor.
     *
     * Decrypts each row; returns `null` plaintext (and logs WARN) when
     * decryption fails — never surfaces partial plaintext or throws to caller.
     */
    suspend fun pollRaw(
        bearerToken: String,
        sinceWallTime: Long = 0L,
        sinceId: String = "",
    ): List<CloudRow> = withContext(Dispatchers.IO) {
        try {
            fetchRows(bearerToken, sinceWallTime, sinceId)
        } catch (e: Exception) {
            Log.w(TAG, "pollRaw exception: ${e.message}")
            emptyList()
        }
    }

    /**
     * Decrypt a single [CloudRow] using [syncKeyBytes].
     * Returns `null` (and logs a warning) if the blob is malformed or decryption
     * fails — never surfaces partial plaintext or throws to the caller.
     */
    fun decryptRow(row: CloudRow, syncKeyBytes: ByteArray): DecryptedItem? {
        // Only text items are supported today (mirrors macOS daemon behaviour).
        if (row.contentType != "text") {
            Log.d(TAG, "decryptRow: skipping non-text row id=${row.id} (${row.contentType})")
            return null
        }
        // `payload_ct` comes back as a `bytea` hex literal `\x<hex>` (or, for
        // legacy rows, bare base64). [decodePayloadCt] accepts both, mirroring
        // the daemon's `decode_payload_ct`.
        val blob = try {
            decodePayloadCt(row.payloadCtWire)
        } catch (e: Exception) {
            Log.w(TAG, "decryptRow: payload_ct decode failed for id=${row.id}: ${e.message}")
            return null
        }
        val plaintext = try {
            cloud_decrypt(row.itemId, blob, syncKeyBytes)
        } catch (e: Exception) {
            // Wrong passphrase, tampered blob, or wrong item_id AAD.
            Log.w(TAG, "decryptRow: cloud_decrypt failed for id=${row.id} — wrong key or tampered blob")
            return null
        }
        return DecryptedItem(
            id = row.id,
            itemId = row.itemId,
            contentType = row.contentType,
            plaintext = plaintext,
            lamportTs = row.lamportTs,
            wallTime = row.wallTime,
            expiresAt = row.expiresAt,
            appBundleId = row.appBundleId,
            deviceId = row.deviceId,
        )
    }

    // ── HTTP helpers ─────────────────────────────────────────────────────────

    private data class HttpResponse(val code: Int, val body: String)

    private fun get(path: String, bearerToken: String): HttpResponse {
        val url = URL("$supabaseUrl$path")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "GET"
        conn.setRequestProperty("apikey", anonKey)
        conn.setRequestProperty("Authorization", "Bearer $bearerToken")
        conn.connectTimeout = TIMEOUT_MS
        conn.readTimeout = TIMEOUT_MS
        return readResponse(conn)
    }

    private fun post(
        path: String,
        body: String,
        bearerToken: String?,
        extraHeaders: Map<String, String> = emptyMap(),
    ): HttpResponse {
        val url = URL("$supabaseUrl$path")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "POST"
        conn.doOutput = true
        conn.setRequestProperty("Content-Type", "application/json")
        conn.setRequestProperty("apikey", anonKey)
        if (bearerToken != null) {
            conn.setRequestProperty("Authorization", "Bearer $bearerToken")
        }
        extraHeaders.forEach { (k, v) -> conn.setRequestProperty(k, v) }
        conn.connectTimeout = TIMEOUT_MS
        conn.readTimeout = TIMEOUT_MS
        OutputStreamWriter(conn.outputStream, Charsets.UTF_8).use { it.write(body) }
        return readResponse(conn)
    }

    private fun readResponse(conn: HttpURLConnection): HttpResponse {
        val code = conn.responseCode
        val stream = if (code in 200..299) conn.inputStream else conn.errorStream ?: conn.inputStream
        val body = BufferedReader(InputStreamReader(stream, Charsets.UTF_8)).use { it.readText() }
        return HttpResponse(code, body)
    }
}
