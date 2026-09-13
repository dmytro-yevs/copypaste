package com.copypaste.app

import android.content.Intent
import android.net.Uri
import org.json.JSONObject
import java.util.concurrent.atomic.AtomicReference

/**
 * `copypaste://pair` payloads for third-party QR scanners.
 *
 * The JSON invite codec stays URL-free. This wrapper is only a VIEW target so
 * a scanner can offer CopyPaste. The pairing code is never logged.
 */
object PairingDeepLinks {
    const val SCHEME = "copypaste"
    const val HOST = "pair"
    const val VERSION = "1"

    private val pending = AtomicReference<String?>(null)

    fun encode(code: String, listenAddr: String): String? {
        if (code.isEmpty() || listenAddr.isEmpty()) return null
        return Uri.Builder()
            .scheme(SCHEME)
            .authority(HOST)
            .appendQueryParameter("v", VERSION)
            .appendQueryParameter("code", code)
            .appendQueryParameter("listen_addr", listenAddr)
            .build()
            .toString()
            .takeIf { it.toByteArray(Charsets.UTF_8).size <= MAX_PAYLOAD_BYTES }
    }

    fun parse(uri: Uri?): String? {
        if (uri == null || uri.scheme != SCHEME) return null
        if (!uri.host.isNullOrEmpty() && uri.host != HOST) return null
        if (uri.host.isNullOrEmpty() && uri.path?.trim('/') != HOST) return null
        val version = uri.getQueryParameter("v") ?: VERSION
        if (version != VERSION) return null
        val code = uri.getQueryParameter("code").orEmpty()
        val listenAddr = uri.getQueryParameter("listen_addr").orEmpty()
        if (code.isEmpty() || listenAddr.isEmpty()) return null
        val payload = JSONObject()
            .put("version", 1)
            .put("code", code)
            .put("listen_addr", listenAddr)
            .toString()
        return payload.takeIf { it.toByteArray(Charsets.UTF_8).size <= MAX_PAYLOAD_BYTES }
    }

    fun offer(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        parse(intent.data)?.let { pending.set(it) }
    }

    fun take(): String? = pending.getAndSet(null)

    private const val MAX_PAYLOAD_BYTES = 512
}
