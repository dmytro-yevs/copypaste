package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

/**
 * HTTP client for the CopyPaste relay server.
 * Uses standard Java HttpURLConnection — no third-party HTTP lib needed.
 */
class RelayClient(private val baseUrl: String) {

    data class Device(val deviceId: String, val token: String)

    data class RelayItem(
        val itemId: String,
        val ciphertext: String,  // base64
        val nonce: String,        // base64
        val senderDeviceId: String,
        val contentType: String,
        val lamportTs: Long
    )

    suspend fun registerDevice(deviceId: String, publicKeyBase64: String): Device? =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("device_id", deviceId)
                    put("public_key", publicKeyBase64)
                }.toString()

                val resp = post("/devices", body, null)
                if (resp.code == 200) {
                    val json = JSONObject(resp.body)
                    Device(
                        deviceId = json.getString("device_id"),
                        token = json.getString("token")
                    )
                } else null
            } catch (e: Exception) {
                // L11: log relay errors so outages are diagnosable (was a silent swallow).
                Log.w(TAG, "registerDevice failed: ${e.javaClass.simpleName}: ${e.message}", e)
                null
            }
        }

    suspend fun uploadItem(deviceId: String, token: String, item: RelayItem): Boolean =
        withContext(Dispatchers.IO) {
            try {
                val body = JSONObject().apply {
                    put("item_id", item.itemId)
                    put("ciphertext", item.ciphertext)
                    put("nonce", item.nonce)
                    put("sender_device_id", item.senderDeviceId)
                    put("content_type", item.contentType)
                    put("lamport_ts", item.lamportTs)
                }.toString()

                val resp = post("/devices/$deviceId/items", body, token)
                resp.code in 200..201
            } catch (e: Exception) {
                Log.w(TAG, "uploadItem failed: ${e.javaClass.simpleName}: ${e.message}", e)
                false
            }
        }

    suspend fun pollItems(deviceId: String, token: String, sinceLamport: Long = 0): List<RelayItem> =
        withContext(Dispatchers.IO) {
            try {
                val resp = get("/devices/$deviceId/items?since_lamport=$sinceLamport", token)
                if (resp.code != 200) return@withContext emptyList()

                val json = JSONObject(resp.body)
                val arr = json.getJSONArray("items")
                (0 until arr.length()).map { i ->
                    val item = arr.getJSONObject(i)
                    RelayItem(
                        itemId = item.getString("item_id"),
                        ciphertext = item.getString("ciphertext"),
                        nonce = item.getString("nonce"),
                        senderDeviceId = item.getString("sender_device_id"),
                        contentType = item.getString("content_type"),
                        lamportTs = item.getLong("lamport_ts")
                    )
                }
            } catch (e: Exception) {
                Log.w(TAG, "pollItems failed: ${e.javaClass.simpleName}: ${e.message}", e)
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
        OutputStreamWriter(conn.outputStream).use { it.write(body) }
        return readResponse(conn)
    }

    private fun readResponse(conn: HttpURLConnection): HttpResponse {
        val code = conn.responseCode
        val stream = if (code in 200..299) conn.inputStream else conn.errorStream
        val body = BufferedReader(InputStreamReader(stream)).use { it.readText() }
        return HttpResponse(code, body)
    }

    companion object {
        private const val TAG = "RelayClient"

        /** Derive bearer token from public key bytes (first 32 hex chars of SHA-256) */
        fun tokenFromPublicKey(publicKeyBytes: ByteArray): String {
            val digest = MessageDigest.getInstance("SHA-256").digest(publicKeyBytes)
            return digest.joinToString("") { "%02x".format(it) }.take(32)
        }
    }
}
